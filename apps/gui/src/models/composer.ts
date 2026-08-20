export type D1Intent =
  | { type: "submit"; laneId: string; content: string }
  | { type: "cancel"; laneId: string }
  | { type: "query_agent_adapters" }
  | { type: "probe_agent_adapter"; agentId: string }
  | { type: "preview_default_lane"; preset: "coder" }
  | {
      type: "create_starter_lane";
      laneId: string;
      preset: "coder";
      branch: string | null;
      previewId: string;
      contentSha256: string;
    }
  | {
      type: "start_agent_session";
      laneId: string;
      agentId: string;
      model: string | null;
      task: string;
    }
  | { type: "send_agent_session_input"; laneId: string; sessionId: string; content: string }
  | { type: "retry_agent_session"; laneId: string; sessionId: string }
  | { type: "cancel_agent_session"; laneId: string; sessionId: string };

/**
 * One composer-control mutation (work mode, permission level, model). Values
 * are the CLI names Core itself publishes in the snapshot. The GUI never
 * applies Core's mode/permission coupling rule locally: it dispatches the
 * control and re-renders from the snapshot Core republishes.
 */
export type ComposerControlIntent =
  | { type: "set_work_mode"; mode: string }
  | { type: "set_permission_level"; level: string }
  | { type: "select_model"; providerId: string; model: string };
