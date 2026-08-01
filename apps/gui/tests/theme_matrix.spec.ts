// @vitest-environment jsdom

import { describe, expect, test } from "vitest";

import {
  VALID_SKIN_MODE_PAIRS,
  applyResolvedTheme,
  resolveTheme,
} from "../src/ui/theme";

describe("appearance resolution", () => {
  test("exports exactly the eight governed skin and mode pairs", () => {
    expect(VALID_SKIN_MODE_PAIRS).toEqual([
      "aurora/dark",
      "aurora/light",
      "ice/dark",
      "ice/light",
      "mono/dark",
      "mono/light",
      "amber/dark",
      "phosphor/dark",
    ]);
  });

  test.each(["amber", "phosphor"] as const)(
    "%s light falls back atomically and preserves the rejected pair",
    (skin) => {
      const result = resolveTheme({
        skin,
        mode: "light",
        density: "comfy",
        motion: "full",
      });

      expect(result).toMatchObject({
        skin: "aurora",
        mode: "dark",
        density: "regular",
        motion: "full",
      });
      expect(result.diagnostics).toContainEqual(
        expect.objectContaining({ rejectedValue: `${skin}/light` }),
      );
    },
  );

  test.each(["compact", "regular", "comfy"] as const)(
    "accepts the %s density",
    (density) => {
      expect(
        resolveTheme({ skin: "ice", mode: "dark", density, motion: "system" })
          .density,
      ).toBe(density);
    },
  );

  test.each(["system", "reduced", "full"] as const)(
    "accepts the %s motion policy",
    (motion) => {
      expect(
        resolveTheme({ skin: "mono", mode: "light", density: "regular", motion })
          .motion,
      ).toBe(motion);
    },
  );

  test("resolves system mode atomically and falls back to dark for corrupt host input", () => {
    expect(
      resolveTheme(
        { skin: "aurora", mode: "system", density: "regular", motion: "system" },
        "light",
      ),
    ).toMatchObject({ skin: "aurora", mode: "light" });

    const corrupt = resolveTheme(
      { skin: "ice", mode: "system", density: "regular", motion: "reduced" },
      "sepia",
    );
    expect(corrupt).toMatchObject({ skin: "ice", mode: "dark" });
    expect(corrupt.diagnostics).toContainEqual(
      expect.objectContaining({ field: "systemMode", rejectedValue: "sepia" }),
    );
  });

  test("retains corrupt preference diagnostics instead of silently overwriting", () => {
    const result = resolveTheme({
      skin: "neon",
      mode: "dark",
      density: "huge",
      motion: "instant",
      diagnostics: [
        {
          code: "core.preference_corrupt",
          key: "ui.preference",
          field: "skin",
          rejectedValue: "neon",
        },
      ],
    });

    expect(result).toMatchObject({
      skin: "aurora",
      mode: "dark",
      density: "regular",
      motion: "system",
    });
    expect(result.diagnostics).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ rejectedValue: "neon" }),
        expect.objectContaining({ rejectedValue: "huge" }),
        expect.objectContaining({ rejectedValue: "instant" }),
      ]),
    );
  });

  test("applies one resolved state to the document without partial mode changes", () => {
    const resolved = resolveTheme({
      skin: "phosphor",
      mode: "dark",
      density: "compact",
      motion: "reduced",
    });
    applyResolvedTheme(document.documentElement, resolved);

    expect(document.documentElement.dataset).toMatchObject({
      skin: "phosphor",
      mode: "dark",
      density: "compact",
      motion: "reduced",
    });
  });
});
