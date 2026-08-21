import { describe, expect, test } from "vitest";

import {
  CATALOGS,
  MESSAGE_ARGUMENT_NAMES,
  catalogKeys,
  catalogPlaceholders,
  translate,
} from "../src/i18n/catalog";
import { formatCode, formatPath, formatShortcut } from "../src/i18n/format";
import { resolveLocale } from "../src/preferences";

describe("localized catalog contract", () => {
  test("English and Chinese catalogs have identical keys and typed arguments", () => {
    expect(catalogKeys(CATALOGS.en)).toEqual(catalogKeys(CATALOGS["zh-CN"]));
    expect(catalogPlaceholders(CATALOGS.en)).toEqual(
      catalogPlaceholders(CATALOGS["zh-CN"]),
    );
    expect(catalogPlaceholders(CATALOGS.en)).toEqual(MESSAGE_ARGUMENT_NAMES);
  });

  test("interpolates typed arguments in both built-in locales", () => {
    expect(
      translate("en", "preferences.unavailable", { capability: "ui.preference_persistence" }),
    ).toContain("ui.preference_persistence");
    expect(
      translate("zh-CN", "preferences.unavailable", {
        capability: "ui.preference_persistence",
      }),
    ).toContain("ui.preference_persistence");
  });

  test("renders a visible sentinel for a missing key without a fallback loop", () => {
    expect(translate("zh-CN", "missing.key" as never, {} as never)).toBe(
      "[missing:missing.key]",
    );
  });

  test("locale precedence is explicit, Core-resolved, system, then English", () => {
    expect(resolveLocale("zh-CN", "en", "en-US")).toBe("zh-CN");
    expect(resolveLocale(undefined, "zh-CN", "en-US")).toBe("zh-CN");
    expect(resolveLocale(undefined, undefined, "zh-Hans-CN")).toBe("zh-CN");
    expect(resolveLocale(undefined, undefined, "fr-FR")).toBe("en");
    expect(resolveLocale("corrupt", "invalid", "")).toBe("en");
  });

  test("shortcuts, paths, and code remain literal", () => {
    expect(formatShortcut("Cmd+Shift+P")).toBe("Cmd+Shift+P");
    expect(formatPath("/tmp/项目/main.rs")).toBe("/tmp/项目/main.rs");
    expect(formatCode("resolve_theme()")).toBe("resolve_theme()");
  });
});
