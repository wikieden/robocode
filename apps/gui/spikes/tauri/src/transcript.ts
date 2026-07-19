import type { ActionRecorder } from "./composer";

export class TranscriptModel {
  anchor: string | null = null;
  newOutputCount = 0;

  constructor(private readonly record: ActionRecorder) {}

  openHistoryAt(rowId: string): void {
    this.anchor = rowId;
    this.record(`history:${rowId}`);
  }

  appendNewOutput(rowId: string): void {
    this.newOutputCount += 1;
    this.record(`output:${rowId}`);
  }
}
