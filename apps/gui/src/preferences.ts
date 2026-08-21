import type { CoreClient } from "./host/core_client";
import type { Locale } from "./i18n/catalog";
import {
  resolveTheme,
  type Density,
  type EffectiveMode,
  type Motion,
  type PreferenceDiagnostic,
  type RequestedMode,
  type Skin,
} from "./ui/theme";

/**
 * Personal UI preferences are Core-owned.
 *
 * Core previews the write, runs the permission gate, writes the user `[ui]`
 * table, re-resolves precedence, and publishes `UiPreferencesUpdated`. This
 * module holds the draft the operator is editing and the request/reconcile
 * helpers around that contract. It never persists a preference itself, never
 * re-resolves precedence, and never treats its own draft as confirmation: only
 * a confirmed Core result may become rendered authority.
 */

/**
 * The capability id Core actually publishes
 * (`FRONTEND_V1_EXTENSION_CAPABILITIES`, `crates/types/src/protocol.rs`). The
 * client must not invent a finer-grained one — an unpublished id can never
 * become available, so a panel gated on it would stay dead forever.
 */
export const UI_PREFERENCE_PERSISTENCE_CAPABILITY = "ui.preference_persistence";

export interface ResolvedPreferences {
  locale: Locale;
  skin: Skin;
  mode: EffectiveMode;
  density: Density;
  motion: Motion;
  diagnostics: PreferenceDiagnostic[];
}

/** A locale as *requested*; `system` is a Core value, not a client fallback. */
export type RequestedLocale = Locale | "system";

/**
 * The axes the operator explicitly selected.
 *
 * Locale and mode accept `system` because Core stores that request and
 * resolves it per host; `ResolvedPreferences` only ever carries the concrete
 * value Core resolved it to.
 */
export interface PreferenceDraft {
  locale?: RequestedLocale;
  skin?: Skin;
  mode?: RequestedMode;
  density?: Density;
  motion?: Motion;
}

export interface PreferenceState {
  resolved: ResolvedPreferences;
  draft: PreferenceDraft | null;
  dirty: boolean;
}

/** The wire patch the host translates into a typed `UiPreferencePatch`. */
export interface PreferencePatch {
  locale?: string;
  skin?: string;
  mode?: string;
  density?: string;
  motion?: string;
}

/** The host's reply to one preference command; mirrors `PreferenceIntentResult`. */
export interface PreferenceIntentResult {
  outcome: { state: "idle" | "pending" | "confirmed" | "rejected"; reason: string | null };
  preferences: ResolvedPreferences | null;
  persisted: boolean;
  diagnostics: PreferenceDiagnostic[];
  pendingCommandId: string | null;
  capabilityAvailable: boolean;
}

export interface UnavailablePreferenceIntent {
  status: "unavailable";
  capability: typeof UI_PREFERENCE_PERSISTENCE_CAPABILITY;
  diagnostic: PreferenceDiagnostic;
}

export interface ConfirmedPreferenceIntent {
  status: "confirmed";
  resolved: ResolvedPreferences;
  /**
   * Whether Core kept a user `[ui]` table. A confirmed restore reports
   * `false` while `resolved` still shows the fallback, exactly as
   * `docs/frontend-integration-contract.md` describes.
   */
  persisted: boolean;
  diagnostics: PreferenceDiagnostic[];
}

export interface RejectedPreferenceIntent {
  status: "rejected";
  /** Core's own reason. The client never rewrites or guesses it. */
  reason: string;
  diagnostics: PreferenceDiagnostic[];
}

export type PreferenceIntentOutcome =
  | ConfirmedPreferenceIntent
  | RejectedPreferenceIntent
  | UnavailablePreferenceIntent;

/** Bounded reconciliation budget for a command Core has accepted but not answered. */
const PREFERENCE_POLL_ATTEMPTS = 24;

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

export function cancelPreferenceDraft(state: PreferenceState): PreferenceState {
  return { ...state, draft: null, dirty: false };
}

/**
 * Projects the draft into the wire patch.
 *
 * Only selected axes appear, so an axis the operator never touched keeps
 * whatever Core resolves for it rather than being pinned to the value that
 * happened to be showing.
 */
export function preferenceDraftPatch(draft: PreferenceDraft | null): PreferencePatch {
  if (!draft) return {};
  const patch: PreferencePatch = {};
  if (draft.locale !== undefined) patch.locale = draft.locale;
  if (draft.skin !== undefined) patch.skin = draft.skin;
  if (draft.mode !== undefined) patch.mode = draft.mode;
  if (draft.density !== undefined) patch.density = draft.density;
  if (draft.motion !== undefined) patch.motion = draft.motion;
  return patch;
}

/** The already-resolved preference values as a screen projection carries them. */
export interface WirePreferences {
  locale: Locale;
  skin: string;
  mode: string;
  density: string;
  motion: string;
  diagnostics: unknown[];
}

/**
 * Narrows a projection's wire preference values into the typed resolution.
 *
 * This is normalization of a value Core already resolved, not a second
 * precedence rule: an unreadable value keeps Core's own diagnostics and picks
 * up the shared theme fallback diagnostic rather than being dropped in
 * silence.
 */
export function resolvedPreferencesFromWire(wire: WirePreferences): ResolvedPreferences {
  const theme = resolveTheme({
    skin: wire.skin,
    mode: wire.mode,
    density: wire.density,
    motion: wire.motion,
    diagnostics: wire.diagnostics as readonly PreferenceDiagnostic[],
  });
  return { locale: wire.locale, ...theme };
}

function unavailable(): UnavailablePreferenceIntent {
  return {
    status: "unavailable",
    capability: UI_PREFERENCE_PERSISTENCE_CAPABILITY,
    diagnostic: {
      code: "gui.core_contract_unavailable",
      key: UI_PREFERENCE_PERSISTENCE_CAPABILITY,
      field: "capability",
      rejectedValue: UI_PREFERENCE_PERSISTENCE_CAPABILITY,
    },
  };
}

/**
 * Reconciles one host reply into a rendered outcome.
 *
 * Only `confirmed` carries a resolution, so a pending or failed command can
 * never leave the panel showing a preference Core did not publish.
 */
function reconcile(result: PreferenceIntentResult): PreferenceIntentOutcome {
  if (!result.capabilityAvailable) return unavailable();
  if (result.outcome.state === "rejected") {
    return {
      status: "rejected",
      reason: result.outcome.reason ?? "Core rejected the preference change.",
      diagnostics: result.diagnostics,
    };
  }
  if (result.outcome.state === "confirmed" && result.preferences) {
    return {
      status: "confirmed",
      resolved: result.preferences,
      persisted: result.persisted,
      diagnostics: result.diagnostics,
    };
  }
  return {
    status: "rejected",
    reason: result.outcome.reason ?? "Core has not confirmed the preference change.",
    diagnostics: result.diagnostics,
  };
}

/**
 * A host failure is still an operator-visible fact.
 *
 * A missing capability arrives as a host error when the adapter fails closed
 * on it, so it is classified as unavailable rather than as a rejection the
 * operator could retry into.
 */
function failure(error: unknown): PreferenceIntentOutcome {
  const reason = error instanceof Error ? error.message : String(error);
  return reason.includes(UI_PREFERENCE_PERSISTENCE_CAPABILITY)
    ? unavailable()
    : { status: "rejected", reason, diagnostics: [] };
}

/** Drains the host until the accepted command reaches a terminal outcome. */
async function settle(
  core: CoreClient,
  first: PreferenceIntentResult,
): Promise<PreferenceIntentOutcome> {
  let result = first;
  for (
    let attempt = 0;
    result.pendingCommandId !== null && attempt < PREFERENCE_POLL_ATTEMPTS;
    attempt += 1
  ) {
    result = await core.preferencesPoll();
  }
  return reconcile(result);
}

/**
 * Sends the current draft as one Core preference mutation.
 *
 * The returned outcome is the only thing a caller may render as authority; the
 * draft stays unsaved until `status === "confirmed"`.
 */
export async function requestPreferenceSave(
  state: PreferenceState,
  core: CoreClient,
  commandId: string,
): Promise<PreferenceIntentOutcome> {
  try {
    return await settle(
      core,
      await core.preferencesSave(commandId, preferenceDraftPatch(state.draft)),
    );
  } catch (error: unknown) {
    return failure(error);
  }
}

/** Asks Core to drop the user `[ui]` table and fall back to its own defaults. */
export async function requestPreferenceRestore(
  core: CoreClient,
  commandId: string,
): Promise<PreferenceIntentOutcome> {
  try {
    return await settle(core, await core.preferencesRestore(commandId));
  } catch (error: unknown) {
    return failure(error);
  }
}
