/**
 * Main-window interaction capture harness.
 *
 * Renders exactly one screenshot state per `?state=` value using the shipped
 * render functions only. Nothing here reimplements a control, a label, or a
 * layout: every pixel comes from `src/screens/**` and `src/components/**`, so
 * a regression in production code shows up in the capture instead of being
 * masked by a harness copy of the UI.
 *
 * Provenance of the projections:
 *
 * - `d1*`, `settings*`, `d6-*` start from `tests/support/d1_projection.ts`,
 *   the same D1 fixture the vitest suites mount.
 * - `d12-*` starts from `../gui-screen-restore/projections/d12.json`, which
 *   `tests/capture_projections.rs` generates by running the real Rust
 *   projection over the canonical `frontend-contract-v1` `merge-gate.json`
 *   fixture. It is generated output and is never hand-edited.
 * - `d11` mirrors the intake fixtures in `tests/d11_intake.spec.ts`, because
 *   the D11 contract has no generated capture projection yet.
 *
 * Every hand-written value below is a *delta* on one of those sources and
 * carries a comment naming the fixture it mirrors.
 *
 * Determinism: the wall clock and `Math.random` are frozen before the first
 * render, so the elapsed-time readout in the work-status strip and any
 * id-shaped value are stable across captures. Host callbacks are stubs that
 * never resolve, so no state ever changes after the harness settles — except
 * `d6-error`, whose stub rejects with one fixed sentence on purpose.
 */

import "../../src/ui/tokens.css";
import "../../src/ui/theme.css";
import "../../src/ui/window_chrome.css";

import type { Locale } from "../../src/i18n/catalog";
import type { ComposerControlIntent } from "../../src/models/composer";
import type { PreferenceIntentOutcome } from "../../src/preferences";
import {
  renderD1Cockpit,
  type D1CockpitProjection,
  type D1IntentResult,
} from "../../src/screens/d1_cockpit";
import type { D6IntentResult, D6RecoveryProjection } from "../../src/screens/d6_recovery";
import {
  renderD11Intake,
  type D11IntakeProjection,
  type D11IntentResult,
} from "../../src/screens/d11_intake";
import {
  renderD12IntegrationGate,
  type D12IntegrationGateProjection,
  type D12IntentResult,
} from "../../src/screens/d12_integration_gate";
import {
  applyResolvedTheme,
  resolveTheme,
  type PreferenceDiagnostic,
} from "../../src/ui/theme";
import { D1_PROJECTION } from "../../tests/support/d1_projection";

/* ------------------------------------------------------------------ */
/* Determinism                                                         */
/* ------------------------------------------------------------------ */

/// Frozen wall clock. `renderWorkStatus` prints `now() - startedAt`, so an
/// unfrozen clock would make two captures of the same state differ.
const FROZEN_EPOCH_MS = Date.parse("2026-01-01T00:00:00.000Z");
Date.now = () => FROZEN_EPOCH_MS;
Math.random = () => 0;

/// A host callback that never answers. A screenshot state must not move after
/// it settles, and a never-resolving promise is the only stub that cannot
/// smuggle a state change into the capture.
const PENDING_FOREVER: Promise<never> = new Promise<never>(() => undefined);
const never = <T>(): Promise<T> => PENDING_FOREVER;

/// One macrotask, so a click's `await` chain and the following render flush
/// before the next harness step runs.
const tick = (): Promise<void> =>
  new Promise((resolve) => {
    window.setTimeout(resolve, 0);
  });

/// Waits for a node a production render is expected to produce. Bounded, so a
/// state that can no longer be reached fails visibly instead of hanging.
async function waitFor<T extends Element>(selector: string): Promise<T | null> {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    const found = document.querySelector<T>(selector);
    if (found) return found;
    await tick();
  }
  return null;
}

function click(selector: string): void {
  document.querySelector<HTMLElement>(selector)?.click();
}

/* ------------------------------------------------------------------ */
/* Parameters and theme                                                */
/* ------------------------------------------------------------------ */

const params = new URLSearchParams(window.location.search);
const state = params.get("state") ?? "d1";
const locale: Locale = params.get("locale") === "zh-CN" ? "zh-CN" : "en";
const mode = params.get("mode") === "light" ? "light" : "dark";
/// Skin follows mode the way the shipped design pairs them; the harness never
/// ships a palette of its own — `resolveTheme` remains the only authority.
const skin = mode === "light" ? "ice" : "aurora";
const preferences = {
  locale,
  skin,
  mode,
  density: "regular",
  motion: "reduced",
  diagnostics: [] as PreferenceDiagnostic[],
};

document.documentElement.lang = locale;
applyResolvedTheme(document.documentElement, resolveTheme(preferences));

const root = document.querySelector<HTMLElement>("#app");
if (!root) throw new Error("capture harness: missing #app");

/* ------------------------------------------------------------------ */
/* D1 cockpit projection                                               */
/* ------------------------------------------------------------------ */

/**
 * The shared D1 fixture with the deltas the cockpit states need.
 *
 * Base: `tests/support/d1_projection.ts` (the D1 fixture the cockpit,
 * statusbar and composer-control suites all mount).
 */
function d1Base(): D1CockpitProjection {
  const base = structuredClone(D1_PROJECTION);
  return {
    ...base,
    // Delta: the harness renders the requested locale/skin/mode. The cockpit
    // reads its locale from `preferences.locale`, so the URL parameter has to
    // land here and not only on the document element.
    preferences: { ...base.preferences, locale, skin, mode },
    contextDock: {
      ...base.contextDock,
      // Delta: a Core context budget, so the dock and the statusbar's
      // `context` segment agree. Mirrors the `ContextUsageProjection` shape
      // asserted in `tests/statusbar.spec.ts`.
      context: {
        budgetId: "budget-lane-core",
        usedTokens: 42_100,
        softTokenLimit: 96_000,
        hardTokenLimit: 128_000,
        remainingTokens: 85_900,
        exceeded: false,
      },
    },
    agentAdapters: [
      {
        ...base.agentAdapters[0]!,
        // Delta: adapter model options, so the model selector shows a second
        // group. Mirrors the adapter fixture in
        // `tests/composer_controls.spec.ts`.
        models: ["gpt-5.3-codex", "gpt-5.3-codex-mini"],
      },
    ],
    statusbar: {
      ...base.statusbar,
      // Delta: the three segments the shared fixture leaves empty, so all nine
      // statusbar segments carry a fact and the pending-gate chip renders.
      // Mirrors the populated statusbar fixture in `tests/statusbar.spec.ts`.
      context: { usedTokens: 42_100, hardTokenLimit: 128_000, exceeded: false },
      diagnosticsCount: 1,
      pendingGateCount: 2,
    },
  };
}

/**
 * A stopped ACP session with its two Core-backed recovery actions available.
 *
 * Mirrors the `STOPPED` fixture in `tests/d6_recovery.spec.ts`: `restart`
 * carries the session id Core published and `close_lane` the lane id, which is
 * what makes the buttons operable rather than inert.
 */
const D6_STOPPED: D6RecoveryProjection = {
  connection: "live",
  state: "agent_stopped",
  detail: "ACP session failed",
  hint: "Restart the agent session or close the Lane.",
  recoverable: true,
  businessSuccessBlocked: true,
  usedTokens: null,
  hardTokenLimit: null,
  missingCapabilities: [],
  actions: [
    { kind: "reconnect", available: false, code: "GUI-CORE-003" },
    { kind: "inspect", available: true, code: "presentation_only" },
    {
      kind: "restart",
      available: true,
      code: "core_command",
      sessionId: "session-lane-core",
      laneId: "lane-core",
    },
    { kind: "close_lane", available: true, code: "core_command", laneId: "lane-core" },
    { kind: "checkpoint", available: false, code: "GUI-CORE-003" },
  ],
};

/// Core's own words for a refused recovery action. Fixed, so `d6-error`
/// captures the same sentence every time.
const D6_REJECTION = "Core rejected restart: session-lane-core is no longer published.";

interface CockpitOptions {
  projection: D1CockpitProjection;
  /** Present so the rail's gear opens the Settings overlay. */
  preferencesAvailable?: boolean;
  /** Rejects instead of hanging, for the recovery-alert state. */
  d6Rejects?: boolean;
}

function mountCockpit(options: CockpitOptions): void {
  const { projection } = options;
  renderD1Cockpit(
    root!,
    projection,
    never<D1IntentResult>,
    never<D1IntentResult>,
    never<unknown>,
    never<D6RecoveryProjection>,
    {
      // No timers: the capture must not move while the operator frames it.
      poll: false,
      showWelcome: false,
      onNavigate: () => undefined,
      sendComposerControl: (_intent: ComposerControlIntent, _laneId: string | null) =>
        never<D1IntentResult>(),
      sendD6Intent: options.d6Rejects
        ? () => Promise.reject(new Error(D6_REJECTION))
        : () => never<D6IntentResult>(),
      preferences:
        options.preferencesAvailable === undefined
          ? undefined
          : {
              isAvailable: async () => options.preferencesAvailable === true,
              save: () => never<PreferenceIntentOutcome>(),
              restore: () => never<PreferenceIntentOutcome>(),
            },
    },
  );
}

/* ------------------------------------------------------------------ */
/* D11 intake projection                                               */
/* ------------------------------------------------------------------ */

/**
 * A probed project waiting on its config confirmation.
 *
 * Mirrors `EMPTY_PROJECTION` in `tests/d11_intake.spec.ts` plus that suite's
 * probed-project delta (`/workspace/demo`, rust pack, credential-locked
 * provider). D11 has no generated capture projection yet, so this is the one
 * screen whose projection is written out rather than loaded.
 */
const D11_PROJECTION: D11IntakeProjection = {
  project: {
    root: "/workspace/demo",
    isGitRepository: true,
    configState: "missing",
    projectName: "demo",
    mode: "rust",
    diagnostics: [],
  },
  preview: null,
  confirmedConfig: null,
  provider: {
    providerId: "deepseek",
    model: "deepseek-chat",
    status: "credential_locked",
    warning: true,
  },
  credentialHandles: [],
  starterLanes: [],
  pendingApproval: null,
  lastError: null,
  recentWork: {
    available: false,
    code: "GUI-CORE-007",
    message: "Recent project and session history is unavailable.",
  },
  credentialIngress: {
    available: false,
    code: "GUI-CORE-001",
    message: "Secure credential ingress is unavailable.",
  },
  capabilities: {
    projectOnboarding: true,
    credentialHandles: true,
    laneLifecycle: true,
  },
};

/* ------------------------------------------------------------------ */
/* D12 integration gate projection                                     */
/* ------------------------------------------------------------------ */

async function d12Blocked(): Promise<D12IntegrationGateProjection> {
  // Generated by `cargo test -p viden-gui --test capture_projections -- --ignored`
  // from the canonical `merge-gate.json` fixture. Accept is unavailable with
  // `missing_evidence` and reject with `no_actor`, which is exactly the
  // blocked state this capture has to show.
  const response = await fetch("../gui-screen-restore/projections/d12.json");
  return (await response.json()) as D12IntegrationGateProjection;
}

async function d12Decidable(): Promise<D12IntegrationGateProjection> {
  const blocked = await d12Blocked();
  const detail = blocked.detail;
  if (!detail) throw new Error("capture harness: generated d12 projection has no detail");
  // Delta: the gate with its required evidence recorded and an actor Core
  // published, so `d12_accept_block` and `d12_reject_block` both return
  // `None`. Mirrors the `DECIDABLE` fixture in
  // `tests/d12_integration_gate.spec.ts`.
  const gate = {
    ...detail.gate,
    evidenceIds: [...detail.gate.requiredEvidence],
    requiresIndependentValidator: false,
    hasValidator: true,
  };
  return {
    ...blocked,
    gates: [gate],
    detail: {
      ...detail,
      gate,
      missingEvidence: [],
      // Delta: a recorded check, so the action bar sits under a real checks
      // list. Mirrors the `checks` entry in the same spec fixture.
      checks: [{ id: "check-1", name: "replay-regression", status: "passed" }],
      actions: [
        { kind: "accept", available: true, code: null },
        { kind: "reject", available: true, code: null },
      ],
    },
  };
}

/// Types a bounce reason through the real input listener, so the rendered
/// enable/disable state of the bounce control is the production rule, not a
/// harness assertion.
function fillBounceReason(text: string): void {
  const input = document.querySelector<HTMLInputElement>("[data-d12-reason]");
  if (!input) return;
  input.value = text;
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

/* ------------------------------------------------------------------ */
/* States                                                              */
/* ------------------------------------------------------------------ */

async function renderState(): Promise<void> {
  switch (state) {
    case "d1": {
      mountCockpit({ projection: d1Base(), preferencesAvailable: true });
      return;
    }

    case "d1-mode-menu": {
      mountCockpit({ projection: d1Base(), preferencesAvailable: true });
      click("[data-control-toggle='work_mode']");
      await waitFor("[data-control-popover='work_mode']");
      return;
    }

    case "d1-model-menu": {
      mountCockpit({ projection: d1Base(), preferencesAvailable: true });
      click("[data-control-toggle='model']");
      await waitFor("[data-control-popover='model']");
      return;
    }

    case "settings": {
      mountCockpit({ projection: d1Base(), preferencesAvailable: true });
      click("[data-settings-toggle]");
      await waitFor("[data-settings-panel]");
      // A draft, so the panel captures the dirty state: Cancel and Save both
      // become enabled only once an axis actually changed.
      click("[data-settings-option='density:comfy']");
      await waitFor("[data-settings-panel] [data-settings-option='density:comfy'][aria-checked='true']");
      return;
    }

    case "settings-unavailable": {
      // Core's handshake without `ui.preference_persistence`: the panel opens
      // read-only and states the missing capability instead of hiding.
      mountCockpit({ projection: d1Base(), preferencesAvailable: false });
      click("[data-settings-toggle]");
      await waitFor("[data-settings-unavailable]");
      return;
    }

    case "d6-actions": {
      mountCockpit({
        projection: { ...d1Base(), recovery: D6_STOPPED },
        preferencesAvailable: true,
      });
      await waitFor("[data-d6-action='inspect']");
      // `inspect` is presentation-only: it expands facts the projection
      // already carries, so expanding it changes no Core state.
      click("[data-d6-action='inspect']");
      await waitFor("[data-d6-inspect]");
      return;
    }

    case "d6-error": {
      mountCockpit({
        projection: { ...d1Base(), recovery: D6_STOPPED },
        preferencesAvailable: true,
        d6Rejects: true,
      });
      await waitFor("[data-d6-action='restart']");
      click("[data-d6-action='restart']");
      await waitFor("[data-d6-error]");
      return;
    }

    case "d12-actions": {
      renderD12IntegrationGate(
        root!,
        await d12Decidable(),
        locale,
        () => undefined,
        never<D12IntentResult>,
      );
      fillBounceReason("Cancel window regressed; revalidate on the origin Lane.");
      return;
    }

    case "d12-blocked": {
      renderD12IntegrationGate(
        root!,
        await d12Blocked(),
        locale,
        () => undefined,
        never<D12IntentResult>,
      );
      return;
    }

    case "d11": {
      renderD11Intake(root!, D11_PROJECTION, never<D11IntentResult>, locale);
      return;
    }

    default: {
      root!.textContent = `unknown state ${state}`;
    }
  }
}

await renderState();
// The operator's capture signal: present only after the state has settled.
document.documentElement.dataset.captureReady = state;
