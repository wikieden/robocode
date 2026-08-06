import { translate, type Locale } from "../i18n/catalog";
import type { D1CockpitProjection } from "../models/workspace";
import { localizedStatus } from "./tool_row";

/// Live turn feedback for the D1 work surface.
///
/// While Core is working, the operator needs three things the transcript
/// cannot give them: that work is still moving, how long it has been moving,
/// and how to stop it. The strip carries only published facts for the first
/// and third; the elapsed clock is explicitly client-observed, because
/// frontend-contract-v1 has no owner-scoped start timestamp (GUI-CORE-010).

export interface WorkStatusModel {
  busy: boolean;
  /// Core session status for the selected Lane, when Core scopes one to it.
  coreStatus: string | null;
  /// The task text Core recorded for the running session.
  task: string | null;
  canCancel: boolean;
  reducedMotion: boolean;
}

export function workStatusModel(
  projection: D1CockpitProjection,
  selectedLaneId: string | null,
  canCancel: boolean,
): WorkStatusModel {
  const session = selectedLaneId
    ? projection.agentSessions.find((candidate) => candidate.laneId === selectedLaneId)
    : undefined;
  return {
    busy: projection.composer.busy,
    // The Agent session is the only fact that carries a status scoped to this
    // Lane. Without one the strip says so rather than borrowing another
    // Lane's status.
    coreStatus: session?.status ?? null,
    task: session?.task ?? null,
    canCancel,
    reducedMotion: projection.preferences.motion === "reduced",
  };
}

export function formatElapsed(milliseconds: number): string {
  const total = Math.max(0, Math.floor(milliseconds / 1000));
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}

export interface WorkStatusStrip {
  element: HTMLElement;
  /// Stops the elapsed clock. Always call before dropping the element.
  dispose: () => void;
}

export function renderWorkStatus(
  model: WorkStatusModel,
  locale: Locale,
  startedAt: number,
  now: () => number,
  onCancel: () => void,
): WorkStatusStrip {
  const strip = document.createElement("div");
  strip.className = "d1-work-status";
  strip.dataset.workStatus = model.busy ? "busy" : "idle";
  strip.setAttribute("role", "status");

  if (!model.busy) {
    strip.textContent = translate(locale, "d1.stream.idle", {});
    return { element: strip, dispose: () => undefined };
  }

  if (model.coreStatus) strip.dataset.workCoreStatus = model.coreStatus;

  const marker = document.createElement("span");
  marker.className = "d1-work-marker";
  marker.dataset.workMarker = "true";
  marker.ariaHidden = "true";
  // Motion is a Core-owned preference; a reduced-motion operator gets a
  // static marker rather than a spinner.
  marker.dataset.motion = model.reducedMotion ? "reduced" : "animated";
  strip.append(marker);

  const label = document.createElement("span");
  label.className = "d1-work-label";
  label.textContent = model.coreStatus
    ? localizedStatus(locale, model.coreStatus)
    : translate(locale, "d1.work.unscopedStatus", {});
  if (!model.coreStatus) label.title = "GUI-CORE-010";
  strip.append(label);

  if (model.task) {
    const task = document.createElement("span");
    task.className = "d1-work-task";
    task.dataset.workTask = "true";
    task.textContent = model.task;
    strip.append(task);
  }

  const elapsed = document.createElement("time");
  elapsed.className = "d1-work-elapsed";
  elapsed.dataset.workElapsed = "true";
  // Screen readers should not hear a per-second tick inside a live region.
  elapsed.setAttribute("aria-hidden", "true");
  elapsed.title = translate(locale, "d1.work.elapsedSource", {});
  const paint = (): void => {
    elapsed.textContent = formatElapsed(now() - startedAt);
  };
  paint();
  const timer = window.setInterval(paint, 1000);
  strip.append(elapsed);

  if (model.canCancel) {
    const cancel = document.createElement("button");
    cancel.type = "button";
    cancel.className = "d1-work-cancel";
    cancel.dataset.workCancel = "true";
    cancel.textContent = translate(locale, "d1.work.cancel", {});
    cancel.addEventListener("click", onCancel);
    strip.append(cancel);
  }

  return {
    element: strip,
    dispose: () => window.clearInterval(timer),
  };
}
