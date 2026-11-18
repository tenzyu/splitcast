"use strict";

const assert = require("node:assert/strict");
const { spawn } = require("node:child_process");
const { once } = require("node:events");
const { readFileSync } = require("node:fs");
const { createInterface } = require("node:readline");

class Browser {
  #child;
  #sequence = 0;
  #pending = new Map();
  #events = new Map();

  constructor(executable, profile) {
    this.#child = spawn(executable, [
      "--headless", "--disable-gpu", "--disable-background-networking", "--disable-sync",
      "--no-first-run", "--no-default-browser-check", "--remote-debugging-pipe",
      "--user-data-dir=" + profile, "about:blank",
    ], { stdio: ["ignore", "ignore", "inherit", "pipe", "pipe"] });
    let buffer = "";
    const rejectPending = (error) => {
      for (const request of this.#pending.values()) request.reject(error);
      this.#pending.clear();
    };
    this.#child.on("error", rejectPending);
    this.#child.on("exit", () => rejectPending(new Error("Chromium closed")));
    this.#child.stdio[3].on("error", rejectPending);
    this.#child.stdio[4].setEncoding("utf8");
    this.#child.stdio[4].on("data", (chunk) => {
      buffer += chunk;
      let end;
      while ((end = buffer.indexOf("\0")) !== -1) {
        const message = JSON.parse(buffer.slice(0, end));
        buffer = buffer.slice(end + 1);
        const request = this.#pending.get(message.id);
        if (request) {
          this.#pending.delete(message.id);
          if (message.error) request.reject(new Error(message.error.message));
          else request.resolve(message.result);
        } else if (message.method) {
          this.#events.get(message.method)?.(message.params);
        }
      }
    });
  }

  call(method, params = {}, sessionId) {
    const id = ++this.#sequence;
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve, reject });
      this.#child.stdio[3].write(JSON.stringify({ id, method, params, sessionId }) + "\0");
    });
  }

  on(method, callback) {
    this.#events.set(method, callback);
  }

  async close() {
    if (!this.#child.pid || this.#child.exitCode !== null || this.#child.signalCode !== null) return;
    const closed = once(this.#child, "exit");
    this.#child.kill("SIGTERM");
    const force = setTimeout(() => this.#child.kill("SIGKILL"), 1000);
    try {
      await closed;
    } finally {
      clearTimeout(force);
    }
  }
}

async function waitFor(check, label) {
  const deadline = performance.now() + 6500;
  while (performance.now() < deadline) {
    const value = await check();
    if (value) return value;
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  throw new Error("Timed out observing " + label);
}

async function exercise(browser, url, token, driver) {
  const { targetInfos } = await browser.call("Target.getTargets");
  const { targetId } = targetInfos.find((target) => target.type === "page");
  const { sessionId } = await browser.call("Target.attachToTarget", { targetId, flatten: true });
  const call = (method, params) => browser.call(method, params, sessionId);
  const evaluate = async (expression) => {
    const reply = await call("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
    assert.equal(reply.exceptionDetails, undefined, reply.exceptionDetails?.text);
    return reply.result.value;
  };
  const result = () => evaluate('document.querySelector("#result").textContent');
  const documentResult = async () => {
    const value = await result();
    return value.startsWith("{") ? JSON.parse(value) : {};
  };
  await call("Page.enable");
  await call("Fetch.enable", { patterns: [{ urlPattern: url + "api/command" }] });
  const requests = [];
  let race = false;
  let competition;
  let deferredStart;
  let delayedStartReply;
  let asynchronousFailure;
  browser.on("Fetch.requestPaused", (event) => {
    (async () => {
      if (event.responseStatusCode !== undefined) {
        assert.equal(event.requestId, deferredStart.requestId);
        const response = await call("Fetch.getResponseBody", { requestId: event.requestId });
        const text = response.base64Encoded ? Buffer.from(response.body, "base64").toString() : response.body;
        delayedStartReply = { status: event.responseStatusCode, body: JSON.parse(text) };
        await call("Fetch.continueResponse", { requestId: event.requestId });
        return;
      }
      const payload = JSON.parse(event.request.postData);
      requests.push(payload.command);
      if (payload.command === "start") {
        deferredStart = { requestId: event.requestId, args: payload.args };
        return;
      }
      if (race && payload.command === "say") {
        race = false;
        // A different authenticated client wins the current ID while the
        // browser request is held at the network boundary.
        const response = await fetch(url + "api/command", {
          method: "POST",
          headers: {
            Authorization: "Bearer " + token,
            Origin: new URL(url).origin,
            "Content-Type": "application/json",
          },
          body: event.request.postData,
          signal: AbortSignal.timeout(2000),
        });
        assert.equal(response.status, 200);
        competition = await response.json();
        assert.equal(competition.id, payload.args.id);
      }
      await call("Fetch.continueRequest", { requestId: event.requestId });
    })().catch((error) => { asynchronousFailure = error; });
  });

  await call("Page.navigate", { url });
  await waitFor(
    () => evaluate('document.title === "SplitCast" && document.readyState === "complete"'),
    "embedded Web assets",
  );
  await evaluate(`document.querySelector("#token").value = ${JSON.stringify(token)};
    document.querySelector("[data-command=status]").click(); true`);
  const ready = await waitFor(async () => {
    const status = await documentResult();
    return status.state === "ready" && status;
  }, "authenticated Ready");

  let publicDialog;
  browser.on("Page.javascriptDialogOpening", (dialog) => {
    publicDialog = dialog.type;
    call("Page.handleJavaScriptDialog", { accept: false })
      .catch((error) => { asynchronousFailure = error; });
  });
  await evaluate(`document.querySelector("#output").value = "twitch-live";
    document.querySelector("#duration").value = "60";
    document.querySelector("[data-command=start]").click(); true`);
  assert.equal(await waitFor(() => publicDialog, "public confirmation dialog"), "confirm");
  await waitFor(async () => (await documentResult()).state === "ready", "declined public Start");
  assert(!requests.includes("start"));

  const speak = () => evaluate(`document.querySelector("#speech").value = "fixture";
    document.querySelector("[data-command=say]").click(); true`);
  await speak();
  const first = await waitFor(async () => (await documentResult()).id, "browser speech admission");
  assert.equal(requests.filter((command) => command === "say").length, 1);

  race = true;
  await speak();
  await waitFor(async () => {
    if (asynchronousFailure) throw asynchronousFailure;
    return competition && (await result()).includes("not_executed");
  }, "browser rejection of the consumed speech ID");
  assert.notEqual(competition.id, first);
  assert.equal(requests.filter((command) => command === "say").length, 2);
  assert.equal(requests.filter((command) => command === "tts").length, 2);

  assert.equal((await driver.next()).value, "stop");
  await evaluate(`document.querySelector("#output").value = "recording";
    document.querySelector("#duration").value = "";
    document.querySelector("[data-command=start]").click(); true`);
  const delayed = await waitFor(() => deferredStart, "Start held before transmission");
  assert.equal(delayed.args.output, "recording");
  assert.equal(delayed.args.session_id, ready.start_id);
  assert.equal(typeof delayed.args.session_id, "string");
  await evaluate('document.querySelector("[data-command=stop]").click(); true');
  const pending = await waitFor(async () => {
    const status = await documentResult();
    return status.stop === "requested" && status;
  }, "admitted Stop before cleanup");
  assert.equal(pending.outcome, "accepted");
  const completed = await waitFor(async () => {
    const status = await documentResult();
    return status.stop === "completed" && status;
  }, "completed browser Stop");
  assert.equal(completed.stop_id, pending.stop_id);
  assert.equal(completed.state, "idle");
  assert.equal(completed.outcome, "completed");
  await call("Fetch.continueRequest", { requestId: delayed.requestId, interceptResponse: true });
  const late = await waitFor(() => {
    if (asynchronousFailure) throw asynchronousFailure;
    return delayedStartReply;
  }, "Agent response to Start delayed past Stop");
  assert.equal(late.status, 409);
  assert.equal(late.body.outcome, "not_executed");
  assert.equal((await documentResult()).stop_id, completed.stop_id);
  assert.equal(requests.filter((command) => command === "start").length, 1);
  assert.equal(requests.filter((command) => command === "stop").length, 1);
  assert.equal(await evaluate("sessionStorage.length + localStorage.length"), 0);
  return { first, stale: competition.id, delayed_start: late.body.outcome, stop: completed };
}

async function main() {
  const [executable, profile, url, tokenFile] = process.argv.slice(2);
  const token = readFileSync(tokenFile, "utf8").trim();
  const input = createInterface({ input: process.stdin });
  const browser = new Browser(executable, profile);
  const deadline = AbortSignal.timeout(12000);
  let timer;
  const runDeadline = new Promise((_, reject) => {
    timer = () => reject(new Error("Browser fixture exceeded 12 seconds"));
    deadline.addEventListener("abort", timer);
  });
  try {
    const receipt = await Promise.race([
      exercise(browser, url, token, input[Symbol.asyncIterator]()),
      runDeadline,
    ]);
    console.log(JSON.stringify(receipt));
  } finally {
    deadline.removeEventListener("abort", timer);
    input.close();
    process.stdin.pause();
    await browser.close();
  }
}

main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
