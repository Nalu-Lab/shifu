// Shifu's status plugin for opencode. Shifu writes this file under SHIFU_HOME at every start and names it in
// OPENCODE_CONFIG_CONTENT, which opencode merges over the user's own config; edits here are overwritten. It only
// reports to `shifu hook opencode <event>` while opencode runs as a Shifu agent (SHIFU_ID set).
import { spawn } from "node:child_process";

const SHIFU = "__SHIFU_BIN__";

export const ShifuStatus = async () => {
  // An opencode started from inside this agent (its bash tool) inherits SHIFU_ID and this plugin; only the
  // outermost one reports.
  const owner = process.env.SHIFU_STATUS_OWNER;
  if (!process.env.SHIFU_ID || (owner && owner !== String(process.pid))) return {};
  process.env.SHIFU_STATUS_OWNER = String(process.pid);

  let queue = Promise.resolve();
  function send(event, input) {
    return new Promise((resolve) => {
      let child;
      try {
        child = spawn(SHIFU, ["hook", "opencode", event], { stdio: ["pipe", "ignore", "ignore"] });
      } catch {
        return resolve();
      }
      const timer = setTimeout(() => child.kill(), 5000);
      child.on("error", () => { clearTimeout(timer); resolve(); });
      child.on("close", () => { clearTimeout(timer); resolve(); });
      child.stdin.on("error", () => {});
      child.stdin.end(JSON.stringify(input));
    });
  }
  // Hooks are separate processes: one at a time keeps the order opencode emitted. Not awaited by the event
  // handler, so opencode never waits on Shifu.
  function hook(event, input = {}) {
    queue = queue.then(() => send(event, input));
  }

  const children = new Set(); // sub-agent sessions (they have a parent): never the agent's own status
  const lastAssistant = new Map(); // sessionID -> id of its latest assistant message
  const texts = new Map(); // messageID -> Map(partID -> text)
  const busy = new Set();
  const failed = new Map(); // sessionID -> error text of the turn, or "abort"

  hook("SessionStart"); // the TUI is up; a session is created only with the first prompt

  return {
    event: async ({ event }) => {
      const p = event.properties ?? {};
      const sid = p.sessionID ?? p.info?.sessionID ?? p.part?.sessionID;
      if (event.type === "session.created" || event.type === "session.updated") {
        if (p.info?.parentID) children.add(p.info.id);
        else if (event.type === "session.created") hook("SessionStart", { session_id: p.info?.id, source: "startup" });
        return;
      }
      if (!sid || children.has(sid)) return;
      switch (event.type) {
        case "message.updated":
          if (p.info?.role === "assistant" && lastAssistant.get(sid) !== p.info.id) {
            texts.delete(lastAssistant.get(sid));
            lastAssistant.set(sid, p.info.id);
          }
          return;
        case "message.part.updated":
          if (p.part?.type === "text" && !p.part.synthetic) {
            if (!texts.has(p.part.messageID)) texts.set(p.part.messageID, new Map());
            texts.get(p.part.messageID).set(p.part.id, p.part.text ?? "");
          }
          return;
        case "session.status":
          if (p.status?.type === "busy" && !busy.has(sid)) {
            busy.add(sid);
            failed.delete(sid);
            hook("UserPromptSubmit", { session_id: sid });
          }
          return;
        case "session.error": {
          const e = p.error ?? {};
          failed.set(sid, e.name === "MessageAbortedError" ? "abort" : String(e.data?.message ?? e.name ?? "error"));
          return;
        }
        case "session.idle": {
          busy.delete(sid);
          const error = failed.get(sid);
          failed.delete(sid);
          if (error === "abort") return hook("Interrupt", { session_id: sid });
          const parts = texts.get(lastAssistant.get(sid));
          const text = parts ? [...parts.values()].join("\n").trim() : "";
          const message = text || (error ? `Error: ${error}` : "");
          return hook("Stop", { session_id: sid, ...(message ? { last_assistant_message: message } : {}) });
        }
        case "permission.asked":
          return hook("PermissionRequest", {
            session_id: sid,
            tool_name: p.permission,
            tool_input: { command: (p.patterns ?? []).join(" ") },
          });
        case "permission.replied":
          return hook("PermissionResult", { session_id: sid, tool_name: "permission" });
        case "question.asked":
          return hook("PreToolUse", { session_id: sid, tool_name: "ask_question", tool_input: { questions: p.questions ?? [] } });
        case "question.replied":
        case "question.rejected":
          return hook("PostToolUse", { session_id: sid, tool_name: "ask_question" });
      }
    },
  };
};
