import { translate, type Locale } from "../i18n/catalog";
import type { D1CockpitProjection } from "../models/workspace";

export function adjacentLaneId(
  laneIds: readonly string[],
  index: number,
  direction: "previous" | "next",
): string | null {
  if (laneIds.length === 0) return null;
  const offset = direction === "next" ? 1 : -1;
  return laneIds[(index + offset + laneIds.length) % laneIds.length] ?? null;
}

export interface LaneRailOptions {
  projection: D1CockpitProjection;
  locale: Locale;
  selectedLaneId: string | null;
  onCreateLane: () => void;
  onSelectLane: (laneId: string) => void;
  onRetryAgent: (sessionId: string, laneId: string) => void;
}

function railButton(label = ""): HTMLButtonElement {
  const element = document.createElement("button");
  element.type = "button";
  element.className = "d1-action";
  element.textContent = label;
  return element;
}

export function renderLaneRail(options: LaneRailOptions): HTMLElement {
  const { projection, locale, selectedLaneId } = options;
  const lanes = document.createElement("nav");
  lanes.className = "side d1-lanes";
  lanes.dataset.shellLandmark = "lane-rail";
  lanes.dataset.cockpitRole = "lanes";
  lanes.setAttribute("aria-label", translate(locale, "d1.lanes", {}));

  const laneTitle = document.createElement("h2");
  laneTitle.textContent = translate(locale, "d1.lanes", {});
  const createLane = railButton(translate(locale, "d1.lane.create", {}));
  createLane.dataset.createLane = "true";
  createLane.classList.add("d1-create-lane");
  createLane.setAttribute("aria-haspopup", "menu");
  createLane.setAttribute("aria-expanded", "false");
  createLane.addEventListener("click", options.onCreateLane);
  lanes.append(laneTitle, createLane);

  projection.lanes.forEach((lane, index) => {
    const boundSession = projection.agentSessions.find(
      (candidate) => candidate.laneId === lane.id,
    );
    const item = railButton();
    item.className = "wslane d1-lane";
    item.dataset.laneId = lane.id;
    item.dataset.laneAgentId = boundSession?.agentId ?? "viden";
    item.setAttribute("aria-current", String(lane.id === selectedLaneId));

    const status = document.createElement("span");
    status.className = "d1-lane-status";
    status.dataset.status = lane.status;
    const copy = document.createElement("span");
    copy.className = "lbody";
    const title = document.createElement("strong");
    title.textContent = lane.id;
    const detail = document.createElement("small");
    detail.textContent = boundSession
      ? `${boundSession.agentId} · ${boundSession.status}`
      : `Viden · ${lane.status}`;
    copy.append(title, detail);
    item.append(status, copy);
    item.addEventListener("keydown", (event) => {
      if (!["ArrowUp", "ArrowDown"].includes(event.key)) return;
      event.preventDefault();
      const target = adjacentLaneId(
        projection.lanes.map((candidate) => candidate.id),
        index,
        event.key === "ArrowDown" ? "next" : "previous",
      );
      if (target) {
        Array.from(lanes.querySelectorAll<HTMLElement>("[data-lane-id]"))
          .find((candidate) => candidate.dataset.laneId === target)
          ?.focus();
      }
    });
    item.addEventListener("click", () => options.onSelectLane(lane.id));
    lanes.append(item);

    if (boundSession && ["failed", "cancelled"].includes(boundSession.status)) {
      const retry = railButton(translate(locale, "d1.session.retry", {}));
      retry.className = "d1-lane-agent-retry";
      retry.dataset.retryLaneAgent = lane.id;
      retry.addEventListener("click", () =>
        options.onRetryAgent(boundSession.sessionId, lane.id),
      );
      lanes.append(retry);
    }
  });
  return lanes;
}
