import type { Locale } from "../i18n/catalog";
import "./d12_integration_gate.css";

/// D12 integration gate.
///
/// The failure path of the acceptance loop. The screen never offers a manual
/// merge: `accept` opens only when Core has recorded every evidence id the
/// gate policy requires, and the conflict is resolved by bouncing it back to
/// the origin Lane.

export interface D12Unavailable {
  key: string;
  code: string;
}

export interface D12Gate {
  gateId: string;
  taskId: string;
  status: string;
  gateType: string;
  projectId: string;
  laneId: string | null;
  requiresIndependentValidator: boolean;
  hasValidator: boolean;
  requiredEvidence: string[];
  evidenceIds: string[];
}

export interface D12Bounce {
  bounceId: string;
  originalLaneId: string;
  taskId: string;
  reason: string;
  status: string;
  evidenceIds: string[];
}

export interface D12Revert {
  revertId: string;
  appliedChangeId: string;
  reason: string;
  restoredPaths: string[];
  auditId: string;
  revertedAt: number;
}

export interface D12Check {
  id: string;
  name: string;
  status: string;
}

export interface D12Action {
  kind: string;
  available: boolean;
  code: string | null;
}

export interface D12GateDetail {
  gate: D12Gate;
  missingEvidence: string[];
  bounces: D12Bounce[];
  reverts: D12Revert[];
  checks: D12Check[];
  actions: D12Action[];
}

export interface D12IntegrationGateProjection {
  gates: D12Gate[];
  selectedGateId: string | null;
  detail: D12GateDetail | null;
  unavailable: D12Unavailable[];
}

export interface D12Controller {
  applyProjection: (projection: D12IntegrationGateProjection) => void;
}

type Copy = Record<string, string>;

const COPY: Record<Locale, Copy> = {
  en: {
    title: "Integration gate",
    gates: "gates",
    conflict: "Merge conflict · bounce to the origin Lane",
    resolved: "Gate passed",
    strong: "strong gate · cannot be bypassed",
    validator: "independent validator required",
    validatorPresent: "validator recorded",
    validatorMissing: "no validator recorded",
    missing: "Missing required evidence",
    timeline: "Recovery timeline",
    noBounce: "Core recorded no bounce for this gate.",
    reverts: "Post-merge rollback",
    checks: "Checks",
    accept: "Accept and merge",
    reject: "Bounce to origin Lane",
    noGate: "Core published no integration gate.",
    "d12.conflict.noStructuredHunk":
      "Core publishes no structured conflict content, so the conflicting hunk cannot be shown.",
  },
  "zh-CN": {
    title: "集成闸",
    gates: "个闸",
    conflict: "合并冲突 · 退回原 Lane",
    resolved: "闸已通过",
    strong: "强闸 · 不可绕过",
    validator: "要求独立验证方",
    validatorPresent: "已记录验证方",
    validatorMissing: "未记录验证方",
    missing: "缺少必需证据",
    timeline: "恢复时间线",
    noBounce: "Core 未为该闸记录任何退回。",
    reverts: "合入后回滚",
    checks: "检查",
    accept: "批准并合入",
    reject: "退回原 Lane",
    noGate: "Core 未发布任何集成闸。",
    "d12.conflict.noStructuredHunk": "Core 不发布结构化冲突内容，冲突 hunk 无法展示。",
  },
};

function label(copy: Copy, key: string): string {
  return copy[key] ?? key;
}

/// Terminal Core statuses. Anything else is still an open gate.
const RESOLVED = new Set(["merged", "accepted", "reverted"]);

export function renderD12IntegrationGate(
  root: HTMLElement,
  initial: D12IntegrationGateProjection,
  locale: Locale,
  onSelect?: (gateId: string) => void,
): D12Controller {
  let projection = initial;
  const copy = COPY[locale];

  const section = (titleKey: string): HTMLElement => {
    const heading = document.createElement("div");
    heading.className = "d12-sec";
    heading.textContent = label(copy, titleKey);
    return heading;
  };

  const render = (): void => {
    const stage = document.createElement("section");
    stage.className = "d12-stage";
    stage.dataset.route = "d12";

    const bar = document.createElement("div");
    bar.className = "d12-head";
    const heading = document.createElement("h2");
    heading.className = "d12-title";
    heading.textContent = copy.title;
    const count = document.createElement("span");
    count.className = "d12-count";
    count.textContent = `${projection.gates.length} ${copy.gates}`;
    bar.append(heading, count);

    const list = document.createElement("div");
    list.className = "d12-gates";
    for (const gate of projection.gates) {
      const chip = document.createElement("button");
      chip.type = "button";
      chip.className = "d12-gchip";
      chip.dataset.d12Gate = gate.gateId;
      chip.setAttribute("aria-pressed", String(projection.selectedGateId === gate.gateId));
      chip.textContent = `${gate.gateId} · ${gate.status}`;
      if (onSelect) chip.addEventListener("click", () => onSelect(gate.gateId));
      list.append(chip);
    }
    bar.append(list);
    stage.append(bar);

    const detail = projection.detail;
    if (!detail) {
      const empty = document.createElement("p");
      empty.className = "d12-muted";
      empty.dataset.d12Empty = "true";
      empty.textContent = copy.noGate;
      stage.append(empty);
      root.replaceChildren(stage);
      return;
    }

    const resolved = RESOLVED.has(detail.gate.status);
    const banner = document.createElement("div");
    banner.className = "d12-banner";
    banner.dataset.d12Banner = resolved ? "resolved" : "conflict";
    const bannerText = document.createElement("span");
    bannerText.textContent = resolved ? copy.resolved : copy.conflict;
    const strength = document.createElement("span");
    strength.className = "d12-strength";
    strength.textContent = resolved ? detail.gate.status : copy.strong;
    banner.append(bannerText, strength);
    stage.append(banner);

    const policy = document.createElement("p");
    policy.className = "d12-policy";
    policy.dataset.d12Policy = detail.gate.gateType;
    const validatorNote = detail.gate.requiresIndependentValidator
      ? `${copy.validator} · ${detail.gate.hasValidator ? copy.validatorPresent : copy.validatorMissing}`
      : "";
    policy.textContent = [detail.gate.gateType, detail.gate.projectId, detail.gate.laneId, validatorNote]
      .filter((part): part is string => Boolean(part))
      .join(" · ");
    stage.append(policy);

    if (detail.missingEvidence.length > 0) {
      const missing = document.createElement("p");
      missing.className = "d12-missing";
      missing.dataset.d12Missing = String(detail.missingEvidence.length);
      missing.textContent = `${copy.missing}: ${detail.missingEvidence.join(", ")}`;
      stage.append(missing);
    }

    stage.append(section("timeline"));
    const timeline = document.createElement("ol");
    timeline.className = "d12-timeline";
    if (detail.bounces.length === 0) {
      const none = document.createElement("li");
      none.className = "d12-muted";
      none.textContent = copy.noBounce;
      timeline.append(none);
    }
    for (const bounce of detail.bounces) {
      const item = document.createElement("li");
      item.dataset.d12Bounce = bounce.bounceId;
      item.dataset.d12BounceStatus = bounce.status;
      item.textContent = `${bounce.originalLaneId} · ${bounce.status} · ${bounce.reason}`;
      timeline.append(item);
    }
    stage.append(timeline);

    if (detail.checks.length > 0) {
      stage.append(section("checks"));
      const checks = document.createElement("ul");
      checks.className = "d12-checks";
      for (const check of detail.checks) {
        const item = document.createElement("li");
        item.dataset.d12Check = check.id;
        item.textContent = `${check.name} · ${check.status}`;
        checks.append(item);
      }
      stage.append(checks);
    }

    if (detail.reverts.length > 0) {
      stage.append(section("reverts"));
      const reverts = document.createElement("ul");
      reverts.className = "d12-reverts";
      for (const revert of detail.reverts) {
        const item = document.createElement("li");
        item.dataset.d12Revert = revert.revertId;
        item.textContent = `${revert.reason} · ${revert.restoredPaths.join(", ")} · ${revert.auditId}`;
        reverts.append(item);
      }
      stage.append(reverts);
    }

    const bar2 = document.createElement("div");
    bar2.className = "d12-gatebar";
    for (const action of detail.actions) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "d12-gbtn";
      button.dataset.d12Action = action.kind;
      button.disabled = !action.available;
      button.textContent = label(copy, action.kind);
      bar2.append(button);
    }
    stage.append(bar2);

    for (const entry of projection.unavailable) {
      const note = document.createElement("p");
      note.className = "d12-unavailable";
      note.dataset.d12Unavailable = entry.code;
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
