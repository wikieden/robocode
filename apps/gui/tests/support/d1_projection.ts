import type { D1CockpitProjection } from "../../src/screens/d1_cockpit";

export const D1_PROJECTION: D1CockpitProjection = {
  preferences: {
    locale: "en",
    skin: "aurora",
    mode: "dark",
    density: "regular",
    motion: "reduced",
    diagnostics: [],
  },
  selectedLaneId: "lane-core",
  lanes: [
    {
      id: "lane-core",
      role: "coder",
      status: "running",
      summary: "Streaming cockpit",
      branch: "codex/lane-core",
    },
  ],
  environment: {
    cwd: "/workspace/viden",
    providerId: "deepseek",
    model: "deepseek-v4-flash",
    workMode: "build",
    permissionLevel: "ask",
    tokenTotal: 1440,
    costMicroUsd: 4200,
  },
  liveWork: {
    tasks: [{ id: "task-core", title: "Freeze contract", status: "running", progress: 40 }],
    tools: [{ id: "tool-cargo", name: "cargo", inputPreview: "cargo test", state: "running" }],
    approvals: [{ id: "approval-shell", title: "Allow test", risk: "high" }],
    queuedInputs: [{ id: "queued-1", contentPreview: "continue" }],
    evidence: [{ id: "evidence-1", kind: "test", summary: "Tests passed", path: null }],
  },
  transcript: [
    { id: "tool-tool-cargo", kind: "tool", content: "cargo · cargo test" },
    { id: "stream", kind: "assistant_stream", content: "Working…" },
  ],
  workspaceEligibility: {
    isGitRepository: true,
    hasHead: true,
    canCreateLane: true,
    diagnostic: null,
  },
  starterLanePreviews: [],
  agentAdapters: [
    {
      agentId: "codex-acp",
      displayName: "Codex",
      startability: "ready",
      diagnostics: [],
    },
  ],
  agentSessions: [],
  composer: { editable: true, busy: true, canCancel: true, canSubmitImmediately: false },
  permissionDock: { workMode: "build", permissionLevel: "ask", request: null },
  recovery: {
    connection: "live",
    state: "live",
    detail: null,
    hint: null,
    recoverable: false,
    businessSuccessBlocked: false,
    usedTokens: null,
    hardTokenLimit: null,
    missingCapabilities: [],
    actions: [],
  },
  unavailableFeatures: [
    { id: "diff", available: false, code: "GUI-CORE-006", message: "Diff is unavailable." },
    { id: "apply", available: false, code: "GUI-CORE-006", message: "Apply is unavailable." },
    { id: "audit", available: false, code: "GUI-CORE-004", message: "Audit is unavailable." },
    { id: "recovery", available: false, code: "GUI-CORE-003", message: "Recovery is unavailable." },
  ],
};
