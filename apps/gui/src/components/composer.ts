export function shouldSubmitComposer(event: KeyboardEvent, composing: boolean): boolean {
  return event.key === "Enter" && !event.shiftKey && !event.isComposing && !composing;
}

export function shouldRouteComposerMutation(
  content: string,
  mutationBlockReason: string | null,
): boolean {
  return content.trim().length > 0 && mutationBlockReason === null;
}
