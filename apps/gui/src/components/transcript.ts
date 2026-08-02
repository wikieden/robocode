import type { D1TranscriptRow } from "../models/transcript";

export function transcriptAtBottom(element: HTMLElement, tolerance = 24): boolean {
  return element.scrollTop + element.clientHeight >= element.scrollHeight - tolerance;
}

/**
 * Keeps transcript semantics derived from the typed row kind, never from
 * display text. Only user and assistant rows participate in the center order.
 */
export interface TranscriptAgent {
  /** Agent id Core scoped the session by. */
  id: string;
  /** Name Core publishes for that agent, or the id when it published none. */
  displayName: string;
}

export function appendTranscriptRows(
  root: HTMLElement,
  rows: D1TranscriptRow[],
  /**
   * Agent that produced the assistant rows, when one produced them. Rows the
   * shell's own runtime produced have no agent and stay attributed to Viden.
   */
  agent?: TranscriptAgent,
): void {
  for (const row of rows) {
    const article = document.createElement("article");
    article.className = "d1-row";
    article.dataset.rowId = row.id;
    article.dataset.kind = row.kind;
    const semanticKind = row.kind.startsWith("user")
      ? "user"
      : row.kind.startsWith("assistant")
        ? "assistant"
        : null;
    if (semanticKind) {
      article.dataset.transcriptRow = semanticKind;
      article.dataset.centerStep = semanticKind;
    }
    const kind = document.createElement("span");
    kind.className = "d1-row-kind";
    if (semanticKind) {
      // An assistant row is attributed to whoever produced it. Only a reply
      // with no agent behind it belongs to the shell.
      const name =
        semanticKind === "user" ? "YOU" : (agent?.displayName.toUpperCase() ?? "VIDEN");
      if (semanticKind === "assistant" && agent) {
        article.dataset.agentId = agent.id;
      }
      const avatar = document.createElement("span");
      avatar.className = "d1-row-avatar";
      avatar.textContent = name.slice(0, 1);
      const label = document.createElement("span");
      label.dataset.rowLabel = "true";
      label.textContent = name;
      kind.append(avatar, label);
    } else {
      kind.textContent = row.kind.replaceAll("_", " ");
    }
    const content = document.createElement("pre");
    content.textContent = row.content;
    article.append(kind, content);
    root.append(article);
  }
}
