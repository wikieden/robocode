export function transcriptAtBottom(element: HTMLElement, tolerance = 24): boolean {
  return element.scrollTop + element.clientHeight >= element.scrollHeight - tolerance;
}
