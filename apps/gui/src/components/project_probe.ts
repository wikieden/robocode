import { translate, type Locale } from "../i18n/catalog";

export interface ProjectProbeView {
  root: string;
  isGitRepository: boolean;
  configState: "missing" | "valid" | "invalid";
  projectName: string | null;
  mode: string | null;
  diagnostics: string[];
}

export function renderProjectProbe(
  project: ProjectProbeView | null,
  locale: Locale,
): HTMLElement {
  const container = document.createElement("div");
  container.className = "detect";
  container.dataset.projectProbe = project ? "confirmed" : "missing";

  const summary = document.createElement("p");
  summary.className = "d11-summary";
  summary.textContent = project?.root ?? translate(locale, "d11.noProject", {});
  container.append(summary);

  if (project?.mode) {
    const mode = document.createElement("p");
    mode.dataset.projectMode = project.mode;
    mode.textContent = translate(locale, "d11.mode", { mode: project.mode });
    container.append(mode);
  }

  for (const diagnostic of project?.diagnostics ?? []) {
    const row = document.createElement("p");
    row.className = "d11-diagnostic";
    row.textContent = diagnostic;
    container.append(row);
  }
  return container;
}
