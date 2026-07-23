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
      branch: string;
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
  | { type: "send_agent_session_input"; sessionId: string; content: string }
  | { type: "retry_agent_session"; sessionId: string }
  | { type: "cancel_agent_session"; sessionId: string };
