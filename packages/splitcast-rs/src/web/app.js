"use strict";

const byId = (id) => document.getElementById(id);
const buttons = Array.from(document.querySelectorAll("[data-command]"));
let sequence = 0;

async function call(token, command, args, timeout = 125000) {
  const response = await fetch("/api/command", {
    method: "POST",
    mode: "same-origin",
    credentials: "omit",
    redirect: "error",
    headers: {
      "Authorization": "Bearer " + token,
      "Content-Type": "application/json",
    },
    signal: AbortSignal.timeout(timeout),
    body: JSON.stringify({ command, args }),
  });
  const reply = await response.json();
  if (!response.ok) {
    const detail = [reply.error, reply.stop, reply.outcome].filter(Boolean).join(" · ");
    throw new Error(detail || response.statusText);
  }
  return reply;
}

async function observeStop(token, receipt, current) {
  const id = receipt.stop_id;
  const deadline = performance.now() + 17000;
  while (id && current === sequence && performance.now() < deadline) {
    const status = await call(token, "status", {}, 2000);
    if (status.stop_id !== id) break;
    if (status.stop === "completed" && status.outcome === "completed" && status.state === "idle") {
      return status;
    }
    if (status.stop !== "requested" || status.outcome !== "accepted") {
      return { ...status, cleanup: "unknown" };
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  return { ...receipt, cleanup: "unknown" };
}

function show(reply) {
  if (reply.state) {
    byId("state").textContent = [reply.role, reply.state, reply.output, reply.stop, reply.cleanup]
      .filter(Boolean).join(" · ");
  } else if (reply.stop) {
    byId("state").textContent = ["Stop", reply.stop, reply.cleanup].filter(Boolean).join(" · ");
  }
  if (typeof reply.image === "string" && /^data:image\/png;base64,[A-Za-z0-9+/=]+$/.test(reply.image)) {
    byId("preview").src = reply.image;
    byId("preview").hidden = false;
    byId("result").textContent = "";
    return;
  }
  byId("result").textContent = JSON.stringify(reply, null, 2);
}

for (const button of buttons) {
  button.addEventListener("click", async () => {
    const command = button.dataset.command;
    const args = JSON.parse(button.dataset.args || "{}");
    const token = byId("token").value.trim();
    if (!token) {
      byId("token").focus();
      byId("result").textContent = "Enter the local control token.";
      return;
    }
    if (command === "start") {
      args.output = byId("output").value;
      const duration = byId("duration");
      if (!duration.checkValidity()) {
        duration.reportValidity();
        return;
      }
      if (duration.value) args.duration_seconds = duration.valueAsNumber;
      if (args.output === "twitch-live") {
        if (!args.duration_seconds) {
          byId("result").textContent = "Set a maximum duration for public output.";
          duration.focus();
          return;
        }
      }
    }
    if (command === "say") args.text = byId("speech").value;
    const current = ++sequence;
    const controls = buttons.filter((item) => item.dataset.command !== "stop");
    for (const item of controls) item.disabled = true;
    if (command === "stop") {
      button.disabled = true;
      byId("preview").hidden = true;
      byId("preview").removeAttribute("src");
    }
    byId("result").textContent = command + " requested…";
    try {
      if (command === "start") {
        const status = await call(token, "status", {}, 5000);
        if (current !== sequence) return;
        if (typeof status.start_id !== "string") throw new Error("No session available for Start.");
        args.session_id = status.start_id;
        if (args.output === "twitch-live") {
          if (status.state !== "ready" && status.state !== "running") {
            throw new Error("Prepare the media session before starting a public broadcast.");
          }
          if (!window.confirm("Start a public Twitch broadcast for up to " + args.duration_seconds + " seconds?")) {
            show(status);
            return;
          }
          args.confirm_public = true;
        }
      }
      if (command === "say") {
        const status = await call(token, "tts", {}, 5000);
        if (current !== sequence) return;
        if (typeof status.next_id !== "string") throw new Error("No current speech ID.");
        args.id = status.next_id;
      }
      const reply = await call(token, command, args);
      if (current !== sequence) return;
      show(reply);
      if (command === "stop" && reply.stop === "requested") {
        let status = { ...reply, cleanup: "unknown" };
        try {
          status = await observeStop(token, reply, current);
        } finally {
          if (current === sequence) show(status);
        }
      }
    } catch (error) {
      if (current === sequence) {
        byId("result").textContent = error.message + " Check status and use Stop before retrying an uncertain operation.";
      }
    } finally {
      if (current === sequence) {
        for (const item of controls) item.disabled = false;
      }
      if (command === "stop") button.disabled = false;
    }
  });
}
