import { describe, expect, test } from "vitest";
import { D1Slice, fixtureProjection, themeAttributes } from "../src/app";
import { ApprovalChoice } from "../src/approval";
import { Density, Skin } from "../src/theme";

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
    const app = exerciseSlice(new D1Slice(fixtureProjection()));

    expect(app.actionLog).toEqual([
      "composition:start",
      "composition:update:你好",
      "composition:commit:你好",
      "paste:第一行\\n第二行",
      "stream:start",
      "queue:你好第一行\\n第二行",
      "stream:cancel",
      "approval:allow-once",
      "history:row-120",
      "output:row-50001",
      "theme:ice-light:comfy",
      "focus:composer",
    ]);
    expect(app.projectionHash()).toBe("e849d08e7c57e3a4");
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
});
