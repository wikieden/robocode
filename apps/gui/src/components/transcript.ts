import type { D1TranscriptRow } from "../models/transcript";

export function transcriptAtBottom(element: HTMLElement, tolerance = 24): boolean {
  return element.scrollTop + element.clientHeight >= element.scrollHeight - tolerance;
}

/**
 * Keeps transcript semantics derived from the typed row kind, never from
 * display text. Only user and assistant rows participate in the center order.
 */
export function appendTranscriptRows(root: HTMLElement, rows: D1TranscriptRow[]): void {
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
    kind.textContent = row.kind.replaceAll("_", " ");
    const content = document.createElement("pre");
    content.textContent = row.content;
    article.append(kind, content);
    root.append(article);
  }
}
