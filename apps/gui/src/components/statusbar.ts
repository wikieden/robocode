import { translate, type Locale } from "../i18n/catalog";
import type { D1StatusbarProjection } from "../models/workspace";

/// Window-level statusbar for the D1 cockpit.
///
/// Every segment renders a fact the host projected from the confirmed Core
/// view. A segment whose fact is absent renders an explicit em-dash — the
/// bar never invents a number. The pending-gate segment is the bar's only
/// interactive element: it navigates to the D2 decision queue.

const EM_DASH = "—";

/// Compact token counts in the design's terminal vocabulary ("42.1k").
export function formatCompactCount(value: number): string {
  if (value < 1000) return String(value);
  const thousands = value / 1000;
  if (thousands >= 100) return `${Math.round(thousands)}k`;
  return `${(Math.round(thousands * 10) / 10).toString()}k`;
}

function formatLatency(milliseconds: number): string {
  if (milliseconds >= 1000) return `${(milliseconds / 1000).toFixed(1)}s`;
  return `${milliseconds}ms`;
}

function segment(
  bar: HTMLElement,
  id: string,
  label: string,
  first: boolean,
): HTMLElement {
  if (!first) {
    const separator = document.createElement("span");
    separator.className = "sb-sep2";
    separator.setAttribute("aria-hidden", "true");
    separator.textContent = "┆";
    bar.append(separator);
  }
  const item = document.createElement("span");
  item.className = "sb-it";
  item.dataset.sbSegment = id;
  const name = document.createElement("span");
  name.className = "lbl";
  name.textContent = `${label} `;
  item.append(name);
  bar.append(item);
  return item;
}

function value(item: HTMLElement, text: string, className = ""): void {
  const strong = document.createElement("b");
  if (className) strong.className = className;
  strong.textContent = text;
  item.append(strong);
}

export function renderStatusbar(
  statusbar: D1StatusbarProjection,
  locale: Locale,
  onNavigate?: (route: string) => void,
): HTMLElement {
  const bar = document.createElement("footer");
  bar.className = "statusbar d1-status";
  bar.dataset.shellLandmark = "statusbar";
  bar.dataset.statusbar = "true";

  const mode = segment(bar, "mode", translate(locale, "d1.statusbar.mode", {}), true);
  value(mode, statusbar.workMode, "gd");

  const perm = segment(bar, "perm", translate(locale, "d1.statusbar.perm", {}), false);
  value(perm, statusbar.permissionLevel);

  const context = segment(bar, "context", translate(locale, "d1.statusbar.context", {}), false);
  if (statusbar.context) {
    const { usedTokens, hardTokenLimit, exceeded } = statusbar.context;
    value(context, `${formatCompactCount(usedTokens)} / ${formatCompactCount(hardTokenLimit)}`);
    if (hardTokenLimit > 0) {
      const percent = document.createElement("span");
      percent.className = exceeded ? "sb-exceeded" : "ac";
      percent.textContent = ` ${Math.round((usedTokens / hardTokenLimit) * 100)}%`;
      context.append(percent);
    }
  } else {
    value(context, EM_DASH);
  }

  // The replay-cursor stream position — Core publishes no event counter, so
  // the segment is labeled and titled as a position, never a count.
  const events = segment(bar, "events", translate(locale, "d1.statusbar.events", {}), false);
  events.title = translate(locale, "d1.statusbar.eventsTitle", {});
  value(events, `#${statusbar.eventStreamPosition}`);

  const lane = segment(bar, "lane", translate(locale, "d1.statusbar.lane", {}), false);
  if (statusbar.lane) {
    value(lane, `${statusbar.lane.laneId} ${statusbar.lane.agentId ?? EM_DASH}`, "ac");
    const phase = document.createElement("span");
    phase.className = "sb-progress";
    phase.textContent = ` ${statusbar.lane.status} ${
      statusbar.lane.progress === null ? EM_DASH : `${statusbar.lane.progress}%`
    }`;
    lane.append(phase);
  } else {
    value(lane, EM_DASH);
  }

  const latency = segment(bar, "latency", translate(locale, "d1.statusbar.latency", {}), false);
  if (statusbar.latency) {
    value(
      latency,
      statusbar.latency.lastLatencyMs === null
        ? EM_DASH
        : formatLatency(statusbar.latency.lastLatencyMs),
    );
    const average = document.createElement("span");
    average.className = "sb-faint";
    average.textContent = ` avg ${
      statusbar.latency.averageLatencyMs === null
        ? EM_DASH
        : formatLatency(statusbar.latency.averageLatencyMs)
    }`;
    latency.append(average);
  } else {
    value(latency, EM_DASH);
  }

  const tokens = segment(bar, "tokens", translate(locale, "d1.statusbar.tokens", {}), false);
  if (statusbar.tokens) {
    value(
      tokens,
      `${formatCompactCount(statusbar.tokens.inputTokens)}↑ ${formatCompactCount(
        statusbar.tokens.outputTokens,
      )}↓`,
    );
  } else {
    value(tokens, EM_DASH);
  }

  const diag = segment(bar, "diag", translate(locale, "d1.statusbar.diag", {}), false);
  value(
    diag,
    `${statusbar.diagnosticsCount}✕`,
    statusbar.diagnosticsCount > 0 ? "sb-error" : "",
  );

  const requests = segment(bar, "req", translate(locale, "d1.statusbar.req", {}), false);
  if (statusbar.requests) {
    value(requests, `${statusbar.requests.requestCount} req`, "ok");
    const errors = document.createElement("span");
    errors.className = statusbar.requests.errorCount > 0 ? "sb-error" : "sb-faint";
    errors.textContent = ` / ${statusbar.requests.errorCount} err`;
    requests.append(errors);
  } else {
    value(requests, EM_DASH);
  }

  if (statusbar.pendingGateCount > 0) {
    // The only interactive segment: it opens the D2 decision queue where the
    // waiting gates are actually decided.
    const gate = document.createElement("button");
    gate.type = "button";
    gate.className = "sb-right d1-sb-gate";
    gate.dataset.sbGate = "true";
    gate.textContent = `⏸ ${translate(locale, "d1.statusbar.gateWaiting", {
      count: String(statusbar.pendingGateCount),
    })}`;
    gate.disabled = !onNavigate;
    gate.addEventListener("click", () => onNavigate?.("d2"));
    bar.append(gate);
  }

  return bar;
}
