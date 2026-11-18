"use strict";

const byId = (id) => document.getElementById(id);
const buttons = Array.from(document.querySelectorAll("[data-command]"));
let sequence = 0;

async function call(command, args) {
  const response = await fetch("/api/command", {
    method: "POST",
    mode: "same-origin",
    credentials: "omit",
    redirect: "error",
    headers: {
      "Authorization": "Bearer " + byId("token").value.trim(),
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ command, args }),
  });
  const reply = await response.json();
  if (!response.ok) {
    throw new Error([reply.error, reply.outcome].filter(Boolean).join(" · ") || response.statusText);
  }
  return reply;
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
    if (command === "start") {
      args.output = byId("output").value;
      if (args.output === "twitch-live" && !window.confirm("Start a public Twitch broadcast?")) return;
    }
    if (command === "say") args.text = byId("speech").value;
    if (!byId("token").value.trim()) {
      byId("token").focus();
      byId("result").textContent = "Enter the local control token.";
      return;
    }

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
      const reply = await call(command, args);
      if (current === sequence) show(reply);
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
