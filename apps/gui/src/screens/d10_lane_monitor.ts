import type { Locale } from "../i18n/catalog";
import type { D14AuditRow } from "./d14_audit_timeline";
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

export interface D10RunStats {
  /** Humanized on the host, so every frontend reads the same duration. */
  wallTime: string;
  wallTimeMs: number;
  runCount: number;
  diffBytes: number;
  /** `null` is Core's own best-effort absence, rendered as unknown. */
  lastExitCode: number | null;
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
  /** `metered` or `blind`, from Core's `AgentRoute::cost_meterability`. */
  costMeterability: string;
  /** Bounded process facts for a cost-blind lane; `null` when Core observed
   * no run. Absence is absence: it is never rendered as a measured zero. */
  runStats: D10RunStats | null;
}

export interface D10LaneMonitorProjection {
  totalLanes: number;
  totalProjects: number;
  awaitingTotal: number;
  lanes: D10Lane[];
  unavailable: D10Unavailable[];
}

/**
 * The D10 event ticker, read from the Core audit timeline (GUI-CORE-014).
 *
 * Shape-compatible with `D14AuditProjection`, because D10 and D14 are two views
 * of one Core timeline: `QueryAudit` -> `AuditPageLoaded`. One bounded
 * newest-first page over the whole workspace, ordered by Core across projects
 * (the `audit-ordering` fixture is the canonical proof). The screen never
 * rebuilds a timeline by diffing successive snapshots.
 *
 * `capabilityAvailable`, `loaded`, and an empty `rows` are three different
 * facts and each gets its own line.
 */
export interface D10Events {
  rows: D14AuditRow[];
  loaded: boolean;
  capabilityAvailable: boolean;
  outcome: { state: string; reason: string | null };
}

export interface D10Controller {
  applyProjection: (projection: D10LaneMonitorProjection) => void;
  applyEvents: (events: D10Events) => void;
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
    blind: "cost-blind route",
    blindHint:
      "Core sees no provider exchange on this route, so it publishes bounded run facts instead of a token or dollar figure.",
    runsUnobserved: "Core has observed no run on this lane yet.",
    wallTime: "wall time",
    runCount: "runs",
    diffBytes: "applied diff",
    exitCode: "last exit",
    exitCodeUnknown: "unknown",
    gate_full: "full gate · per-call interception",
    gate_cooperative: "cooperative · advisory plus worktree fence",
    gate_containment: "containment · worktree fence and exit diff only",
    events: "Event stream",
    eventsPending: "Reading the Core audit timeline\u2026",
    eventsEmpty: "Core published no audited event yet.",
    eventsUnavailable: "Core publishes no audit timeline, so the event stream is unavailable.",
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
    blind: "成本不可计量路由",
    blindHint: "Core 在该路由上看不到任何模型调用，因此只发布有界的运行事实，而不是 token 或金额。",
    runsUnobserved: "Core 尚未在该 Lane 上观测到任何运行。",
    wallTime: "累计耗时",
    runCount: "运行次数",
    diffBytes: "已应用 diff",
    exitCode: "最近退出码",
    exitCodeUnknown: "未知",
    gate_full: "强门控 · 逐调用拦截",
    gate_cooperative: "半合作 · 建议性 + worktree 兜底",
    gate_containment: "围栏兜底 · 仅 worktree + 退出 diff",
    events: "事件流",
    eventsPending: "正在读取 Core 审计时间线…",
    eventsEmpty: "Core 尚未发布任何审计事件。",
    eventsUnavailable: "Core 未发布审计时间线，事件流不可用。",
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
  initialEvents: D10Events | null = null,
): D10Controller {
  let projection = initial;
  let events = initialEvents;
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

    if (lane.costMeterability === "blind") {
      // The marker is a Core fact, not a warning badge: it says this lane's
      // cost is unobservable, which is why the row below carries process facts
      // instead of a token or dollar figure.
      const cost = document.createElement("div");
      cost.className = "d10-cost";
      cost.dataset.d10Meterability = "blind";
      const marker = document.createElement("span");
      marker.className = "d10-blind";
      marker.title = copy.blindHint;
      marker.textContent = copy.blind;
      cost.append(marker);

      if (lane.runStats) {
        const stats = lane.runStats;
        const facts: [string, string][] = [
          [copy.wallTime, stats.wallTime],
          [copy.runCount, String(stats.runCount)],
          [copy.diffBytes, `${stats.diffBytes} B`],
          [
            copy.exitCode,
            // A missing exit code is labelled, never defaulted to 0: Core
            // publishes `null` for a force-kill, a still-running process, or a
            // tmux session with no exit-code channel.
            stats.lastExitCode === null ? copy.exitCodeUnknown : String(stats.lastExitCode),
          ],
        ];
        for (const [name, value] of facts) {
          const fact = document.createElement("span");
          fact.className = "d10-runfact";
          fact.dataset.d10RunFact = name;
          fact.textContent = `${name} ${value}`;
          cost.append(fact);
        }
      } else {
        // Absence is absence. An unobserved lane must not be drawn as a
        // measured zero, which is a different Core fact.
        const none = document.createElement("span");
        none.className = "d10-runfact d10-muted";
        none.dataset.d10RunStats = "none";
        none.textContent = copy.runsUnobserved;
        cost.append(none);
      }
      card.append(cost);
    }

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

  /**
   * The ambient event ticker: one bounded newest-first page of the Core audit
   * timeline, rendered exactly as Core delivered it.
   *
   * Every row carries the stable audit id, Core's dotted action key (never
   * localized), the owning project and Lane, and the timestamp — the four
   * facts GUI-CORE-014's close criteria name. Rows carry no action: this is
   * ambient content, and the Decision Center owns the actionable queue.
   */
  const ticker = (): HTMLElement => {
    const section = document.createElement("section");
    section.className = "d10-ticker";
    section.dataset.d10Ticker = "true";
    const heading = document.createElement("h3");
    heading.className = "d10-thead";
    heading.textContent = copy.events;
    section.append(heading);

    const note = (text: string, state: string): HTMLElement => {
      const line = document.createElement("p");
      line.className = "d10-muted";
      line.dataset.d10TickerState = state;
      line.textContent = text;
      return line;
    };
    if (!events || !events.capabilityAvailable) {
      section.append(note(copy.eventsUnavailable, "unavailable"));
      return section;
    }
    if (events.outcome.state === "rejected") {
      // Core's own words for the refusal, never a client paraphrase.
      section.append(note(events.outcome.reason ?? copy.eventsUnavailable, "rejected"));
      return section;
    }
    if (!events.loaded) {
      section.append(note(copy.eventsPending, "pending"));
      return section;
    }
    if (events.rows.length === 0) {
      section.append(note(copy.eventsEmpty, "empty"));
      return section;
    }
    const list = document.createElement("ol");
    list.className = "d10-tlist";
    for (const row of events.rows) {
      const item = document.createElement("li");
      item.className = "d10-trow";
      item.dataset.d10Event = row.auditId;
      item.dataset.d10EventProject = row.projectId;

      const when = document.createElement("span");
      when.className = "d10-ttime";
      // Core's unix seconds, rendered in the viewer's locale. The ordering is
      // Core's; the client only formats.
      when.textContent = new Date(row.timestamp * 1000).toISOString().slice(11, 19);

      const kind = document.createElement("span");
      kind.className = "d10-tkind";
      // Core's stable dotted key, raw. Localizing it would make the timeline
      // undiffable across languages.
      kind.textContent = row.action;

      const owner = document.createElement("span");
      owner.className = "d10-towner";
      owner.textContent = row.laneId ?? row.projectId ?? copy.unbound;

      const outcome = document.createElement("span");
      outcome.className = "d10-toutcome";
      outcome.dataset.d10Outcome = row.outcome;
      outcome.textContent = row.outcome;

      item.append(when, kind, owner, outcome);
      list.append(item);
    }
    section.append(list);
    return section;
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

    stage.append(ticker());

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
    applyEvents: (next) => {
      events = next;
      render();
    },
  };
}
