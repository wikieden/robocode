import type { Locale } from "../i18n/catalog";
import type { D1TranscriptRow } from "./transcript";

export type PermissionChoice =
  | "once"
  | "session"
  | "repo_allowlist"
  | "always"
  | "edit"
  | "deny";

export interface PermissionDockProjection {
  workMode: string;
  permissionLevel: string;
  request: {
    id: string;
    toolName: string;
    title: string;
    message: string;
    inputPreview: string;
    isMutating: boolean;
    reason: string | null;
    risk: string;
    target: { kind: string; display: string; canonicalRef: string | null };
    policyReasonKey: string;
    policyReasonArgs: Record<string, string>;
    expiresAt: number;
    defaultAction: string;
    auditId: string;
    blockedByPlan: boolean;
    actions: Array<{
      kind: PermissionChoice;
      available: boolean;
      sessionId: string | null;
      paths: string[];
      code: string | null;
    }>;
  } | null;
}

export type D6State =
  | "live"
  | "empty"
  | "connecting"
  | "disconnected"
  | "provider_error"
  | "agent_stopped"
  | "context_overflow"
  | "gate_queue_clear"
  | "incompatible_schema"
  | "missing_feature_capability"
  | "event_gap";

export interface D6RecoveryProjection {
  connection: "disconnected" | "connecting" | "live" | "recovering" | "incompatible";
  state: D6State;
  detail: string | null;
  hint: string | null;
  recoverable: boolean;
  businessSuccessBlocked: boolean;
  usedTokens: number | null;
  hardTokenLimit: number | null;
  missingCapabilities: string[];
  actions: Array<{ kind: string; available: boolean; code: string }>;
}

export interface WorkspaceSourceProjection {
  status: "ready" | "unavailable" | "truncated";
  branch: string | null;
  worktree: string | null;
  ahead: number;
  behind: number;
  added: number;
  deleted: number;
  dirty: boolean;
}

export interface ContextUsageProjection {
  budgetId: string;
  usedTokens: number;
  softTokenLimit: number;
  hardTokenLimit: number;
  remainingTokens: number;
  exceeded: boolean;
}

export interface LaneAgentProjection {
  laneId: string;
  workspaceId: string;
  projectId: string;
  sessionId: string | null;
  taskId: string | null;
  turnId: string | null;
}

export interface ProviderHealthProjection {
  providerId: string;
  model: string;
  status: string;
  requestCount: number;
  errorCount: number;
  lastLatencyMs: number | null;
  averageLatencyMs: number | null;
  tokensPerSecond: number | null;
}

export interface RuntimeServiceProjection {
  id: string;
  kind: "mcp" | "lsp";
  label: string;
  status: "connected" | "ready" | "degraded" | "offline" | "unavailable";
  detailKey: string | null;
}

export interface ChecklistItemProjection {
  id: string;
  kind: "workspace_change" | "check_run";
  label: string;
  status: string;
  command: string | null;
  path: string | null;
  summary: string | null;
  /** Typed WorkspaceChange patch; absent facts must render as unavailable. */
  patch?: string | null;
  failingLocation: string | null;
  additions: number | null;
  deletions: number | null;
}

export interface ContextDockProjection {
  source: WorkspaceSourceProjection | null;
  context: ContextUsageProjection | null;
  laneAgent: LaneAgentProjection | null;
  provider: ProviderHealthProjection | null;
  services: RuntimeServiceProjection[];
  checklist: ChecklistItemProjection[];
}

export interface D1CockpitProjection {
  preferences: {
    locale: Locale;
    skin: string;
    mode: string;
    density: string;
    motion: string;
    diagnostics: unknown[];
  };
  selectedLaneId: string | null;
  contextDock: ContextDockProjection;
  lanes: Array<{
    id: string;
    role: string;
    status: string;
    summary: string;
    branch: string | null;
  }>;
  environment: {
    cwd: string;
    providerId: string;
    model: string;
    workMode: string;
    permissionLevel: string;
    tokenTotal: number;
    costMicroUsd: number | null;
  };
  liveWork: {
    tasks: Array<{ id: string; title: string; status: string; progress: number }>;
    tools: Array<{ id: string; name: string; inputPreview: string; state: string }>;
    approvals: Array<{ id: string; title: string; risk: string }>;
    queuedInputs: Array<{ id: string; contentPreview: string }>;
    evidence: Array<{ id: string; kind: string; summary: string; path: string | null }>;
  };
  transcript: D1TranscriptRow[];
  workspaceEligibility: {
    isGitRepository: boolean;
    hasHead: boolean;
    canCreateLane: boolean;
    diagnostic: string | null;
  } | null;
  starterLanePreviews: Array<{
    previewId: string;
    contentSha256: string;
    laneId: string;
    branch: string | null;
    diagnostics: string[];
  }>;
  agentAdapters: Array<{
    agentId: string;
    displayName: string;
    startability: string;
    diagnostics: string[];
  }>;
  agentSessions: Array<{
    sessionId: string;
    laneId: string;
    agentId: string;
    model: string | null;
    status: string;
    task: string;
    diagnostic: string | null;
    output?: string | null;
    conversation?: Array<{
      messageId: string;
      role: "user" | "assistant";
      content: string;
      /// Typed content Core published with the message. Absent when Core
      /// published none; the client never synthesizes a part.
      parts?: {
        kind: string;
        mediaType: string | null;
        reference: string | null;
        text: string | null;
        label: string | null;
      }[];
    }>;
  }>;
  composer: {
    editable: boolean;
    busy: boolean;
    canCancel: boolean;
    canSubmitImmediately: boolean;
  };
  permissionDock: PermissionDockProjection;
  recovery: D6RecoveryProjection;
  unavailableFeatures: Array<{
    id: string;
    available: false;
    code: string;
    message: string;
  }>;
}
