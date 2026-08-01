import type { Locale } from "../i18n/catalog";
import "./d10_lane_monitor.css";

/// D10 Lane monitor.
///
/// One card per Core Lane across every project. Gate strength, status, project
/// binding, and progress are published Core facts; the screen renders them and
/// declares what the contract does not carry.

export interface D10Unavailable {
  key: string;
  code: string;
}

export interface D10Agent {
  sessionId: string;
  agentId: string;
  model: string | null;
  status: string;
}

export interface D10Evidence {
  id: string;
  kind: string;
  summary: string;
}

export interface D10Lane {
  id: string;
  projectId: string | null;
  summary: string;
  role: string;
  route: string;
  gateStrength: string;
  mutationPolicy: string;
  status: string;
  awaitsHuman: boolean;
  branch: string | null;
  worktree: string | null;
  progress: number | null;
  agents: D10Agent[];
  evidence: D10Evidence[];
  tokenLimit: number | null;
  costLimitMicroUsd: number | null;
}

export interface D10LaneMonitorProjection {
  totalLanes: number;
  totalProjects: number;
  awaitingTotal: number;
  lanes: D10Lane[];
  unavailable: D10Unavailable[];
}

export interface D10Controller {
  applyProjection: (projection: D10LaneMonitorProjection) => void;
}

type Copy = Record<string, string>;

/// Gate strength glyphs are the registered design marks for the three Core
/// values, not decoration: they say how far the lane's output can be trusted.
const GATE_GLYPH: Record<string, string> = {
  full: "●",
  cooperative: "◐",
  containment: "○",
};

const COPY: Record<Locale, Copy> = {
  en: {
    title: "Lane Monitor",
    lanes: "lanes",
    projects: "projects",
    awaiting: "awaiting you",
    all: "All",
    unbound: "no project binding",
    noProgress: "no Core task",
    noLanes: "Core published no Lane.",
    decide: "Open Decision Center",
    gate_full: "full gate · per-call interception",
    gate_cooperative: "cooperative · advisory plus worktree fence",
    gate_containment: "containment · worktree fence and exit diff only",
    "d10.events.noOrderedLog":
      "Core publishes no ordered event log in the view state, so the event stream is unavailable.",
  },
  "zh-CN": {
    title: "Lane 监视器",
    lanes: "条 lane",
    projects: "个项目",
    awaiting: "项等你",
    all: "全部",
    unbound: "无项目绑定",
    noProgress: "无 Core 任务",
    noLanes: "Core 未发布任何 Lane。",
    decide: "打开决策中心",
    gate_full: "强门控 · 逐调用拦截",
    gate_cooperative: "半合作 · 建议性 + worktree 兜底",
    gate_containment: "围栏兜底 · 仅 worktree + 退出 diff",
    "d10.events.noOrderedLog": "Core 在视图状态中不发布有序事件日志，事件流不可用。",
  },
};

function label(copy: Copy, key: string): string {
  return copy[key] ?? key;
}

export function renderD10LaneMonitor(
  root: HTMLElement,
  initial: D10LaneMonitorProjection,
  locale: Locale,
  openDecisionCenter?: () => void,
): D10Controller {
  let projection = initial;
  let filter = "all";
  const copy = COPY[locale];

  const laneCard = (lane: D10Lane): HTMLElement => {
    const card = document.createElement("article");
    card.className = "d10-card";
    card.dataset.d10Lane = lane.id;
    card.dataset.d10Status = lane.status;
    if (lane.awaitsHuman) card.dataset.d10Attention = "true";

    const head = document.createElement("div");
    head.className = "d10-chead";
    const id = document.createElement("span");
    id.className = "d10-lid";
    id.textContent = lane.id;
    const summary = document.createElement("span");
    summary.className = "d10-summary";
    summary.textContent = lane.summary;
    head.append(id, summary);

    const meta = document.createElement("div");
    meta.className = "d10-meta";
    const project = document.createElement("span");
    project.className = "d10-project";
    project.dataset.d10Project = lane.projectId ?? "";
    project.textContent = lane.projectId ?? copy.unbound;
    meta.append(project);
    for (const agent of lane.agents) {
      const chip = document.createElement("span");
      chip.className = "d10-chip";
      chip.textContent = agent.model ? `${agent.agentId} · ${agent.model}` : agent.agentId;
      meta.append(chip);
    }
    const gate = document.createElement("span");
    gate.className = "d10-gate";
    gate.dataset.d10Gate = lane.gateStrength;
    gate.title = label(copy, `gate_${lane.gateStrength}`);
    gate.textContent = `${GATE_GLYPH[lane.gateStrength] ?? "?"} ${lane.gateStrength}`;
    const status = document.createElement("span");
    status.className = "d10-status";
    status.textContent = lane.status;
    meta.append(gate, status);

    const progress = document.createElement("div");
    progress.className = "d10-progress";
    if (lane.progress === null) {
      progress.dataset.d10Progress = "none";
      progress.textContent = copy.noProgress;
    } else {
      progress.dataset.d10Progress = String(lane.progress);
      const fill = document.createElement("i");
      fill.style.width = `${lane.progress}%`;
      progress.append(fill);
    }

    card.append(head, meta, progress);

    for (const entry of lane.evidence) {
      const row = document.createElement("p");
      row.className = "d10-evidence";
      row.dataset.d10Evidence = entry.id;
      row.textContent = `${entry.kind} · ${entry.summary}`;
      card.append(row);
    }

    if (lane.awaitsHuman && openDecisionCenter) {
      const action = document.createElement("button");
      action.type = "button";
      action.className = "d10-action";
      action.dataset.d10Action = "decide";
      action.textContent = `${copy.decide} ↗`;
      action.addEventListener("click", openDecisionCenter);
      card.append(action);
    }
    return card;
  };

  const render = (): void => {
    const stage = document.createElement("section");
    stage.className = "d10-stage";
    stage.dataset.route = "d10";

    const bar = document.createElement("div");
    bar.className = "d10-head";
    const heading = document.createElement("h2");
    heading.className = "d10-title";
    heading.textContent = copy.title;
    const counts = document.createElement("span");
    counts.className = "d10-counts";
    counts.dataset.d10Counts = `${projection.totalLanes}/${projection.totalProjects}/${projection.awaitingTotal}`;
    counts.textContent = `${projection.totalLanes} ${copy.lanes} · ${projection.totalProjects} ${copy.projects} · ${projection.awaitingTotal} ${copy.awaiting}`;
    bar.append(heading, counts);

    const projects = [
      ...new Set(
        projection.lanes
          .map((lane) => lane.projectId)
          .filter((id): id is string => typeof id === "string"),
      ),
    ];
    const chips = document.createElement("div");
    chips.className = "d10-fchips";
    for (const key of ["all", ...projects]) {
      const chip = document.createElement("button");
      chip.type = "button";
      chip.className = "d10-fchip";
      chip.dataset.d10Filter = key;
      chip.setAttribute("aria-pressed", String(filter === key));
      chip.textContent = key === "all" ? copy.all : key;
      chip.addEventListener("click", () => {
        filter = key;
        render();
      });
      chips.append(chip);
    }
    bar.append(chips);
    stage.append(bar);

    const grid = document.createElement("div");
    grid.className = "d10-grid";
    const shown = projection.lanes.filter(
      (lane) => filter === "all" || lane.projectId === filter,
    );
    if (shown.length === 0) {
      const empty = document.createElement("p");
      empty.className = "d10-muted";
      empty.dataset.d10Empty = "true";
      empty.textContent = copy.noLanes;
      grid.append(empty);
    }
    for (const lane of shown) grid.append(laneCard(lane));
    stage.append(grid);

    for (const entry of projection.unavailable) {
      const note = document.createElement("p");
      note.className = "d10-unavailable";
      note.dataset.d10Unavailable = entry.code;
      note.textContent = `${label(copy, entry.key)} · ${entry.code}`;
      stage.append(note);
    }

    root.replaceChildren(stage);
  };

  render();
  return {
    applyProjection: (next) => {
      projection = next;
      render();
    },
  };
}
