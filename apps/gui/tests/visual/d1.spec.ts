// @vitest-environment jsdom

import { describe, expect, test, vi } from "vitest";

import { renderD1Cockpit, type D1IntentResult } from "../../src/screens/d1_cockpit";
import {
  DENSITIES,
  MOTIONS,
  VALID_SKIN_MODE_PAIRS,
  applyResolvedTheme,
  resolveTheme,
} from "../../src/ui/theme";
import { D1_PROJECTION } from "../support/d1_projection";

function renderCase(
  locale: "en" | "zh-CN",
  pair: (typeof VALID_SKIN_MODE_PAIRS)[number],
  density: (typeof DENSITIES)[number],
  motion: "system" | "reduced",
  fontScale = "100%",
) {
  document.body.innerHTML = '<main id="app"></main>';
  document.documentElement.lang = locale;
  document.documentElement.style.fontSize = fontScale;
  const [skin, mode] = pair.split("/") as [
    typeof D1_PROJECTION.preferences.skin,
    typeof D1_PROJECTION.preferences.mode,
  ];
  const preferences = { locale, skin, mode, density, motion, diagnostics: [] };
  applyResolvedTheme(document.documentElement, resolveTheme(preferences));
  const root = document.querySelector<HTMLElement>("#app")!;
  const result: D1IntentResult = {
    projection: { ...D1_PROJECTION, preferences },
    pendingCommandId: null,
    outcome: { state: "confirmed", reason: null },
  };
  const controller = renderD1Cockpit(
    root,
    result.projection,
    vi.fn(async () => result),
    vi.fn(async () => result),
    undefined,
    undefined,
    { poll: false },
  );
  return { root, controller };
}

describe("D1 visual preference matrix", () => {
  test.each(
    (["en", "zh-CN"] as const).flatMap((locale) =>
      VALID_SKIN_MODE_PAIRS.flatMap((pair) =>
        DENSITIES.flatMap((density) =>
          (["system", "reduced"] as const).map((motion) => ({
            locale,
            pair,
            density,
            motion,
          })),
        ),
      ),
    ),
  )(
    "renders D1 without clipping-prone structural overlap in $locale $pair $density $motion",
    ({ locale, pair, density, motion }) => {
      const { root, controller } = renderCase(locale, pair, density, motion);
      const [skin, mode] = pair.split("/");

      expect(document.documentElement.lang).toBe(locale);
      expect(document.documentElement.style.fontSize).toBe("100%");
      expect(document.documentElement.dataset).toMatchObject({ skin, mode, density, motion });
      expect(root.querySelector("[data-shell-landmark='topbar']")).not.toBeNull();
      expect(root.querySelector("[data-shell-landmark='activity-rail']")).not.toBeNull();
      expect(root.querySelector("[data-shell-landmark='lane-work-surface']")).not.toBeNull();
      expect(root.querySelector("[data-shell-landmark='statusbar']")).not.toBeNull();
      expect(root.querySelector("[data-permission-region] [data-composer]")).toBeNull();
      expect(root.querySelector("[data-composer]")?.getAttribute("aria-label")).toBeTruthy();
      expect(root.querySelector("[data-composer-send]")?.getAttribute("aria-label")).toBeTruthy();
      expect(root.textContent).not.toContain("[missing:");
      controller.dispose();
    },
  );

  test("keeps zh-CN primary and assistive D1 copy localized on the complete work surface", () => {
    const { root, controller } = renderCase("zh-CN", "ice/light", "comfy", "reduced");

    expect(root.querySelector('[aria-label="转录"]')).not.toBeNull();
    expect(root.querySelector("[data-composer]")?.getAttribute("aria-label")).toBe(
      "输入给当前 Lane 的消息",
    );
    expect(root.querySelector("[data-composer-send]")?.getAttribute("aria-label")).toBe(
      "发送消息",
    );
    expect(root.textContent).toContain("实时工作");
    expect(root.textContent).not.toContain("Transcript");
    expect(root.textContent).not.toContain("Live Work");
    expect(root.textContent).not.toContain("GUI-CORE-");
    controller.dispose();
  });

  test("renders the D1 surface at 200% font scale without changing Core-resolved preferences", () => {
    const { root, controller } = renderCase(
      "zh-CN",
      "ice/light",
      "comfy",
      "reduced",
      "200%",
    );

    expect(document.documentElement.style.fontSize).toBe("200%");
    expect(document.documentElement.dataset).toMatchObject({
      skin: "ice",
      mode: "light",
      density: "comfy",
      motion: "reduced",
    });
    expect(root.querySelector("[data-shell-landmark='topbar']")).not.toBeNull();
    expect(root.querySelector("[data-permission-region] [data-composer]")).toBeNull();
    expect(root.querySelector("[data-composer]")?.getAttribute("aria-label")).toBe(
      "输入给当前 Lane 的消息",
    );
    expect(root.querySelector("[data-composer-send]")?.getAttribute("aria-label")).toBe(
      "发送消息",
    );
    expect(root.textContent).not.toContain("[missing:");
    controller.dispose();
  });
});
