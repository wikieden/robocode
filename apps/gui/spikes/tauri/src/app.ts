import { ApprovalChoice, ApprovalDockModel } from "./approval";
import { ComposerModel } from "./composer";
import { Density, Skin, ThemeModel } from "./theme";
import { TranscriptModel } from "./transcript";
import d1Fixture from "../../../../../crates/types/tests/fixtures/frontend-contract-v1/d1-vertical-slice.json";

export interface ProjectionState {
  projectId: string;
  laneId: string;
  sessionId: string;
  taskId: string;
  viewHash: string;
}

export function fixtureProjection(): ProjectionState {
  const owner = d1Fixture.events[0].owner;
  const laneEvent = d1Fixture.events.find((event) => event.event.kind.type === "lane_updated");
  const taskEvent = d1Fixture.events.find((event) => event.event.kind.type === "task_updated");
  if (laneEvent?.event.kind.type !== "lane_updated" || taskEvent?.event.kind.type !== "task_updated") {
    throw new Error("canonical D1 fixture is missing lane/task projection facts");
  }
  return {
    projectId: owner.project_id,
    laneId: laneEvent.event.kind.payload.lane!.id,
    sessionId: owner.session_id,
    taskId: taskEvent.event.kind.payload.task!.id,
    viewHash: d1Fixture.expected_view_sha256,
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

  syncComposerFromFramework(value: string): void {
    this.composer.syncFromFramework(value);
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
    return this.projection.viewHash;
  }

  projectLabel(): string {
    return `${this.projection.projectId} / ${this.projection.laneId}`;
  }

  private record(action: string): void {
    this.actionLog.push(action);
  }
}

export function renderD1Slice(root: HTMLElement, app: D1Slice): void {
  const theme = themeAttributes(app.theme.skin);
  root.innerHTML = `
    <main class="d1-shell" data-skin="${theme.skin}" data-mode="${theme.mode}" data-density="${app.theme.density}">
      <header><strong>Viden</strong><span>${app.projectLabel()}</span></header>
      <section data-role="history-viewport" aria-label="Conversation history" tabindex="1">
        <article data-role="tool-row" aria-label="Tool activity">Core fixture ready</article>
        <output data-role="new-output-count" aria-live="polite">${app.transcript.newOutputCount}</output>
      </section>
      <aside data-role="approval-dock" aria-label="Permission request">
        <button type="button" data-choice="allow-once">Allow once</button>
        <button type="button" data-choice="deny">Deny</button>
      </aside>
      <footer>
        <textarea data-role="composer" aria-label="Message composer" tabindex="2"></textarea>
        <button data-role="queue-action" type="button">Queue</button>
        <button data-role="cancel-action" type="button">Cancel</button>
        <label>Skin
          <select data-role="skin-select">
            <option value="aurora-dark">Aurora dark</option>
            <option value="ice-light">Ice light</option>
          </select>
        </label>
        <label>Density
          <select data-role="density-select">
            <option value="compact">Compact</option>
            <option value="regular" selected>Regular</option>
            <option value="comfy">Comfy</option>
          </select>
        </label>
      </footer>
    </main>`;

  const shell = root.querySelector<HTMLElement>(".d1-shell")!;
  const composer = root.querySelector<HTMLTextAreaElement>('[data-role="composer"]')!;
  const history = root.querySelector<HTMLElement>('[data-role="history-viewport"]')!;
  const newOutput = root.querySelector<HTMLOutputElement>('[data-role="new-output-count"]')!;
  const skin = root.querySelector<HTMLSelectElement>('[data-role="skin-select"]')!;
  const density = root.querySelector<HTMLSelectElement>('[data-role="density-select"]')!;

  const sync = (): void => {
    composer.value = app.composer.draft;
    newOutput.value = String(app.transcript.newOutputCount);
    const attributes = themeAttributes(app.theme.skin);
    shell.dataset.skin = attributes.skin;
    shell.dataset.mode = attributes.mode;
    shell.dataset.density = app.theme.density;
    skin.value = app.theme.skin;
    density.value = app.theme.density;
    root.querySelectorAll<HTMLElement>("[data-focus-visible]").forEach((element) => {
      delete element.dataset.focusVisible;
    });
    if (app.focusedRole) {
      root.querySelector<HTMLElement>(`[data-role="${app.focusedRole}"]`)?.setAttribute(
        "data-focus-visible",
        "true",
      );
    }
  };

  composer.addEventListener("compositionstart", () => app.composer.beginComposition());
  composer.addEventListener("compositionupdate", (event) => {
    app.composer.updateComposition((event as CompositionEvent).data ?? "");
  });
  composer.addEventListener("compositionend", () => {
    app.composer.commitComposition();
    sync();
  });
  composer.addEventListener("input", (event) => {
    if (!(event as InputEvent).isComposing) {
      app.syncComposerFromFramework(composer.value);
    }
  });
  composer.addEventListener("paste", (event) => {
    const text = event.clipboardData?.getData("text/plain") ?? "";
    if (text) {
      event.preventDefault();
      app.composer.paste(text);
      sync();
    }
  });
  composer.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && !event.shiftKey && app.composer.submit()) {
      event.preventDefault();
    }
  });
  root.querySelector<HTMLButtonElement>('[data-role="queue-action"]')!.addEventListener(
    "click",
    () => app.queueCurrentDraft(),
  );
  root.querySelector<HTMLButtonElement>('[data-role="cancel-action"]')!.addEventListener(
    "click",
    () => app.cancelStream(),
  );
  root.querySelector<HTMLButtonElement>('[data-choice="allow-once"]')!.addEventListener(
    "click",
    () => app.approval.respond(ApprovalChoice.AllowOnce),
  );
  root.querySelector<HTMLButtonElement>('[data-choice="deny"]')!.addEventListener("click", () =>
    app.approval.respond(ApprovalChoice.Deny),
  );
  history.addEventListener("scroll", () => {
    if (history.dataset.anchor) {
      app.transcript.openHistoryAt(history.dataset.anchor);
    }
  });
  history.addEventListener("viden:new-output", (event) => {
    app.transcript.appendNewOutput((event as CustomEvent<string>).detail);
    sync();
  });
  skin.addEventListener("change", () => {
    app.theme.select(skin.value as Skin, app.theme.density);
    sync();
  });
  density.addEventListener("change", () => {
    app.theme.select(app.theme.skin, density.value as Density);
    sync();
  });
  root.querySelectorAll<HTMLElement>("[data-role]").forEach((element) => {
    element.addEventListener("focus", () => {
      const role = element.dataset.role;
      if (role && D1Slice.requiredRoles.includes(role as (typeof D1Slice.requiredRoles)[number])) {
        app.focus(role);
        sync();
      }
    });
  });
  sync();
}
