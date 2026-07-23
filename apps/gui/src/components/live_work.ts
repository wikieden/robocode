import type { D1CockpitProjection } from "../models/workspace";
import type { Locale } from "../i18n/catalog";
import { translate } from "../i18n/catalog";
import { toolRowText } from "./tool_row";
import { localizedStatus } from "./tool_row";

export const MAX_LIVE_WORK_ITEMS = 24;

const RISK_KEYS = {
  low: "d1.risk.low",
  medium: "d1.risk.medium",
  high: "d1.risk.high",
  critical: "d1.risk.critical",
} as const;

export function taskRowText(
  task: D1CockpitProjection["liveWork"]["tasks"][number],
  locale: Locale,
): string {
  return `${task.title} · ${localizedStatus(locale, task.status)} · ${task.progress}%`;
}

export function approvalRowText(
  approval: D1CockpitProjection["liveWork"]["approvals"][number],
  locale: Locale,
): string {
  const key = RISK_KEYS[approval.risk as keyof typeof RISK_KEYS];
  return `${approval.title} · ${key ? translate(locale, key, {}) : approval.risk}`;
}

export interface LiveWorkEntry {
  primary: string;
  secondary: string | null;
  text: string;
}

/** Flatten typed categories once, then cap the shared presentation list. */
export function boundedLiveWorkEntries(
  projection: D1CockpitProjection,
  locale: Locale,
): LiveWorkEntry[] {
  const { tasks, tools, approvals, queuedInputs, evidence } = projection.liveWork;
  return [
    ...tasks.map((task) => ({
      primary: task.title,
      secondary: `${localizedStatus(locale, task.status)} · ${task.progress}%`,
      text: taskRowText(task, locale),
    })),
    ...tools.map((tool) => ({ primary: tool.name, secondary: tool.inputPreview, text: toolRowText(tool) })),
    ...approvals.map((approval) => ({
      primary: approval.title,
      secondary: approvalRowText(approval, locale).replace(`${approval.title} · `, ""),
      text: approvalRowText(approval, locale),
    })),
    ...queuedInputs.map((input) => ({
      primary: input.contentPreview,
      secondary: null,
      text: input.contentPreview,
    })),
    ...evidence.map((item) => ({
      primary: item.summary,
      secondary: item.kind,
      text: `${item.kind} · ${item.summary}`,
    })),
  ].slice(0, MAX_LIVE_WORK_ITEMS);
}

/** A compact, typed live-work summary for the center work surface. */
export function renderLiveWorkBar(
  projection: D1CockpitProjection,
  locale: Locale,
): HTMLElement | null {
  const entries = boundedLiveWorkEntries(projection, locale);
  if (entries.length === 0) {
    return null;
  }
  const bar = document.createElement("section");
  bar.className = "d1-live-work-bar";
  bar.dataset.liveWorkBar = "true";
  bar.dataset.centerStep = "live-work";
  bar.setAttribute("role", "status");
  const title = document.createElement("strong");
  title.textContent = translate(locale, "d1.liveWork", {});
  const primary = document.createElement("strong");
  primary.className = "d1-live-work-primary";
  primary.dataset.liveWorkPrimary = "true";
  const [first, ...remaining] = entries;
  bar.setAttribute("aria-label", translate(locale, "d1.liveWork", {}));
  if (first) {
    bar.setAttribute("aria-description", first.text);
  }
  primary.textContent = first?.primary ?? "";
  const secondary = document.createElement("span");
  secondary.className = "d1-live-work-secondary";
  secondary.dataset.liveWorkSecondary = "true";
  secondary.textContent = [
    ...(first?.secondary ? [first.secondary] : []),
    ...remaining.map((entry) => entry.text),
  ].join(" · ");
  bar.append(title, primary, secondary);
  return bar;
}
