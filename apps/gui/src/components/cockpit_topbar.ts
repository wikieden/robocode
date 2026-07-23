import { translate, type Locale } from "../i18n/catalog";
import type { D1CockpitProjection } from "../models/workspace";
import { createCanonicalGuiIcon } from "./activity_rail";

const BRAND_MARK_URL = new URL(
  "../../../../docs/viden-design/Viden/brand-assets/icon.svg",
  import.meta.url,
).href;

export interface CockpitTopbar {
  element: HTMLElement;
  contextDrawerToggle: HTMLButtonElement;
}

export function renderCockpitTopbar(
  projection: D1CockpitProjection,
  locale: Locale,
  showWelcome: boolean,
): CockpitTopbar {
  const titlebar = document.createElement("header");
  titlebar.className = "titlebar vbar d1-titlebar";
  titlebar.dataset.shellLandmark = "topbar";
  titlebar.dataset.tauriDragRegion = "true";

  const lights = document.createElement("span");
  lights.className = "tl";
  lights.ariaHidden = "true";
  for (const lightClass of ["a", "b", "c"]) {
    const light = document.createElement("i");
    light.className = lightClass;
    lights.append(light);
  }

  const brand = document.createElement("span");
  brand.className = "d1-topbar-brand";
  brand.dataset.tauriDragRegion = "true";
  const mark = document.createElement("img");
  mark.src = BRAND_MARK_URL;
  mark.alt = "";
  mark.width = 24;
  mark.height = 24;
  const wordmark = document.createElement("strong");
  wordmark.textContent = "viden";
  brand.append(mark, wordmark);

  const project = document.createElement("span");
  project.className = "projsel d1-topbar-project";
  project.dataset.tauriDragRegion = "true";
  project.textContent = showWelcome
    ? translate(locale, "d1.welcome.noProject", {})
    : projection.environment.cwd;

  const lane = projection.lanes.find((candidate) => candidate.id === projection.selectedLaneId);
  const laneSummary = document.createElement("span");
  laneSummary.className = "gitops d1-topbar-lane";
  laneSummary.dataset.tauriDragRegion = "true";
  laneSummary.textContent = lane
    ? `${lane.id} · ${lane.summary}${lane.branch ? ` · ${lane.branch}` : ""}`
    : translate(locale, "d1.lanes", {});

  const contextDrawerToggle = document.createElement("button");
  contextDrawerToggle.type = "button";
  contextDrawerToggle.className = "tbtbtn d1-context-drawer-toggle";
  contextDrawerToggle.dataset.contextDrawerToggle = "true";
  contextDrawerToggle.setAttribute("aria-controls", "d1-context-dock");
  contextDrawerToggle.setAttribute("aria-expanded", "false");
  contextDrawerToggle.setAttribute(
    "aria-label",
    `${translate(locale, "d1.environment", {})} dock`,
  );
  contextDrawerToggle.hidden = showWelcome;
  contextDrawerToggle.append(createCanonicalGuiIcon("panel"));

  const tools = document.createElement("span");
  tools.className = "tbtools";
  tools.append(contextDrawerToggle);
  titlebar.append(lights, brand, project, laneSummary, tools);
  return { element: titlebar, contextDrawerToggle };
}
