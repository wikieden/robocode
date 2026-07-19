export type ActionRecorder = (action: string) => void;

export class ComposerModel {
  private composing = false;
  private composition = "";
  private value = "";

  constructor(private readonly record: ActionRecorder) {}

  get draft(): string {
    return this.value;
  }

  beginComposition(): void {
    this.composing = true;
    this.composition = "";
    this.record("composition:start");
  }

  updateComposition(value: string): void {
    this.composition = value;
    this.record(`composition:update:${value}`);
  }

  commitComposition(): void {
    this.value += this.composition;
    this.record(`composition:commit:${this.composition}`);
    this.composition = "";
    this.composing = false;
  }

  paste(value: string): void {
    this.value += value;
    this.record(`paste:${value.replaceAll("\n", "\\n")}`);
  }

  submit(): boolean {
    if (this.composing) {
      return false;
    }
    this.record(`submit:${this.value.replaceAll("\n", "\\n")}`);
    return true;
  }
}
