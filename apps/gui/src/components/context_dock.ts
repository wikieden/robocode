import { translate, type Locale } from "../i18n/catalog";
import type { D1CockpitProjection } from "../models/workspace";
import { environmentValues } from "./environment";
import { boundedLiveWorkEntries } from "./live_work";

function definition(list: HTMLDListElement, label: string, value: string): void {
  const term = document.createElement("dt");
  term.textContent = label;
  const detail = document.createElement("dd");
  detail.textContent = value;
  list.append(term, detail);
}

export function renderContextDock(
  projection: D1CockpitProjection,
  locale: Locale,
  matchesSelectedLane = true,
): HTMLElement {
  const dock = document.createElement("aside");
  dock.id = "d1-context-dock";
  dock.className = "envp d1-right";
  dock.dataset.shellLandmark = "context-dock";
  dock.dataset.cockpitRole = "context";
  dock.dataset.contextDock = "true";
  dock.dataset.drawerOpen = "false";
  if (!matchesSelectedLane) {
    const waiting = document.createElement("p");
    waiting.dataset.contextDockWaiting = "true";
    waiting.textContent = translate(locale, "d1.context.switching", {});
    dock.append(waiting);
    return dock;
  }

  const environment = document.createElement("section");
  environment.setAttribute("aria-label", translate(locale, "d1.environment", {}));
  const environmentTitle = document.createElement("h2");
  environmentTitle.textContent = translate(locale, "d1.environment", {});
  const environmentFacts = document.createElement("dl");
  const environmentLabels = [
    translate(locale, "d1.environment.provider", {}),
    translate(locale, "d1.environment.model", {}),
    translate(locale, "d1.environment.mode", {}),
    translate(locale, "d1.environment.permission", {}),
    translate(locale, "d1.environment.tokens", {}),
    translate(locale, "d1.environment.cost", {}),
  ];
  environmentValues(projection.environment).forEach((value, index) => {
    definition(environmentFacts, environmentLabels[index]!, value);
  });
  environment.append(environmentTitle, environmentFacts);

  const liveWork = document.createElement("section");
  liveWork.setAttribute("aria-label", translate(locale, "d1.liveWork", {}));
  const workTitle = document.createElement("h2");
  workTitle.textContent = translate(locale, "d1.liveWork", {});
  liveWork.append(workTitle);
  for (const { text } of boundedLiveWorkEntries(projection, locale)) {
    const item = document.createElement("div");
    item.className = "d1-work-item";
    item.textContent = text;
    liveWork.append(item);
  }
  for (const unavailable of projection.unavailableFeatures) {
    const item = document.createElement("div");
    item.className = "d1-unavailable";
    item.dataset.unavailableFeature = unavailable.id;
    item.setAttribute("aria-disabled", "true");
    item.textContent = `${translate(locale, "d1.unavailable", {})} · ${unavailable.id} · ${unavailable.code}`;
    item.title = unavailable.code;
    liveWork.append(item);
  }
  dock.append(environment, liveWork);
  return dock;
}
