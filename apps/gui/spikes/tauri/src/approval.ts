import type { ActionRecorder } from "./composer";

export enum ApprovalChoice {
  AllowOnce = "allow-once",
  Deny = "deny",
}

export class ApprovalDockModel {
  lastChoice: ApprovalChoice | null = null;

  constructor(private readonly record: ActionRecorder) {}

  respond(choice: ApprovalChoice): void {
    this.lastChoice = choice;
    this.record(`approval:${choice}`);
  }
}
