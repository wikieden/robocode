import type { Locale } from "../i18n/catalog";
import type {
  PermissionChoice,
  PermissionDockProjection,
} from "../models/workspace";
import "./permission_dock.css";

export type { PermissionChoice, PermissionDockProjection } from "../models/workspace";

export interface PermissionIntent {
  type: "respond";
  requestId: string;
  choice: PermissionChoice;
  feedback: string | null;
}

export interface PermissionIntentResult {
  projection: PermissionDockProjection;
  pendingCommandId: string | null;
  outcome: { state: "idle" | "pending" | "confirmed" | "rejected"; reason: string | null };
}

const COPY = {
  en: {
    region: "Permission required",
    target: "Target",
    reason: "Reason",
    scope: "Scope",
    policy: "Policy",
    expires: "Expires",
    defaultAction: "Default",
    audit: "Audit",
    plan: "Plan mode blocks mutating approval responses.",
    once: "Allow once",
    session: "Allow for session",
    repo_allowlist: "Allow repo paths",
    always: "Always allow",
    edit: "Edit request",
    deny: "Deny",
  },
  "zh-CN": {
    region: "需要权限",
    target: "目标",
    reason: "原因",
    scope: "范围",
    policy: "策略",
    expires: "过期时间",
    defaultAction: "默认动作",
    audit: "审计",
    plan: "Plan 模式禁止变更型批准响应。",
    once: "仅允许一次",
    session: "本会话允许",
    repo_allowlist: "允许仓库路径",
    always: "始终允许",
    edit: "编辑请求",
    deny: "拒绝",
  },
} as const;

const SHORTCUTS: Partial<Record<PermissionChoice, string>> = {
  once: "Y",
  session: "A",
  repo_allowlist: "Shift+A",
  edit: "E",
  deny: "N",
};

export function renderPermissionDock(
  root: HTMLElement,
  projection: PermissionDockProjection,
  send: (intent: PermissionIntent) => Promise<unknown>,
  locale: Locale,
): void {
  const request = projection.request;
  if (!request) {
    root.replaceChildren();
    return;
  }
  const copy = COPY[locale];
  const dock = document.createElement("section");
  dock.className = "gperm dock";
  dock.dataset.permissionDock = "true";
  dock.tabIndex = -1;
  dock.setAttribute("role", "region");
  dock.setAttribute("aria-label", copy.region);

  const heading = document.createElement("header");
  heading.className = "gperm-hd";
  const cue = document.createElement("span");
  cue.className = "ic";
  cue.ariaHidden = "true";
  cue.textContent = "!";
  const title = document.createElement("span");
  title.textContent = `${request.title} · ${request.toolName}`;
  const risk = document.createElement("span");
  risk.className = `risk ${["high", "critical"].includes(request.risk) ? "hi" : request.risk === "low" ? "lo" : "md"}`;
  risk.textContent = request.risk.toUpperCase();
  heading.append(cue, title, risk);

  const facts = document.createElement("div");
  facts.className = "gperm-what";
  const command = document.createElement("code");
  command.className = "cmd";
  command.textContent = request.inputPreview;
  facts.append(command);
  const factText = document.createElement("p");
  factText.className = "why";
  const scope = request.actions
    .filter((action) => action.available && action.kind !== "deny")
    .map((action) => {
      if (action.kind === "session") return `${action.kind}(${action.sessionId ?? "—"})`;
      if (action.kind === "repo_allowlist") return `${action.kind}(${action.paths.join(", ")})`;
      return action.kind;
    })
    .join(", ");
  const policyArgs = Object.keys(request.policyReasonArgs).length > 0
    ? ` ${JSON.stringify(request.policyReasonArgs)}`
    : "";
  factText.textContent = [
    request.message,
    `${copy.target}: ${request.target.kind} · ${request.target.display}${request.target.canonicalRef ? ` · ${request.target.canonicalRef}` : ""}`,
    `${copy.reason}: ${request.reason ?? "—"}`,
    `${copy.scope}: ${scope || "—"}`,
    `${copy.policy}: ${request.policyReasonKey}${policyArgs}`,
    `${copy.expires}: ${request.expiresAt}`,
    `${copy.defaultAction}: ${request.defaultAction}`,
    `${copy.audit}: ${request.auditId}`,
  ].join(" · ");
  facts.append(factText);

  if (request.blockedByPlan) {
    const alert = document.createElement("p");
    alert.className = "gperm-plan";
    alert.setAttribute("role", "alert");
    alert.textContent = copy.plan;
    facts.append(alert);
  }

  const options = document.createElement("div");
  options.className = "gperm-opts";
  const actionButtons = new Map<PermissionChoice, HTMLButtonElement>();
  for (const action of request.actions) {
    const actionButton = document.createElement("button");
    actionButton.type = "button";
    actionButton.className = `gperm-opt${action.kind === "deny" ? " deny" : ""}`;
    actionButton.dataset.permissionAction = action.kind;
    actionButton.disabled = !action.available;
    const shortcut = SHORTCUTS[action.kind];
    if (shortcut) actionButton.setAttribute("aria-keyshortcuts", shortcut);
    const label = copy[action.kind];
    actionButton.textContent = `${shortcut ? `${shortcut} · ` : ""}${label}${action.code ? ` · ${action.code}` : ""}`;
    actionButton.addEventListener("click", () => {
      if (!action.available) return;
      void send({
        type: "respond",
        requestId: request.id,
        choice: action.kind,
        feedback: null,
      });
    });
    actionButtons.set(action.kind, actionButton);
    options.append(actionButton);
  }
  dock.addEventListener("keydown", (event) => {
    const choice = event.key.toLowerCase() === "y" ? "once" : event.key.toLowerCase() === "n" ? "deny" : null;
    if (!choice) return;
    const action = actionButtons.get(choice);
    if (!action || action.disabled) return;
    event.preventDefault();
    action.focus();
    action.click();
  });
  dock.append(heading, facts, options);
  root.replaceChildren(dock);
}
