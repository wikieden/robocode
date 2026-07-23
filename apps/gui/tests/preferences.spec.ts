// @vitest-environment jsdom

import { describe, expect, test, vi } from "vitest";

import {
  acceptResolvedPreferences,
  createPreferenceState,
  requestPreferenceRestore,
  requestPreferenceSave,
  updatePreferenceDraft,
} from "../src/preferences";

const authoritative = {
  locale: "en" as const,
  skin: "aurora" as const,
  mode: "dark" as const,
  density: "regular" as const,
  motion: "system" as const,
  diagnostics: [],
};

describe("Core-owned preference workflow", () => {
  test("keeps edits as a GUI-local unsaved draft", () => {
    const state = updatePreferenceDraft(createPreferenceState(authoritative), {
      locale: "zh-CN",
      skin: "ice",
      mode: "light",
    });

    expect(state.resolved).toEqual(authoritative);
    expect(state.draft).toMatchObject({ locale: "zh-CN", skin: "ice", mode: "light" });
    expect(state.dirty).toBe(true);
  });

  test("save and restore are typed unavailable until GUI-CORE-005 lands", () => {
    const storageSpy = vi.spyOn(Storage.prototype, "setItem");
    const state = updatePreferenceDraft(createPreferenceState(authoritative), {
      density: "compact",
    });

    expect(requestPreferenceSave(state)).toEqual({
      status: "unavailable",
      capability: "ui.preferences.mutation",
      diagnostic: expect.objectContaining({ code: "gui.core_contract_unavailable" }),
    });
    expect(requestPreferenceRestore()).toEqual({
      status: "unavailable",
      capability: "ui.preferences.restore_defaults",
      diagnostic: expect.objectContaining({ code: "gui.core_contract_unavailable" }),
    });
    expect(storageSpy).not.toHaveBeenCalled();
    storageSpy.mockRestore();
  });

  test("changes rendered authority only after a resolved Core snapshot", () => {
    const draft = updatePreferenceDraft(createPreferenceState(authoritative), {
      locale: "zh-CN",
    });
    expect(draft.resolved.locale).toBe("en");

    const confirmed = acceptResolvedPreferences(draft, {
      ...authoritative,
      locale: "zh-CN",
    });
    expect(confirmed.resolved.locale).toBe("zh-CN");
    expect(confirmed.draft).toBeNull();
    expect(confirmed.dirty).toBe(false);
  });
});
