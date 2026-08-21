// @vitest-environment jsdom

import { describe, expect, test, vi } from "vitest";

import type { CoreClient } from "../src/host/core_client";
import {
  UI_PREFERENCE_PERSISTENCE_CAPABILITY,
  acceptResolvedPreferences,
  cancelPreferenceDraft,
  createPreferenceState,
  preferenceDraftPatch,
  requestPreferenceRestore,
  requestPreferenceSave,
  updatePreferenceDraft,
  type PreferenceIntentResult,
  type PreferenceState,
} from "../src/preferences";

const authoritative = {
  locale: "en" as const,
  skin: "aurora" as const,
  mode: "dark" as const,
  density: "regular" as const,
  motion: "system" as const,
  diagnostics: [],
};

function confirmed(
  overrides: Partial<PreferenceIntentResult> = {},
): PreferenceIntentResult {
  return {
    outcome: { state: "confirmed", reason: null },
    preferences: { ...authoritative, skin: "ice" },
    persisted: true,
    diagnostics: [],
    pendingCommandId: null,
    capabilityAvailable: true,
    ...overrides,
  };
}

function client(overrides: Partial<CoreClient> = {}): CoreClient {
  const unreachable = (name: string) => async () => {
    throw new Error(`unexpected CoreClient call: ${name}`);
  };
  return {
    preferencesAvailable: async () => true,
    preferencesSave: unreachable("preferences_save"),
    preferencesRestore: unreachable("preferences_restore"),
    preferencesPoll: unreachable("preferences_poll"),
    ...overrides,
  } as unknown as CoreClient;
}

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

  test("maps only the drafted axes into the typed Core patch", () => {
    const state = updatePreferenceDraft(createPreferenceState(authoritative), {
      locale: "system",
      mode: "system",
      density: "comfy",
    });

    // Unselected axes stay absent so Core keeps resolving them; `system` is a
    // real Core value, not a client-side "leave it alone".
    expect(preferenceDraftPatch(state.draft)).toEqual({
      locale: "system",
      mode: "system",
      density: "comfy",
    });
    expect(preferenceDraftPatch(null)).toEqual({});
  });

  test("a save sends the draft patch and confirms from Core's resolution", async () => {
    const state = updatePreferenceDraft(createPreferenceState(authoritative), {
      skin: "ice",
    });
    const preferencesSave = vi.fn(async () => confirmed());

    const result = await requestPreferenceSave(
      state,
      client({ preferencesSave } as Partial<CoreClient>),
      "gui-pref-1",
    );

    expect(preferencesSave).toHaveBeenCalledWith("gui-pref-1", { skin: "ice" });
    expect(result).toEqual({
      status: "confirmed",
      resolved: { ...authoritative, skin: "ice" },
      persisted: true,
      diagnostics: [],
    });
  });

  test("a restore confirms with no persisted table while the fallback resolves", async () => {
    const preferencesRestore = vi.fn(async () =>
      confirmed({ preferences: authoritative, persisted: false }),
    );

    const result = await requestPreferenceRestore(
      client({ preferencesRestore } as Partial<CoreClient>),
      "gui-pref-2",
    );

    expect(preferencesRestore).toHaveBeenCalledWith("gui-pref-2");
    expect(result).toEqual({
      status: "confirmed",
      resolved: authoritative,
      persisted: false,
      diagnostics: [],
    });
  });

  test("a Core rejection carries Core's own reason", async () => {
    const state = updatePreferenceDraft(createPreferenceState(authoritative), {
      skin: "amber",
      mode: "light",
    });

    const result = await requestPreferenceSave(
      state,
      client({
        preferencesSave: async () => ({
          outcome: { state: "rejected", reason: "amber has no light mode" },
          preferences: null,
          persisted: false,
          diagnostics: [],
          pendingCommandId: null,
          capabilityAvailable: true,
        }),
      } as Partial<CoreClient>),
      "gui-pref-3",
    );

    expect(result).toEqual({
      status: "rejected",
      reason: "amber has no light mode",
      diagnostics: [],
    });
  });

  test("keeps polling while Core has not published the receipt", async () => {
    const state = updatePreferenceDraft(createPreferenceState(authoritative), {
      density: "compact",
    });
    const preferencesPoll = vi
      .fn()
      .mockResolvedValueOnce(
        confirmed({ outcome: { state: "pending", reason: null }, pendingCommandId: "gui-pref-4" }),
      )
      .mockResolvedValueOnce(confirmed());

    const result = await requestPreferenceSave(
      state,
      client({
        preferencesSave: async () =>
          confirmed({
            outcome: { state: "pending", reason: null },
            preferences: null,
            persisted: false,
            pendingCommandId: "gui-pref-4",
          }),
        preferencesPoll,
      } as Partial<CoreClient>),
      "gui-pref-4",
    );

    expect(preferencesPoll).toHaveBeenCalledTimes(2);
    expect(result).toMatchObject({ status: "confirmed", persisted: true });
  });

  test("an absent capability is reported with the real Core capability id", async () => {
    const state = updatePreferenceDraft(createPreferenceState(authoritative), {
      density: "comfy",
    });
    const storageSpy = vi.spyOn(Storage.prototype, "setItem");

    const save = await requestPreferenceSave(
      state,
      client({
        preferencesSave: async () =>
          confirmed({
            outcome: { state: "idle", reason: null },
            preferences: null,
            persisted: false,
            capabilityAvailable: false,
          }),
      } as Partial<CoreClient>),
      "gui-pref-5",
    );
    const restore = await requestPreferenceRestore(
      client({
        preferencesRestore: async () => {
          throw new Error("missing Core capability `ui.preference_persistence`");
        },
      } as Partial<CoreClient>),
      "gui-pref-6",
    );

    for (const result of [save, restore]) {
      expect(result).toEqual({
        status: "unavailable",
        capability: UI_PREFERENCE_PERSISTENCE_CAPABILITY,
        diagnostic: expect.objectContaining({
          code: "gui.core_contract_unavailable",
          rejectedValue: UI_PREFERENCE_PERSISTENCE_CAPABILITY,
        }),
      });
    }
    // Nothing is ever mirrored into a second client-side preference store.
    expect(storageSpy).not.toHaveBeenCalled();
    storageSpy.mockRestore();
  });

  test("a host transport failure renders as a rejection, not a silent save", async () => {
    const state = updatePreferenceDraft(createPreferenceState(authoritative), {
      motion: "reduced",
    });

    const result = await requestPreferenceSave(
      state,
      client({
        preferencesSave: async () => {
          throw new Error("core pipe closed");
        },
      } as Partial<CoreClient>),
      "gui-pref-7",
    );

    expect(result).toMatchObject({ status: "rejected" });
    expect((result as { reason: string }).reason).toContain("core pipe closed");
  });

  test("changes rendered authority only after a resolved Core snapshot", () => {
    const draft = updatePreferenceDraft(createPreferenceState(authoritative), {
      locale: "zh-CN",
    });
    expect(draft.resolved.locale).toBe("en");

    const confirmedState: PreferenceState = acceptResolvedPreferences(draft, {
      ...authoritative,
      locale: "zh-CN",
    });
    expect(confirmedState.resolved.locale).toBe("zh-CN");
    expect(confirmedState.draft).toBeNull();
    expect(confirmedState.dirty).toBe(false);
  });

  test("cancel discards a GUI-local draft without changing Core-resolved authority", () => {
    const draft = updatePreferenceDraft(createPreferenceState(authoritative), {
      locale: "zh-CN",
      density: "comfy",
      motion: "reduced",
    });

    const cancelled = cancelPreferenceDraft(draft);

    expect(cancelled.resolved).toEqual(authoritative);
    expect(cancelled.draft).toBeNull();
    expect(cancelled.dirty).toBe(false);
  });
});
