/** Right-click items shared by the tab and the sidebar: 标为未读 and 置顶. */
import type { AgentView } from "./format";
import type { MenuItem } from "./menu";
import { getPinnedTasks, markUnread, setTaskPinned } from "./store";

/** 标为未读: a reply already looked at counts as new again, back in 在等你. */
export function unreadItem(agent: AgentView | undefined): MenuItem[] {
  return agent?.status === "needs_reply" && agent.mark === "parked" ? [{ label: "标为未读", run: () => markUnread(agent.name) }] : [];
}

/** 置顶 keeps the task first in its project. */
export function pinItem(taskId: string | undefined): MenuItem[] {
  if (!taskId) return [];
  const pinned = getPinnedTasks().has(taskId);
  return [{ label: pinned ? "取消置顶" : "置顶", run: () => setTaskPinned(taskId, !pinned) }];
}
