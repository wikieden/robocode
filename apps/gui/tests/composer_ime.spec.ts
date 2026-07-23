// @vitest-environment jsdom

import { expect, test, vi } from "vitest";

import { renderD1Cockpit } from "../src/screens/d1_cockpit";
import { D1_PROJECTION } from "./support/d1_projection";

test("CJK composition, multiline paste, and undo do not submit prematurely", () => {
  document.body.innerHTML = '<main id="app"></main>';
  const root = document.querySelector<HTMLElement>("#app")!;
  const result = {
    projection: D1_PROJECTION,
    pendingCommandId: null,
    outcome: { state: "confirmed" as const, reason: null },
  };
  const send = vi.fn(async () => result);
  renderD1Cockpit(root, D1_PROJECTION, send, async () => result);
  const composer = root.querySelector<HTMLTextAreaElement>("[data-composer]")!;

  composer.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true }));
  composer.value = "你好";
  composer.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertCompositionText" }));
  composer.dispatchEvent(
    new KeyboardEvent("keydown", { key: "Enter", bubbles: true, isComposing: true }),
  );
  expect(send).not.toHaveBeenCalled();

  composer.dispatchEvent(new CompositionEvent("compositionend", { bubbles: true }));
  composer.value = "你好\n第二行";
  composer.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertFromPaste" }));
  composer.dispatchEvent(new InputEvent("beforeinput", { bubbles: true, inputType: "historyUndo" }));
  expect(send).not.toHaveBeenCalled();

  composer.value = "你好";
  composer.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
  expect(send).toHaveBeenCalledWith({
    type: "submit",
    laneId: "lane-core",
    content: "你好",
  });
});
