import type { Locale } from "../i18n/catalog";
import type { D6RecoveryProjection, D6State } from "../models/workspace";
import "./d6_recovery.css";

export type { D6RecoveryProjection } from "../models/workspace";

const COPY = {
  en: {
    empty: ["EMPTY", "No Core-owned Lane is available yet."],
    connecting: ["CONNECTING", "Establishing the versioned Core connection."],
    disconnected: ["DISCONNECTED", "The Core transport is unavailable."],
    provider_error: ["PROVIDER ERROR", "Core reports a provider failure."],
    agent_stopped: ["AGENT STOPPED", "Core reports that execution has stopped."],
    context_overflow: ["CONTEXT OVERFLOW", "The Core context hard limit was exceeded."],
    gate_queue_clear: ["QUEUE CLEAR", "Core reports no pending approvals or gates."],
    incompatible_schema: ["INCOMPATIBLE", "The frontend schema is incompatible with Core."],
    missing_feature_capability: ["CAPABILITY MISSING", "Core did not advertise a required capability."],
    event_gap: ["EVENT GAP", "Ordered events are incomplete; snapshot recovery is required."],
    live: ["LIVE", "Core state is current."],
    facts: "Core facts",
    reconnect: "Reconnect",
    inspect: "Inspect facts",
    restart: "Restart agent",
    close_lane: "Close Lane",
    checkpoint: "Restore checkpoint",
    open_project: "Open project",
    tokens: "Tokens",
    missing: "Missing",
    recovered: "Core snapshot is live.",
  },
  "zh-CN": {
    empty: ["空工作区", "Core 尚未提供可用 Lane。"],
    connecting: ["连接中", "正在建立版本化 Core 连接。"],
    disconnected: ["已断开", "Core 传输当前不可用。"],
    provider_error: ["提供方错误", "Core 报告提供方故障。"],
    agent_stopped: ["智能体已停止", "Core 报告执行已停止。"],
    context_overflow: ["上下文溢出", "已超过 Core 上下文硬限制。"],
    gate_queue_clear: ["队列已清空", "Core 报告没有待处理批准或门禁。"],
    incompatible_schema: ["协议不兼容", "前端 schema 与 Core 不兼容。"],
    missing_feature_capability: ["能力缺失", "Core 未声明所需能力。"],
    event_gap: ["事件缺口", "有序事件不完整，需要快照恢复。"],
    live: ["在线", "Core 状态为最新。"],
    facts: "Core 事实",
    reconnect: "重新连接",
    inspect: "检查事实",
    restart: "重启智能体",
    close_lane: "关闭 Lane",
    checkpoint: "恢复检查点",
    open_project: "打开项目",
    tokens: "Token",
    missing: "缺失能力",
    recovered: "Core 快照已恢复在线。",
  },
} as const;

export interface D6Controller {
  applyProjection: (projection: D6RecoveryProjection) => void;
}

export function renderD6Recovery(
  root: HTMLElement,
  initial: D6RecoveryProjection,
  reconnect: () => Promise<D6RecoveryProjection>,
  locale: Locale,
  openProject?: () => void,
): D6Controller {
  let projection = initial;
  let reconnecting = false;
  const copy = COPY[locale];

  const render = (): void => {
    const stateCopy = copy[projection.state as D6State];
    const stage = document.createElement("section");
    stage.className = "d6-stage";
    stage.dataset.d6State = projection.state;
    stage.setAttribute("aria-live", "polite");
    stage.setAttribute("aria-busy", String(reconnecting));

    const tag = document.createElement("span");
    tag.className = "d6-tag";
    tag.dataset.d6Tag = "true";
    tag.textContent = `◌ ${stateCopy[0]}`;
    const title = document.createElement("h2");
    title.textContent = stateCopy[1];
    const detail = document.createElement("p");
    detail.className = "d6-detail";
    detail.textContent = projection.detail ?? projection.hint ?? copy.facts;
    stage.append(tag, title, detail);

    const facts = document.createElement("dl");
    facts.className = "d6-facts";
    if (projection.usedTokens !== null || projection.hardTokenLimit !== null) {
      const dt = document.createElement("dt");
      dt.textContent = copy.tokens;
      const dd = document.createElement("dd");
      dd.textContent = `${projection.usedTokens?.toLocaleString(locale) ?? "—"} / ${projection.hardTokenLimit?.toLocaleString(locale) ?? "—"}`;
      facts.append(dt, dd);
    }
    if (projection.missingCapabilities.length > 0) {
      const dt = document.createElement("dt");
      dt.textContent = copy.missing;
      const dd = document.createElement("dd");
      dd.textContent = projection.missingCapabilities.join(", ");
      facts.append(dt, dd);
    }
    if (facts.children.length > 0) stage.append(facts);

    const actions = document.createElement("div");
    actions.className = "d6-actions";
    for (const action of projection.actions) {
      const actionButton = document.createElement("button");
      actionButton.type = "button";
      actionButton.dataset.d6Action = action.kind;
      actionButton.disabled = !action.available || reconnecting;
      const label = copy[action.kind as keyof typeof copy] ?? action.kind;
      actionButton.textContent = `${label}${action.available ? "" : ` · ${action.code}`}`;
      if (action.kind === "reconnect" && action.available) {
        actionButton.addEventListener("click", () => {
          if (reconnecting) return;
          reconnecting = true;
          render();
          void reconnect().then((next) => {
            // Success is rendered only after the callback returns Core's live projection.
            projection = next;
          }).finally(() => {
            reconnecting = false;
            render();
          });
        });
      }
      actions.append(actionButton);
    }
    if (projection.state === "empty" && openProject) {
      const openProjectButton = document.createElement("button");
      openProjectButton.type = "button";
      openProjectButton.dataset.openProject = "true";
      openProjectButton.textContent = copy.open_project;
      openProjectButton.addEventListener("click", openProject);
      actions.prepend(openProjectButton);
    }
    stage.append(actions);
    if (projection.state === "live" && projection.connection === "live") {
      const success = document.createElement("p");
      success.dataset.d6Success = "true";
      success.setAttribute("role", "status");
      success.textContent = `✓ ${copy.recovered}`;
      stage.append(success);
    }
    root.replaceChildren(stage);
  };

  const controller: D6Controller = {
    applyProjection: (next) => {
      projection = next;
      if (next.connection === "live") reconnecting = false;
      render();
    },
  };
  render();
  return controller;
}
