import { ApprovalDockModel } from "./approval";
import { ComposerModel } from "./composer";
import { Skin, ThemeModel } from "./theme";
import { TranscriptModel } from "./transcript";

export interface ProjectionState {
  projectId: string;
  laneId: string;
  sessionId: string;
  taskId: string;
}

export function fixtureProjection(): ProjectionState {
  return {
    projectId: "project-1",
    laneId: "lane-1",
    sessionId: "session-1",
    taskId: "task-1",
  };
}

export function themeAttributes(skin: Skin): { skin: "aurora" | "ice"; mode: "dark" | "light" } {
  return skin === Skin.AuroraDark
    ? { skin: "aurora", mode: "dark" }
    : { skin: "ice", mode: "light" };
}

export class D1Slice {
  static readonly requiredRoles = [
    "composer",
    "tool-row",
    "approval-dock",
    "queue-action",
    "cancel-action",
    "history-viewport",
    "new-output-count",
  ] as const;

  readonly actionLog: string[] = [];
  readonly composer = new ComposerModel((action) => this.record(action));
  readonly approval = new ApprovalDockModel((action) => this.record(action));
  readonly transcript = new TranscriptModel((action) => this.record(action));
  readonly theme = new ThemeModel((action) => this.record(action));
  readonly exposedRoles = [...D1Slice.requiredRoles];
  focusedRole: string | null = null;
  visibleFocus = false;
  private streaming = false;

  constructor(private readonly projection: ProjectionState) {}

  startStream(): void {
    this.streaming = true;
    this.record("stream:start");
  }

  queueCurrentDraft(): void {
    if (!this.streaming) {
      throw new Error("queue is available only while streaming");
    }
    this.record(`queue:${this.composer.draft.replaceAll("\n", "\\n")}`);
  }

  cancelStream(): void {
    if (!this.streaming) {
      return;
    }
    this.streaming = false;
    this.record("stream:cancel");
  }

  focus(role: string): void {
    if (!this.exposedRoles.includes(role as (typeof D1Slice.requiredRoles)[number])) {
      throw new Error(`unknown role: ${role}`);
    }
    this.focusedRole = role;
    this.visibleFocus = true;
    this.record(`focus:${role}`);
  }

  focusNext(): (typeof D1Slice.requiredRoles)[number] {
    const currentIndex = this.focusedRole === null ? -1 : this.exposedRoles.indexOf(
      this.focusedRole as (typeof D1Slice.requiredRoles)[number],
    );
    const role = this.exposedRoles[(currentIndex + 1) % this.exposedRoles.length];
    this.focus(role);
    return role;
  }

  projectionHash(): string {
    const canonical = `${this.projection.projectId}|${this.projection.laneId}|${this.projection.sessionId}|${this.projection.taskId}\n${this.actionLog.join("\n")}`;
    let hash = 0xcbf29ce484222325n;
    for (const byte of new TextEncoder().encode(canonical)) {
      hash ^= BigInt(byte);
      hash = BigInt.asUintN(64, hash * 0x100000001b3n);
    }
    return hash.toString(16).padStart(16, "0");
  }

  private record(action: string): void {
    this.actionLog.push(action);
  }
}

export function renderD1Slice(root: HTMLElement, app: D1Slice): void {
  const theme = themeAttributes(app.theme.skin);
  root.innerHTML = `
    <main class="d1-shell" data-skin="${theme.skin}" data-mode="${theme.mode}" data-density="${app.theme.density}">
      <header><strong>Viden</strong><span>project-1 / lane-1</span></header>
      <section data-role="history-viewport" aria-label="Conversation history">
        <article data-role="tool-row" aria-label="Tool activity">Core fixture ready</article>
        <output data-role="new-output-count" aria-live="polite">${app.transcript.newOutputCount}</output>
      </section>
      <aside data-role="approval-dock" aria-label="Permission request">
        <button type="button">Allow once</button><button type="button">Deny</button>
      </aside>
      <footer>
        <textarea data-role="composer" aria-label="Message composer"></textarea>
        <button data-role="queue-action" type="button">Queue</button>
        <button data-role="cancel-action" type="button">Cancel</button>
      </footer>
    </main>`;
}
