// @vitest-environment jsdom

import { beforeEach, describe, expect, test, vi } from "vitest";

import { renderAgentMenu, type AgentMenuModel } from "../src/components/agent_menu";

const MODEL: AgentMenuModel = {
  locale: "en",
  canCreateLane: true,
  probing: false,
  eligibilityDiagnostic: null,
  adapters: [
    {
      agentId: "codex",
      displayName: "Codex",
      startability: "ready",
      diagnostics: [],
    },
  ],
};

function setup(model: AgentMenuModel = MODEL) {
  document.body.innerHTML = '<div id="host"><button id="anchor">+</button></div>';
  const anchor = document.querySelector<HTMLButtonElement>("#anchor")!;
  const onSelect = vi.fn();
  const controller = renderAgentMenu(anchor, model, onSelect);
  return { anchor, onSelect, controller };
}

describe("compact agent menu", () => {
  beforeEach(() => {
    document.documentElement.lang = "en";
  });

  test("offers every ready Agent as the unique owner of a new Lane", () => {
    const { controller } = setup();

    expect(controller.root.textContent).toContain("NEW LANE");
    expect(controller.root.textContent).toContain("Viden Agent");
    expect(controller.root.textContent).not.toContain("DELEGATE TO CURRENT LANE");
    expect(controller.root.textContent).toContain("Codex");
    expect(
      controller.root.querySelector('[data-agent-id="codex"]')?.getAttribute("aria-disabled"),
    ).toBe("false");
  });

  test("presents built-in ACP adapters in the stable product order", () => {
    const { controller } = setup({
      ...MODEL,
      adapters: [
        {
          agentId: "custom-z",
          displayName: "Zed ACP",
          startability: "ready",
          diagnostics: [],
        },
        { agentId: "kiro-cli", displayName: "Kiro", startability: "ready", diagnostics: [] },
        {
          agentId: "claude-acp",
          displayName: "Claude",
          startability: "ready",
          diagnostics: [],
        },
        {
          agentId: "custom-acp",
          displayName: "Custom ACP",
          startability: "ready",
          diagnostics: [],
        },
        {
          agentId: "codex-acp",
          displayName: "Codex",
          startability: "ready",
          diagnostics: [],
        },
      ],
    });

    expect(
      Array.from(controller.root.querySelectorAll<HTMLElement>("[data-agent-id]")).map(
        (item) => item.dataset.agentId,
      ),
    ).toEqual(["codex-acp", "claude-acp", "kiro-cli", "custom-acp", "custom-z"]);
  });

  test("disables every Agent when Core says a new Lane is ineligible", () => {
    const { controller } = setup({ ...MODEL, canCreateLane: false });

    expect(
      controller.root.querySelector('[data-agent-id="codex"]')?.getAttribute("aria-disabled"),
    ).toBe("true");
  });

  test("uses roving keyboard focus and restores the trigger on Escape", () => {
    const { anchor, controller } = setup();
    const native = controller.root.querySelector<HTMLButtonElement>("[data-native-agent]")!;
    const acp = controller.root.querySelector<HTMLButtonElement>('[data-agent-id="codex"]')!;
    expect(document.activeElement).toBe(native);

    native.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    expect(document.activeElement).toBe(acp);
    acp.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));

    expect(document.activeElement).toBe(anchor);
    expect(document.querySelector('[role="menu"]')).toBeNull();
  });

  test("selects only a Core-ready item", () => {
    const { onSelect, controller } = setup();
    controller.root.querySelector<HTMLButtonElement>('[data-agent-id="codex"]')?.click();
    expect(onSelect).toHaveBeenCalledWith({ kind: "acp", agentId: "codex" });
  });

  test("does not steal focus from the task prompt opened by a selection", () => {
    document.body.innerHTML = '<div id="host"><button id="anchor">+</button></div>';
    const anchor = document.querySelector<HTMLButtonElement>("#anchor")!;
    const prompt = document.createElement("textarea");
    const controller = renderAgentMenu(anchor, MODEL, () => {
      document.body.append(prompt);
      prompt.focus();
    });

    controller.root.querySelector<HTMLButtonElement>("[data-native-agent]")?.click();

    expect(document.activeElement).toBe(prompt);
  });
});
