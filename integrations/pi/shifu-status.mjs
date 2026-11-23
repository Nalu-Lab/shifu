// Shifu's status extension for Pi. Shifu writes this file under SHIFU_HOME at every start and adds it with
// `--extension`; edits here are overwritten. It only reports to `shifu hook pi <event>` while Pi runs as a Shifu
// agent (SHIFU_ID set). The user's own extensions, providers and settings load as usual.
import { spawn } from "node:child_process";

const SHIFU = "__SHIFU_BIN__";

export default function (pi) {
  // A pi started from inside this agent (its bash tool) inherits SHIFU_ID; only the outermost one reports.
  const owner = process.env.SHIFU_STATUS_OWNER;
  if (!process.env.SHIFU_ID || (owner && owner !== String(process.pid))) return;
  process.env.SHIFU_STATUS_OWNER = String(process.pid);

  let queue = Promise.resolve();
  function send(event, input) {
    return new Promise((resolve) => {
      let child;
      try {
        child = spawn(SHIFU, ["hook", "pi", event], { stdio: ["pipe", "ignore", "ignore"] });
      } catch {
        return resolve();
      }
      const timer = setTimeout(() => child.kill(), 5000);
      child.on("error", () => { clearTimeout(timer); resolve(); });
      child.on("close", () => { clearTimeout(timer); resolve(); });
      child.stdin.on("error", () => {}); // The hook may exit before reading when the agent was removed.
      child.stdin.end(JSON.stringify(input));
    });
  }
  // Hooks are separate processes: one at a time keeps the order Pi emitted.
  function hook(event, ctx, extra = {}) {
    const input = { session_id: ctx.sessionManager.getSessionFile(), ...extra };
    queue = queue.then(() => send(event, input));
    return queue;
  }

  pi.on("session_start", (_event, ctx) => hook("SessionStart", ctx));
  pi.on("agent_start", (_event, ctx) => hook("UserPromptSubmit", ctx));
  pi.on("agent_end", (event, ctx) => {
    const last = [...event.messages].reverse().find((m) => m.role === "assistant");
    const text = (last?.content ?? []).filter((c) => c.type === "text").map((c) => c.text).join("\n")
      || (last?.errorMessage ? `Error: ${last.errorMessage}` : "");
    return hook("Stop", ctx, text ? { last_assistant_message: text } : {});
  });
  // /new, /resume and /fork end this session and start another (session_start follows); only quitting is an end.
  pi.on("session_shutdown", (event, ctx) => (!event?.reason || event.reason === "quit") ? hook("SessionEnd", ctx) : undefined);
}
