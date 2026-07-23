export function shouldSubmitComposer(event: KeyboardEvent, composing: boolean): boolean {
  return event.key === "Enter" && !event.shiftKey && !event.isComposing && !composing;
}
