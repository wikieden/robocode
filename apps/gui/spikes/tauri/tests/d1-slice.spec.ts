import { describe, expect, test } from "vitest";
import { JSDOM } from "jsdom";
import {
  createFixtureD1Slice,
  D1Slice,
  fixtureProjection,
  renderD1Slice,
  themeAttributes,
} from "../src/app";
import { ApprovalChoice } from "../src/approval";
import { Density, Skin } from "../src/theme";
import parityEvidence from "../../common/d1-slice-evidence.json";

function exerciseSlice(app: D1Slice): D1Slice {
  app.composer.beginComposition();
  app.composer.updateComposition("你好");
  expect(app.composer.submit()).toBe(false);
  app.composer.commitComposition();
  app.composer.paste("第一行\n第二行");
  app.startStream();
  app.queueCurrentDraft();
  app.cancelStream();
  app.approval.respond(ApprovalChoice.AllowOnce);
  app.transcript.openHistoryAt("row-120");
  app.transcript.appendNewOutput("row-50001");
  app.theme.select(Skin.IceLight, Density.Comfy);
  app.focus("composer");
  return app;
}

describe("equal D1 slice", () => {
  test("supports CJK streaming approval history and accessible focus", () => {
    const app = exerciseSlice(new D1Slice(fixtureProjection()));

    expect(app.composer.draft).toBe("你好第一行\n第二行");
    expect(app.transcript.anchor).toBe("row-120");
    expect(app.transcript.newOutputCount).toBe(1);
    expect(app.focusedRole).toBe("composer");
    expect(app.visibleFocus).toBe(true);
    expect(app.exposedRoles).toEqual(D1Slice.requiredRoles);
  });

  test("matches the shared action log and projection hash", () => {
    const projection = fixtureProjection();
    const app = exerciseSlice(new D1Slice(projection));

    expect(projection).toEqual({
      projectId: "project_viden",
      laneId: "lane_d1_core",
      sessionId: "session_d1-vertical-slice",
      taskId: "task_d1_core",
      viewHash: "7dd8faf04cca9f3013198e25823894eae91c2869e27087aa1eb0a34890cdf804",
    });

    expect(app.exposedRoles).toEqual(parityEvidence.required_roles);
    expect(app.actionLog).toEqual(parityEvidence.action_log);
    expect(app.projectionHash()).toBe(parityEvidence.projection_hash);
  });

  test("supports denial and keyboard-only focus traversal", () => {
    const app = new D1Slice(fixtureProjection());

    app.approval.respond(ApprovalChoice.Deny);
    const visited = D1Slice.requiredRoles.map(() => app.focusNext());

    expect(app.approval.lastChoice).toBe(ApprovalChoice.Deny);
    expect(visited).toEqual(D1Slice.requiredRoles);
    expect(app.focusedRole).toBe("new-output-count");
    expect(app.visibleFocus).toBe(true);
  });

  test("exposes both skins and all density choices", () => {
    const app = new D1Slice(fixtureProjection());

    for (const skin of [Skin.AuroraDark, Skin.IceLight]) {
      for (const density of [Density.Compact, Density.Regular, Density.Comfy]) {
        app.theme.select(skin, density);
        expect(app.theme.skin).toBe(skin);
        expect(app.theme.density).toBe(density);
      }
    }

    expect(themeAttributes(Skin.AuroraDark)).toEqual({ skin: "aurora", mode: "dark" });
    expect(themeAttributes(Skin.IceLight)).toEqual({ skin: "ice", mode: "light" });
  });

  test("routes real DOM composition, buttons, scroll, theme, and focus into the slice", () => {
    const dom = new JSDOM('<div id="app"></div>');
    const root = dom.window.document.querySelector<HTMLElement>("#app")!;
    const app = createFixtureD1Slice(fixtureProjection());
    renderD1Slice(root, app);

    const composer = root.querySelector<HTMLTextAreaElement>('[data-role="composer"]')!;
    composer.dispatchEvent(new dom.window.Event("compositionstart", { bubbles: true }));
    const update = new dom.window.Event("compositionupdate", { bubbles: true });
    Object.defineProperty(update, "data", { value: "你好" });
    composer.dispatchEvent(update);
    composer.dispatchEvent(new dom.window.KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    expect(app.actionLog.some((action) => action.startsWith("submit:"))).toBe(false);
    const end = new dom.window.Event("compositionend", { bubbles: true });
    Object.defineProperty(end, "data", { value: "你好" });
    composer.dispatchEvent(end);

    const paste = new dom.window.Event("paste", { bubbles: true, cancelable: true });
    Object.defineProperty(paste, "clipboardData", {
      value: { getData: () => "第一行\n第二行" },
    });
    composer.dispatchEvent(paste);
    root.querySelector<HTMLButtonElement>('[data-role="queue-action"]')!.click();
    root.querySelector<HTMLButtonElement>('[data-role="cancel-action"]')!.click();
    root.querySelector<HTMLButtonElement>('[data-choice="allow-once"]')!.click();
    root.querySelector<HTMLButtonElement>('[data-choice="deny"]')!.click();

    const history = root.querySelector<HTMLElement>('[data-role="history-viewport"]')!;
    history.dispatchEvent(new dom.window.Event("scroll"));
    history.dispatchEvent(new dom.window.CustomEvent("viden:new-output", { detail: "row-50001" }));

    const skin = root.querySelector<HTMLSelectElement>('[data-role="skin-select"]')!;
    const density = root.querySelector<HTMLSelectElement>('[data-role="density-select"]')!;
    skin.value = Skin.IceLight;
    skin.dispatchEvent(new dom.window.Event("change"));
    density.value = Density.Comfy;
    density.dispatchEvent(new dom.window.Event("change"));
    composer.focus();

    expect(app.composer.draft).toBe("你好第一行\n第二行");
    expect(app.approval.lastChoice).toBe(ApprovalChoice.Deny);
    expect(app.transcript.anchor).toBe("event-1");
    expect(app.transcript.newOutputCount).toBe(1);
    expect(app.theme.skin).toBe(Skin.IceLight);
    expect(app.theme.density).toBe(Density.Comfy);
    expect(app.focusedRole).toBe("composer");
    expect(root.querySelector('[data-role="composer"]')?.getAttribute("data-focus-visible")).toBe(
      "true",
    );
    expect(app.actionLog).toEqual([
      "stream:start",
      "composition:start",
      "composition:update:你好",
      "composition:commit:你好",
      "paste:第一行\\n第二行",
      "queue:你好第一行\\n第二行",
      "stream:cancel",
      "approval:allow-once",
      "approval:deny",
      "history:event-1",
      "output:row-50001",
      "theme:ice-light:regular",
      "theme:ice-light:comfy",
      "focus:composer",
    ]);
  });

  test("accepts committed framework input without replaying composition state", () => {
    const app = new D1Slice(fixtureProjection());

    app.syncComposerFromFramework("你好\nframework");

    expect(app.composer.draft).toBe("你好\nframework");
  });

  test("syncs ordinary textarea input through the real DOM binding", () => {
    const dom = new JSDOM('<div id="app"></div>');
    const root = dom.window.document.querySelector<HTMLElement>("#app")!;
    const app = new D1Slice(fixtureProjection());
    renderD1Slice(root, app);
    const composer = root.querySelector<HTMLTextAreaElement>('[data-role="composer"]')!;

    composer.value = "plain input";
    composer.dispatchEvent(new dom.window.InputEvent("input", { bubbles: true }));

    expect(app.composer.draft).toBe("plain input");
  });
});
