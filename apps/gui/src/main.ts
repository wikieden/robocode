import type { CoreClient } from "./host/core_client";
import { createTauriCoreClient } from "./host/tauri_core_client";
import { translate } from "./i18n/catalog";
import type { ComposerControlIntent } from "./models/composer";
import type { PermissionIntent, PermissionIntentResult } from "./components/permission_dock";
import type { D6Intent, D6RecoveryProjection } from "./models/workspace";
import {
  requestPreferenceRestore,
  requestPreferenceSave,
  type PreferenceIntentOutcome,
  type PreferenceState,
  type ResolvedPreferences,
} from "./preferences";
import {
  renderD1Cockpit,
  type D1Controller,
  type D1CockpitProjection,
  type D1Intent,
  type D1IntentResult,
} from "./screens/d1_cockpit";
import {
  renderD10LaneMonitor,
  type D10LaneMonitorProjection,
} from "./screens/d10_lane_monitor";
import {
  draftFromD11Projection,
  pendingFromD11Result,
  renderD11Intake,
  type D11Intent,
} from "./screens/d11_intake";
import {
  renderD12IntegrationGate,
  type D12IntegrationGateProjection,
  type D12Intent,
} from "./screens/d12_integration_gate";
import {
  renderD13FleetWorkflow,
  type D13FleetWorkflowProjection,
} from "./screens/d13_fleet_workflow";
import {
  renderD14AuditTimeline,
  type D14AuditTimelineProjection,
} from "./screens/d14_audit_timeline";
import {
  renderD2Decisions,
  type D2DecisionsProjection,
  type D2Intent,
  type D2IntentResult,
} from "./screens/d2_decisions";
import {
  renderD4LaneCreate,
  type D4Intent,
  type D4IntentResult,
  type D4StarterSeed,
} from "./screens/d4_lane_create";
import { applyResolvedTheme, resolveTheme } from "./ui/theme";
import "./ui/theme.css";
import "./ui/tokens.css";
import "./ui/window_chrome.css";

type ShellState = "connecting" | "disconnected" | "empty";

function shellProjection(
  preferences: ResolvedPreferences | undefined,
  shellState: ShellState,
): D1CockpitProjection {
  const locale = preferences?.locale ?? "en";
  const connectionState = shellState === "empty" ? "live" : shellState;
  return {
    preferences: preferences ?? {
      locale,
      skin: "aurora",
      mode: "dark",
      density: "regular",
      motion: "system",
      diagnostics: [],
    },
    selectedLaneId: null,
    // The shell has no confirmed view, so it has no workspace source either:
    // the titlebar's git block stays absent rather than showing a clean tree.
    topbarSource: null,
    contextDock: {
      source: null,
      context: null,
      laneAgent: null,
      provider: null,
      services: [],
      checklist: [],
    },
    lanes: [],
    environment: {
      cwd: "—",
      providerId: "—",
      model: "—",
      workMode: "—",
      permissionLevel: "—",
      tokenTotal: 0,
      costMicroUsd: null,
    },
    liveWork: {
      tasks: [],
      tools: [],
      approvals: [],
      queuedInputs: [],
      evidence: [],
    },
    transcript: [],
    workspaceEligibility: null,
    starterLanePreviews: [],
    agentAdapters: [],
    agentSessions: [],
    composer: {
      editable: false,
      busy: false,
      canCancel: false,
      canSubmitImmediately: false,
    },
    // Presentation-safe placeholders only: before Core is live there is no
    // fact to show, so every segment renders its explicit absent state.
    statusbar: {
      workMode: "—",
      permissionLevel: "—",
      context: null,
      eventStreamPosition: 0,
      lane: null,
      latency: null,
      tokens: null,
      diagnosticsCount: 0,
      requests: null,
      pendingGateCount: 0,
    },
    permissionDock: { workMode: "—", permissionLevel: "—", request: null },
    recovery: {
      connection: connectionState,
      state: shellState,
      detail:
        shellState === "empty"
          ? null
          : translate(
              locale,
              shellState === "connecting" ? "connection.pending" : "connection.unavailable",
              {},
            ),
      hint: null,
      recoverable: false,
      businessSuccessBlocked: shellState !== "empty",
      usedTokens: null,
      hardTokenLimit: null,
      missingCapabilities: [],
      actions: [],
    },
    unavailableFeatures: [],
  };
}

export function bootstrapShell(
  root: HTMLElement,
  preferences?: ResolvedPreferences,
  connectionState: "connecting" | "disconnected" = "connecting",
): void {
  const locale = preferences?.locale ?? "en";
  if (preferences) {
    document.documentElement.lang = locale;
    applyResolvedTheme(document.documentElement, resolveTheme(preferences));
  }

  // The cockpit is the stable application shell. Before Core is live, only
  // presentation-safe placeholders and the typed D6 connection state render.
  const projection = shellProjection(preferences, connectionState);
  root.dataset.clientState = connectionState;
  root.dataset.route = "d1";
  renderD1Cockpit(
    root,
    projection,
    async () => {
      throw new Error(translate(locale, "connection.pending", {}));
    },
    async () => ({
      projection,
      pendingCommandId: null,
      outcome: { state: "idle", reason: null },
    }),
    undefined,
    undefined,
    { poll: false },
  );
}

export async function hydrateShellFromCore(
  root: HTMLElement,
  core: CoreClient = createTauriCoreClient(),
): Promise<void> {
  let resolvedPreferences: ResolvedPreferences | undefined;
  try {
    const preferences = await core.resolvedPreferences();
    resolvedPreferences = preferences ?? undefined;
    if (preferences) {
      bootstrapShell(root, preferences);
    }
    const locale = preferences?.locale ?? "en";
      const sendD4 = async (intent: D4Intent) =>
        await core.d4SendIntent(`gui-d4-${crypto.randomUUID()}`, intent);
      const pollD4 = async () => await core.d4Poll();
      const sendD11 = async (intent: D11Intent) =>
        await core.d11SendIntent(`gui-d11-${crypto.randomUUID()}`, intent);
      const pollD11 = async () => await core.d11Poll();
      const pollD1 = async (laneId?: string, waitForEvent = false) =>
        await core.d1Poll(laneId ?? null, waitForEvent);
      const sendD1 = async (intent: D1Intent) =>
        await core.d1SendIntent(`gui-d1-${crypto.randomUUID()}`, intent);
      const sendPermission = async (intent: PermissionIntent) =>
        await core.permissionSendIntent(`gui-permission-${crypto.randomUUID()}`, intent);
      const recoverD6 = async () => await core.d6Recover();
      const sendD6Intent = async (intent: D6Intent) =>
        await core.d6SendIntent(`gui-d6-${crypto.randomUUID()}`, intent);
      const sendComposerControl = async (
        intent: ComposerControlIntent,
        selectedLaneId: string | null,
      ) => {
        const commandId = `gui-d1-${crypto.randomUUID()}`;
        if (intent.type === "set_work_mode") {
          return await core.setWorkMode(commandId, intent.mode, selectedLaneId);
        }
        if (intent.type === "set_permission_level") {
          return await core.setPermissionLevel(commandId, intent.level, selectedLaneId);
        }
        return await core.selectModel(commandId, intent.providerId, intent.model, selectedLaneId);
      };
      // The Core-owned preference loop. Only a confirmed Core result reaches
      // the live document: the theme and the `lang` attribute follow the
      // resolution Core published, never the operator's unsaved draft.
      const applyConfirmedPreferences = (
        outcome: PreferenceIntentOutcome,
      ): PreferenceIntentOutcome => {
        if (outcome.status === "confirmed") {
          document.documentElement.lang = outcome.resolved.locale;
          applyResolvedTheme(document.documentElement, resolveTheme(outcome.resolved));
        }
        return outcome;
      };
      const preferencePort = {
        isAvailable: async () => await core.preferencesAvailable(),
        save: async (state: PreferenceState) =>
          applyConfirmedPreferences(
            await requestPreferenceSave(state, core, `gui-pref-${crypto.randomUUID()}`),
          ),
        restore: async () =>
          applyConfirmedPreferences(
            await requestPreferenceRestore(core, `gui-pref-${crypto.randomUUID()}`),
          ),
      };
      let activeD1: D1Controller | null = null;
      const onCoreWake = core.onCoreWake;

      // Content Core persisted lives in the workspace, which the webview
      // cannot open. The host reads it and returns an inline data URL.
      const resolveContent = async (reference: string) => await core.agentContent(reference);

      const showD1 = async (laneId?: string) => {
        const projection = await core.d1Cockpit(laneId ?? null);
        if (!projection || (laneId && projection.selectedLaneId !== laneId)) {
          throw new Error(
            laneId
              ? `Core did not confirm D1 Lane ${laneId}`
              : "Core did not provide the D1 cockpit projection",
          );
        }
        activeD1?.dispose();
        root.dataset.route = "d1";
        root.dataset.clientState = "connected";
        document.documentElement.lang = projection.preferences.locale;
        const currentPreferences = await core.resolvedPreferences();
        if (currentPreferences) {
          applyResolvedTheme(document.documentElement, resolveTheme(currentPreferences));
        }
        if (projection.selectedLaneId) {
          root.dataset.focusLaneId = projection.selectedLaneId;
        } else {
          delete root.dataset.focusLaneId;
        }
        activeD1 = renderD1Cockpit(
          root,
          projection,
          sendD1,
          pollD1,
          sendPermission,
          recoverD6,
          {
            // "Full setup" is the designed D11 intake flow, not the D4 Lane
            // form: D4 creates one Lane, D11 walks project probe, config
            // confirmation, and the starter Lanes it then hands to D4.
            onFullSetup: () => void showD11(),
            onCoreWake,
            resolveContent,
            sendD6Intent,
            sendComposerControl,
            preferences: preferencePort,
            onNavigate: (route: string) => {
              // Every restored screen re-reads its own Core projection before
              // it renders; the rail only names the route.
              if (route === "d2") void showD2();
              else if (route === "d10") void showD10();
              else if (route === "d12") void showD12();
              else if (route === "d13") void showD13();
              else if (route === "d14") void showD14();
            },
          },
        );
      };

      // D11 is the full project intake and first-run setup flow. It is entered
      // from the agent menu's full-setup action and from `?screen=d11`; the
      // Welcome "Open project" path stays the compact native folder flow.
      //
      // Core publishes no first-run signal (the D11 projection carries only
      // probe/preview/confirmed-config facts), so the shell never redirects
      // into D11 on its own.
      const showD11 = async () => {
        activeD1?.dispose();
        activeD1 = null;
        // `d11_poll` is the entry read as well as the wait: it drains the
        // ordered events already queued and reports any command still awaiting
        // its Core receipt, so the screen resumes instead of restarting.
        const result = await pollD11();
        root.dataset.route = "d11";
        renderD11Intake(
          root,
          result.projection,
          sendD11,
          locale,
          pollD11,
          {
            draft: draftFromD11Projection(result.projection),
            pending: pendingFromD11Result(result),
          },
          // Starter Lanes are created by D4, which owns the preview/confirm
          // receipt loop; D11 only hands over the seeds the operator picked.
          (queue) => void showD4([...queue]),
          undefined,
          () => void showD1(),
        );
      };

      const showD4 = async (queue: D4StarterSeed[] = []) => {
        activeD1?.dispose();
        activeD1 = null;
        const d4Result = await pollD4();
        root.dataset.route = "d4";
        renderD4LaneCreate(root, d4Result, sendD4, pollD4, locale, {
          queue,
          queueIndex: 0,
          completedLaneIds: [],
          onCancel: () => void showD1(),
          onNavigateToD1: (laneId) => {
            // D4 supplies only the exact Core receipt Lane id; D1 then
            // re-reads the canonical view before it renders or sends.
            void showD1(laneId);
          },
        });
      };

      // D2 is the cross-Lane decision queue. It reads the same Core facts as
      // the D1 permission dock, so it never keeps a second decision model.
      const showD2 = async () => {
        activeD1?.dispose();
        activeD1 = null;
        const projection = await core.d2Decisions(null);
        if (!projection) {
          throw new Error("Core did not provide the D2 decision projection");
        }
        root.dataset.route = "d2";
        renderD2Decisions(
          root,
          projection,
          async (intent: D2Intent) =>
            await core.d2SendIntent(`gui-d2-${crypto.randomUUID()}`, intent),
          locale,
        );
      };

      // D10 watches every Lane across projects. It is read-only: every
      // actionable decision routes back into D2.
      const showD10 = async () => {
        activeD1?.dispose();
        activeD1 = null;
        const projection = await core.d10LaneMonitor();
        if (!projection) {
          throw new Error("Core did not provide the D10 lane monitor projection");
        }
        root.dataset.route = "d10";
        renderD10LaneMonitor(root, projection, locale, () => void showD2());
      };

      // D12 is the integration-gate failure path. Accept opens only when Core
      // says every required evidence id is present.
      const showD12 = async (gateId?: string) => {
        activeD1?.dispose();
        activeD1 = null;
        const projection = await core.d12IntegrationGate(gateId ?? null);
        if (!projection) {
          throw new Error("Core did not provide the D12 integration gate projection");
        }
        root.dataset.route = "d12";
        renderD12IntegrationGate(
          root,
          projection,
          locale,
          (next) => void showD12(next),
          async (intent: D12Intent) =>
            await core.d12SendIntent(`gui-d12-${crypto.randomUUID()}`, intent),
        );
      };

      // D14 is the audit trail. It pages the Core replay cursor and never
      // reconstructs history from the current view state.
      const showD14 = async () => {
        activeD1?.dispose();
        activeD1 = null;
        const page = async (after: string | null) => await core.d14AuditTimeline(after, 200);
        const projection = await page(null);
        root.dataset.route = "d14";
        renderD14AuditTimeline(root, projection, locale, (after) => page(after));
      };

      // D13 is the fleet board. It is read-only: workflow mutations stay with
      // the Core commands that own the DAG.
      const showD13 = async () => {
        activeD1?.dispose();
        activeD1 = null;
        const projection = await core.d13FleetWorkflow();
        if (!projection) {
          throw new Error("Core did not provide the D13 fleet projection");
        }
        root.dataset.route = "d13";
        renderD13FleetWorkflow(root, projection, locale);
      };

      const openProject = async () => {
        const selected = await core.pickProjectFolder(
          translate(locale, "d1.welcome.openFolderTitle", {}),
        );
        if (selected === null) return;
        await core.openWorkspace(selected);
        await showD1();
      };

      const initialProjection = await core.d1Cockpit(null);
      if (initialProjection) {
        // D4 remains an explicit compatibility surface; normal project entry
        // and the D1 `+` action use the compact native/ACP menu.
        const screen = new URLSearchParams(window.location.search).get("screen");
        if (screen === "d4") {
          await showD4();
        } else if (screen === "d11") {
          await showD11();
        } else if (screen === "d2") {
          await showD2();
        } else if (screen === "d10") {
          await showD10();
        } else if (screen === "d12") {
          await showD12();
        } else if (screen === "d13") {
          await showD13();
        } else if (screen === "d14") {
          await showD14();
        } else {
          await showD1();
        }
      } else {
        root.dataset.clientState = "empty";
        root.dataset.route = "d1";
        const projection = shellProjection(resolvedPreferences, "empty");
        activeD1 = renderD1Cockpit(
          root,
          projection,
          async () => {
            throw new Error("No project is open");
          },
          async () => ({
            projection,
            pendingCommandId: null,
            outcome: { state: "idle", reason: null },
          }),
          undefined,
          undefined,
          {
            onOpenProject: openProject,
            showWelcome: true,
            poll: false,
            // A host is bound even without a project, so the gear opens the
            // panel and states what Core has not published rather than
            // disappearing.
            preferences: preferencePort,
          },
        );
      }
  } catch {
    // Keep the D1 shell visible and make the failed Core bootstrap explicit.
    bootstrapShell(root, resolvedPreferences, "disconnected");
  }
}

const root = document.querySelector<HTMLElement>("#app");
if (root) {
  bootstrapShell(root);
  void hydrateShellFromCore(root);
}
