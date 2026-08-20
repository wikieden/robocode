import type { PermissionIntent, PermissionIntentResult } from "../components/permission_dock";
import type { D6Intent, D6IntentResult, D6RecoveryProjection } from "../models/workspace";
import type { ResolvedPreferences } from "../preferences";
import type { D1CockpitProjection, D1Intent, D1IntentResult } from "../screens/d1_cockpit";
import type { D10LaneMonitorProjection } from "../screens/d10_lane_monitor";
import type { D12IntegrationGateProjection } from "../screens/d12_integration_gate";
import type { D13FleetWorkflowProjection } from "../screens/d13_fleet_workflow";
import type { D14AuditTimelineProjection } from "../screens/d14_audit_timeline";
import type { D2DecisionsProjection, D2Intent, D2IntentResult } from "../screens/d2_decisions";
import type { D4Intent, D4IntentResult } from "../screens/d4_lane_create";

/**
 * The complete host boundary of the GUI shell. Every Core read, Core command,
 * host event subscription, and native chrome interaction the frontend needs
 * flows through this interface, so the screens and the shell stay ignorant of
 * which desktop host (Tauri today, any other later) carries the transport.
 *
 * Methods mirror the Core-owned projection/intent contract one-to-one; the
 * interface must not grow GUI-private state or reducers.
 */
export interface CoreClient {
  resolvedPreferences(): Promise<ResolvedPreferences | null>;

  d1Cockpit(selectedLaneId: string | null): Promise<D1CockpitProjection | null>;
  d1SendIntent(commandId: string, intent: D1Intent): Promise<D1IntentResult>;
  d1Poll(selectedLaneId: string | null, waitForEvent: boolean): Promise<D1IntentResult>;

  /**
   * Composer controls. Each sends its exact Core command and resolves with
   * the refreshed D1 result, whose projection carries the mode, permission,
   * provider/model, and model options Core now publishes — including coupled
   * changes Core made that the click never asked for.
   */
  setWorkMode(
    commandId: string,
    mode: string,
    selectedLaneId: string | null,
  ): Promise<D1IntentResult>;
  setPermissionLevel(
    commandId: string,
    level: string,
    selectedLaneId: string | null,
  ): Promise<D1IntentResult>;
  selectModel(
    commandId: string,
    providerId: string,
    model: string,
    selectedLaneId: string | null,
  ): Promise<D1IntentResult>;

  d4SendIntent(commandId: string, intent: D4Intent): Promise<D4IntentResult>;
  d4Poll(): Promise<D4IntentResult>;

  d2Decisions(selectedId: string | null): Promise<D2DecisionsProjection | null>;
  d2SendIntent(commandId: string, intent: D2Intent): Promise<D2IntentResult>;

  d10LaneMonitor(): Promise<D10LaneMonitorProjection | null>;
  d12IntegrationGate(selectedGateId: string | null): Promise<D12IntegrationGateProjection | null>;
  d13FleetWorkflow(): Promise<D13FleetWorkflowProjection | null>;
  d14AuditTimeline(after: string | null, limit: number): Promise<D14AuditTimelineProjection>;

  permissionSendIntent(
    commandId: string,
    intent: PermissionIntent,
  ): Promise<PermissionIntentResult>;
  d6Recover(): Promise<D6RecoveryProjection>;
  /** Sends one D6 recovery action as its Core command and re-reads recovery. */
  d6SendIntent(commandId: string, intent: D6Intent): Promise<D6IntentResult>;

  /** Resolve Core-persisted content to an inline data URL the webview can show. */
  agentContent(reference: string): Promise<string>;

  openWorkspace(root: string): Promise<void>;
  /** Native folder picker; resolves null when the user cancels. */
  pickProjectFolder(title: string): Promise<string | null>;

  /**
   * Subscribe to the host's "Core advanced" wake. Undefined when the host has
   * no push bridge (e.g. a browser harness); the cockpit then keeps its
   * bounded long poll. The returned function stops the handler from firing.
   */
  onCoreWake: ((handler: () => void) => () => void) | undefined;
}
