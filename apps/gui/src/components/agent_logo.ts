export type AgentLogoKind = "viden" | "codex" | "claude" | "kiro";

// These static assets mirror the registered brand marks in the GUI design package.
const LOGO_URLS: Record<AgentLogoKind, string> = {
  viden: new URL("../assets/agents/viden.svg", import.meta.url).href,
  codex: new URL("../assets/agents/codex.svg", import.meta.url).href,
  claude: new URL("../assets/agents/claude.svg", import.meta.url).href,
  kiro: new URL("../assets/agents/kiro.svg", import.meta.url).href,
};

export function resolveAgentLogoKind(agentId: string, displayName: string): AgentLogoKind | null {
  const identity = `${agentId} ${displayName}`.toLowerCase();
  if (identity.includes("codex")) return "codex";
  if (identity.includes("claude")) return "claude";
  if (identity.includes("kiro")) return "kiro";
  return null;
}

export function createAgentLogo(kind: AgentLogoKind): HTMLImageElement {
  const logo = document.createElement("img");
  logo.className = "agent-menu-logo";
  logo.dataset.agentLogo = kind;
  logo.src = LOGO_URLS[kind];
  logo.alt = "";
  logo.setAttribute("aria-hidden", "true");
  logo.draggable = false;
  return logo;
}
