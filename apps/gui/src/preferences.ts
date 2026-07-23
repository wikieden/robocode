import type { Locale } from "./i18n/catalog";
import type {
  Density,
  EffectiveMode,
  Motion,
  PreferenceDiagnostic,
  Skin,
} from "./ui/theme";

export interface ResolvedPreferences {
  locale: Locale;
  skin: Skin;
  mode: EffectiveMode;
  density: Density;
  motion: Motion;
  diagnostics: PreferenceDiagnostic[];
}

export type PreferenceDraft = Partial<Omit<ResolvedPreferences, "diagnostics">>;

export interface PreferenceState {
  resolved: ResolvedPreferences;
  draft: PreferenceDraft | null;
  dirty: boolean;
}

export interface UnavailablePreferenceIntent {
  status: "unavailable";
  capability: "ui.preferences.mutation" | "ui.preferences.restore_defaults";
  diagnostic: PreferenceDiagnostic;
}

function normalizeLocale(value: unknown): Locale | undefined {
  if (value === "en" || value === "zh-CN") {
    return value;
  }
  return undefined;
}

export function resolveLocale(
  explicit: unknown,
  coreResolved: unknown,
  system: unknown,
): Locale {
  return (
    normalizeLocale(explicit) ??
    normalizeLocale(coreResolved) ??
    (typeof system === "string" && /^zh(?:[-_.]|$)/i.test(system) ? "zh-CN" : "en")
  );
}

export function createPreferenceState(resolved: ResolvedPreferences): PreferenceState {
  return { resolved, draft: null, dirty: false };
}

export function updatePreferenceDraft(
  state: PreferenceState,
  update: PreferenceDraft,
): PreferenceState {
  return {
    ...state,
    draft: { ...(state.draft ?? {}), ...update },
    dirty: true,
  };
}

export function acceptResolvedPreferences(
  state: PreferenceState,
  resolved: ResolvedPreferences,
): PreferenceState {
  return { ...state, resolved, draft: null, dirty: false };
}

function unavailable(
  capability: UnavailablePreferenceIntent["capability"],
): UnavailablePreferenceIntent {
  return {
    status: "unavailable",
    capability,
    diagnostic: {
      code: "gui.core_contract_unavailable",
      key: "GUI-CORE-005",
      field: "capability",
      rejectedValue: capability,
    },
  };
}

export function requestPreferenceSave(_state: PreferenceState): UnavailablePreferenceIntent {
  return unavailable("ui.preferences.mutation");
}

export function requestPreferenceRestore(): UnavailablePreferenceIntent {
  return unavailable("ui.preferences.restore_defaults");
}
