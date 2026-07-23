export interface LaneWorkSurfaceParts {
  work: HTMLElement;
  permission: HTMLElement;
  composer: HTMLElement;
  showWelcome: boolean;
}

/**
 * Owns presentation geometry only. Runtime facts remain projected by Core and
 * the screen orchestrator supplies the already-rendered regions.
 */
export function renderLaneWorkSurface(parts: LaneWorkSurfaceParts): HTMLElement {
  const surface = document.createElement("main");
  surface.className = "d1-main d1-lane-work-surface";
  surface.dataset.shellLandmark = "lane-work-surface";
  surface.dataset.cockpitRole = "work";
  surface.dataset.laneWorkSurface = "true";
  if (parts.showWelcome) surface.classList.add("d1-main-welcome");

  parts.work.dataset.workSurface = "true";
  surface.append(parts.work);
  if (!parts.showWelcome) {
    parts.permission.dataset.permissionRegion = "true";
    parts.composer.dataset.composerRegion = "true";
    surface.append(parts.permission, parts.composer);
  }
  return surface;
}
