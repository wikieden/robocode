import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import { translate } from "./i18n/catalog";
import type { PermissionIntent, PermissionIntentResult } from "./components/permission_dock";
import type { D6RecoveryProjection } from "./models/workspace";
import type { ResolvedPreferences } from "./preferences";
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
  renderD12IntegrationGate,
  type D12IntegrationGateProjection,
} from "./screens/d12_integration_gate";
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

export async function hydrateShellFromCore(root: HTMLElement): Promise<void> {
  let resolvedPreferences: ResolvedPreferences | undefined;
  try {
    const preferences = await invoke<ResolvedPreferences | null>("resolved_preferences");
    resolvedPreferences = preferences ?? undefined;
    if (preferences) {
      bootstrapShell(root, preferences);
    }
    const locale = preferences?.locale ?? "en";
      const sendD4 = async (intent: D4Intent) =>
        await invoke<D4IntentResult>("d4_send_intent", {
          commandId: `gui-d4-${crypto.randomUUID()}`,
          intent,
        });
      const pollD4 = async () => await invoke<D4IntentResult>("d4_poll");
      const pollD1 = async (laneId?: string, waitForEvent = false) =>
        await invoke<D1IntentResult>("d1_poll", {
          selectedLaneId: laneId ?? null,
          waitForEvent,
        });
      const sendD1 = async (intent: D1Intent) => {
        return await invoke<D1IntentResult>("d1_send_intent", {
          commandId: `gui-d1-${crypto.randomUUID()}`,
          intent,
        });
      };
      const sendPermission = async (intent: PermissionIntent) =>
        await invoke<PermissionIntentResult>("permission_send_intent", {
          commandId: `gui-permission-${crypto.randomUUID()}`,
          intent,
        });
      const recoverD6 = async () =>
        await invoke<D6RecoveryProjection>("d6_recover");
      let activeD1: D1Controller | null = null;

      const showD1 = async (laneId?: string) => {
        const projection = await invoke<D1CockpitProjection | null>("d1_cockpit", {
          selectedLaneId: laneId ?? null,
        });
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
        const currentPreferences = await invoke<ResolvedPreferences | null>("resolved_preferences");
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
          { onFullSetup: () => void showD4() },
        );
      };

      const showD4 = async () => {
        activeD1?.dispose();
        activeD1 = null;
        const d4Result = await pollD4();
        root.dataset.route = "d4";
        renderD4LaneCreate(root, d4Result, sendD4, pollD4, locale, {
          queue: [],
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
        const projection = await invoke<D2DecisionsProjection | null>("d2_decisions", {
          selectedId: null,
        });
        if (!projection) {
          throw new Error("Core did not provide the D2 decision projection");
        }
        root.dataset.route = "d2";
        renderD2Decisions(
          root,
          projection,
          async (intent: D2Intent) =>
            await invoke<D2IntentResult>("d2_send_intent", {
              commandId: `gui-d2-${crypto.randomUUID()}`,
              intent,
            }),
          locale,
        );
      };

      // D10 watches every Lane across projects. It is read-only: every
      // actionable decision routes back into D2.
      const showD10 = async () => {
        activeD1?.dispose();
        activeD1 = null;
        const projection = await invoke<D10LaneMonitorProjection | null>("d10_lane_monitor");
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
        const projection = await invoke<D12IntegrationGateProjection | null>(
          "d12_integration_gate",
          { selectedGateId: gateId ?? null },
        );
        if (!projection) {
          throw new Error("Core did not provide the D12 integration gate projection");
        }
        root.dataset.route = "d12";
        renderD12IntegrationGate(root, projection, locale, (next) => void showD12(next));
      };

      const openProject = async () => {
        const selected = await open({
          directory: true,
          multiple: false,
          title: translate(locale, "d1.welcome.openFolderTitle", {}),
        });
        if (typeof selected !== "string") return;
        await invoke<void>("open_workspace", { root: selected });
        await showD1();
      };

      const initialProjection = await invoke<D1CockpitProjection | null>("d1_cockpit", {
        selectedLaneId: null,
      });
      if (initialProjection) {
        // D4 remains an explicit compatibility surface; normal project entry
        // and the D1 `+` action use the compact native/ACP menu.
        const screen = new URLSearchParams(window.location.search).get("screen");
        if (screen === "d4") {
          await showD4();
        } else if (screen === "d2") {
          await showD2();
        } else if (screen === "d10") {
          await showD10();
        } else if (screen === "d12") {
          await showD12();
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
          { onOpenProject: openProject, showWelcome: true, poll: false },
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
