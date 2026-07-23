import { translate, type Locale } from "../i18n/catalog";
import {
  renderProjectProbe,
  type ProjectProbeView,
} from "../components/project_probe";
import {
  renderProviderHealth,
  type ProviderHealthView,
} from "../components/provider_health";
import "./d11_intake.css";
import type { D4StarterSeed } from "./d4_lane_create";

export interface D11ConfigPreview {
  previewId: string;
  relativePath: string;
  contentSha256: string;
  exactContents: string | null;
  valid: boolean;
  diagnostics: string[];
}

export interface D11ConfirmedConfig {
  previewId: string;
  relativePath: string;
  contentSha256: string;
}

export interface D11IntakeProjection {
  project: ProjectProbeView | null;
  preview: D11ConfigPreview | null;
  confirmedConfig: D11ConfirmedConfig | null;
  provider: ProviderHealthView | null;
  credentialHandles: Array<{
    providerId: string;
    maskedHandle: string;
    status: string;
  }>;
  starterLanes: Array<{ id: string; role: string; status: string }>;
  pendingApproval: { id: string; title: string } | null;
  lastError: string | null;
  recentWork: { available: boolean; code: string; message: string };
  credentialIngress: { available: boolean; code: string; message: string };
  capabilities: {
    projectOnboarding: boolean;
    credentialHandles: boolean;
    laneLifecycle: boolean;
    starterLanePreview?: boolean;
  };
}

export interface D11IntentResult {
  projection: D11IntakeProjection;
  pendingCommandId: string | null;
  pendingIntent: D11PendingIntent | null;
}

export type D11PendingIntent =
  | "probe_project"
  | "preview_project_config"
  | "confirm_project_config"
  | "store_credential_handle";

export type D11Intent =
  | { type: "probe_project" }
  | { type: "preview_project_config"; contents: string }
  | { type: "confirm_project_config" };

export interface D11Draft {
  projectPath: string;
  configContents: string;
}

export interface D11Controller {
  draft: D11Draft;
}

type SendD11Intent = (intent: D11Intent) => Promise<D11IntentResult>;
type PollD11Intent = () => Promise<D11IntentResult>;
type RenderPendingPermission = (host: HTMLElement) => void;

export interface D11PendingState {
  commandId: string;
  intent: D11PendingIntent;
}

export interface D11RenderState {
  draft: D11Draft;
  pending: D11PendingState | null;
  connectionState?: "connected" | "disconnected";
}

const D11_POLL_DELAY_MS = 250;
const D11_MAX_POLL_DELAY_MS = 1_000;

function publishIntentResult(
  root: HTMLElement,
  result: D11IntentResult,
  send: SendD11Intent,
  locale: Locale,
  poll: PollD11Intent | undefined,
  draft: D11Draft,
  reviewStarterQueue?: (queue: readonly D4StarterSeed[]) => void,
  renderPendingPermission?: RenderPendingPermission,
  onExitToCockpit?: () => void,
): void {
  const pending =
    result.pendingCommandId && result.pendingIntent
      ? { commandId: result.pendingCommandId, intent: result.pendingIntent }
      : null;
  // Projection changes remain visible while the adapter retains command
  // identity; Accepted-only progress never becomes a GUI-owned success fact.
  renderD11Intake(
    root,
    result.projection,
    send,
    locale,
    poll,
    { draft, pending },
    reviewStarterQueue,
    renderPendingPermission,
    onExitToCockpit,
  );
}

export function draftFromD11Projection(projection: D11IntakeProjection): D11Draft {
  return {
    projectPath: projection.project?.root ?? "",
    configContents:
      projection.preview?.exactContents ??
      `[project]\nname = "${projection.project?.projectName ?? "project"}"\npack = "${projection.project?.mode ?? "rust"}"\n`,
  };
}

export function pendingFromD11Result(
  result: D11IntentResult,
): D11PendingState | null {
  return result.pendingCommandId && result.pendingIntent
    ? { commandId: result.pendingCommandId, intent: result.pendingIntent }
    : null;
}

function button(label: string, marker: string): HTMLButtonElement {
  const element = document.createElement("button");
  element.type = "button";
  element.className = "gbtn";
  element.textContent = label;
  element.dataset[marker] = "true";
  return element;
}

function step(label: string, index: number): HTMLElement {
  const item = document.createElement("div");
  item.className = "stp";
  const number = document.createElement("span");
  number.className = "nn";
  number.textContent = String(index);
  const text = document.createElement("span");
  text.className = "t1";
  text.textContent = label;
  item.append(number, text);
  return item;
}

export function renderD11Intake(
  root: HTMLElement,
  projection: D11IntakeProjection,
  send: SendD11Intent,
  locale: Locale,
  poll?: PollD11Intent,
  renderState?: D11RenderState,
  reviewStarterQueue?: (queue: readonly D4StarterSeed[]) => void,
  renderPendingPermission?: RenderPendingPermission,
  onExitToCockpit?: () => void,
): D11Controller {
  const draft: D11Draft =
    renderState?.draft ?? draftFromD11Projection(projection);
  const pending = renderState?.pending ?? null;
  const commandPending = pending !== null;
  let commandInFlight = commandPending;
  const commandButtons: Array<{
    element: HTMLButtonElement;
    enabled: () => boolean;
  }> = [];
  const registerCommandButton = (
    element: HTMLButtonElement,
    enabled: () => boolean,
  ) => {
    commandButtons.push({ element, enabled });
    element.disabled = commandInFlight || !enabled();
  };
  const setCommandButtonsDisabled = (disabled: boolean) => {
    for (const commandButton of commandButtons) {
      commandButton.element.disabled = disabled || !commandButton.enabled();
    }
  };
  const submitIntent = (
    intent: D11Intent,
    status: HTMLElement,
  ) => {
    if (commandInFlight) return;
    commandInFlight = true;
    status.textContent = translate(locale, "d11.waiting", {});
    setCommandButtonsDisabled(true);
    void send(intent)
      .then((result) => {
        publishIntentResult(
          root,
          result,
          send,
          locale,
          poll,
          draft,
          reviewStarterQueue,
          renderPendingPermission,
          onExitToCockpit,
        );
      })
      .catch((error: unknown) => {
        commandInFlight = false;
        status.textContent = String(error);
        setCommandButtonsDisabled(false);
      });
  };

  root.dataset.clientState = renderState?.connectionState ?? "connected";
  const screen = document.createElement("section");
  screen.className = "frame d11-frame";
  screen.dataset.screen = "d11-intake";

  const titlebar = document.createElement("header");
  titlebar.className = "vbar d11-titlebar";
  titlebar.dataset.tauriDragRegion = "true";
  const wordmark = document.createElement("span");
  wordmark.className = "wm";
  wordmark.textContent = "[◉] viden";
  const title = document.createElement("h1");
  title.textContent = translate(locale, "d11.intake", {});
  wordmark.dataset.tauriDragRegion = "true";
  title.dataset.tauriDragRegion = "true";
  titlebar.append(wordmark, title);

  const steps = document.createElement("nav");
  steps.className = "steps";
  steps.append(
    step(translate(locale, "d11.step.project", {}), 1),
    step(translate(locale, "d11.step.mode", {}), 2),
    step(translate(locale, "d11.step.config", {}), 3),
    step(translate(locale, "d11.step.lanes", {}), 4),
  );

  const content = document.createElement("main");
  content.className = "d11-content";
  const pathLabel = document.createElement("label");
  pathLabel.textContent = translate(locale, "d11.projectPath", {});
  const repoBar = document.createElement("div");
  repoBar.className = "repobar";
  const pathInput = document.createElement("input");
  pathInput.dataset.projectPath = "true";
  pathInput.autocomplete = "off";
  pathInput.value = draft.projectPath;
  pathInput.addEventListener("input", () => {
    draft.projectPath = pathInput.value;
  });
  const probe = button(translate(locale, "d11.probe", {}), "probeProject");
  const probeState = document.createElement("p");
  probeState.dataset.probeState = "true";
  probeState.setAttribute("role", "status");
  probeState.setAttribute("aria-live", "polite");
  registerCommandButton(probe, () => projection.capabilities.projectOnboarding);
  if (pending?.intent === "probe_project") {
    probeState.textContent = translate(locale, "d11.waiting", {});
  }
  probe.addEventListener("click", () => {
    submitIntent({ type: "probe_project" }, probeState);
  });
  repoBar.append(pathInput, probe);
  pathLabel.append(repoBar);
  content.append(pathLabel, probeState, renderProjectProbe(projection.project, locale));

  const bootstrapGap = document.createElement("p");
  bootstrapGap.className = "d11-unavailable";
  bootstrapGap.dataset.projectBootstrap = "unavailable";
  bootstrapGap.textContent = translate(locale, "d11.projectBootstrapUnavailable", {});
  content.append(bootstrapGap, renderProviderHealth(projection.provider, locale));

  const configLabel = document.createElement("label");
  configLabel.textContent = "viden.toml";
  const configDraft = document.createElement("textarea");
  configDraft.dataset.configDraft = "true";
  configDraft.value = draft.configContents;
  configDraft.addEventListener("input", () => {
    draft.configContents = configDraft.value;
  });
  configLabel.append(configDraft);
  const preview = button(translate(locale, "d11.preview", {}), "previewConfig");
  const previewState = document.createElement("p");
  previewState.dataset.previewState = "true";
  previewState.setAttribute("role", "status");
  previewState.setAttribute("aria-live", "polite");
  registerCommandButton(preview, () => projection.capabilities.projectOnboarding);
  if (pending?.intent === "preview_project_config") {
    previewState.textContent = translate(locale, "d11.waiting", {});
  }
  preview.addEventListener("click", () => {
    submitIntent(
      {
        type: "preview_project_config",
        contents: draft.configContents,
      },
      previewState,
    );
  });
  content.append(configLabel, preview, previewState);

  const laneState = document.createElement("p");
  laneState.dataset.starterState = "true";
  laneState.setAttribute("role", "status");
  laneState.setAttribute("aria-live", "polite");
  const starter = button(translate(locale, "d11.lane.create", {}), "createStarterLane");
  starter.classList.add("primary");
  const presetFieldset = document.createElement("fieldset");
  presetFieldset.className = "d11-starter-presets";
  const presetLegend = document.createElement("legend");
  presetLegend.textContent = translate(locale, "d11.step.lanes", {});
  presetFieldset.append(presetLegend);
  const selectedPresets = new Set<D4StarterSeed["preset"]>(["coder"]);
  for (const preset of ["coder", "reviewer", "tester"] as const) {
    const label = document.createElement("label");
    const input = document.createElement("input");
    input.type = "checkbox";
    input.checked = preset === "coder";
    input.dataset.starterPreset = preset;
    input.addEventListener("change", () => {
      if (input.checked) selectedPresets.add(preset);
      else selectedPresets.delete(preset);
      starter.disabled = !projection.confirmedConfig || selectedPresets.size === 0;
    });
    label.append(input, document.createTextNode(translate(locale, `d4.preset.${preset}`, {})));
    presetFieldset.append(label);
  }
  starter.disabled = !projection.confirmedConfig;
  starter.addEventListener("click", () => {
    if (!projection.confirmedConfig || selectedPresets.size === 0) return;
    const queue = (["coder", "reviewer", "tester"] as const)
      .filter((preset) => selectedPresets.has(preset))
      .map((preset) => ({
        laneId: `starter-${preset}`,
        preset,
        branch: null,
        worktreePath: null,
      }));
    reviewStarterQueue?.(queue);
  });
  if (projection.starterLanes.length > 0) {
    laneState.textContent = translate(locale, "d11.lane.ready", {});
  }
  content.append(presetFieldset, starter, laneState);

  appendSafeFacts(content, projection, locale);

  const tomlRail = document.createElement("aside");
  tomlRail.className = "tomlrail";
  const railTitle = document.createElement("h2");
  railTitle.textContent = projection.preview?.relativePath ?? "viden.toml";
  const code = document.createElement("pre");
  code.dataset.configPreview = "true";
  code.textContent = projection.preview?.exactContents ?? "";
  const confirmState = document.createElement("p");
  confirmState.dataset.confirmState = "true";
  confirmState.setAttribute("role", "status");
  confirmState.setAttribute("aria-live", "polite");
  if (projection.confirmedConfig && pending?.intent !== "confirm_project_config") {
    confirmState.textContent = translate(locale, "d11.confirmed", {});
  }
  if (pending?.intent === "confirm_project_config") {
    confirmState.textContent = translate(locale, "d11.waiting", {});
  }
  if (projection.pendingApproval) {
    const approval = document.createElement("p");
    approval.dataset.coreApproval = projection.pendingApproval.id;
    approval.setAttribute("role", "status");
    approval.textContent = projection.pendingApproval.title;
    tomlRail.append(approval);
    if (renderPendingPermission) {
      const permissionHost = document.createElement("div");
      permissionHost.dataset.d11PermissionHost = "true";
      tomlRail.append(permissionHost);
      renderPendingPermission(permissionHost);
    }
  }
  if (projection.lastError) {
    const error = document.createElement("p");
    error.dataset.coreError = "true";
    error.setAttribute("role", "alert");
    error.textContent = projection.lastError;
    tomlRail.append(error);
  }
  const confirm = button(translate(locale, "d11.confirm", {}), "confirmConfig");
  confirm.classList.add("primary");
  registerCommandButton(confirm, () => Boolean(projection.preview?.valid));
  confirm.addEventListener("click", () => {
    submitIntent({ type: "confirm_project_config" }, confirmState);
  });
  tomlRail.append(railTitle, code, confirm, confirmState);

  const footer = document.createElement("footer");
  footer.className = "wfoot";
  const cancel = button(translate(locale, "d11.cancel", {}), "cancelIntake");
  cancel.addEventListener("click", () => {
    if (onExitToCockpit) {
      onExitToCockpit();
      return;
    }
    draft.projectPath = "";
    draft.configContents = "";
    pathInput.value = "";
    configDraft.value = "";
    confirmState.textContent = "";
    laneState.textContent = "";
    pathInput.focus();
  });
  footer.append(cancel);

  const layout = document.createElement("div");
  layout.className = "d11-layout";
  layout.append(steps, content, tomlRail);
  screen.append(titlebar, layout, footer);
  root.replaceChildren(screen);
  pathInput.focus();

  if (pending && poll) {
    const pendingStatus = () =>
      pending.intent === "probe_project"
        ? probeState
        : pending.intent === "preview_project_config"
          ? previewState
          : confirmState;
    const schedulePoll = (delay: number) => {
      window.setTimeout(() => {
        if (!root.contains(screen)) return;
        void poll()
          .then((result) => {
            if (root.contains(screen)) {
              publishIntentResult(
                root,
                result,
                send,
                locale,
                poll,
                draft,
                reviewStarterQueue,
                renderPendingPermission,
                onExitToCockpit,
              );
            }
          })
          .catch((error: unknown) => {
            if (!root.contains(screen)) return;
            pendingStatus().textContent = String(error);
            schedulePoll(Math.min(delay * 2, D11_MAX_POLL_DELAY_MS));
          });
      }, delay);
    };
    schedulePoll(D11_POLL_DELAY_MS);
  }
  return { draft };
}

function appendSafeFacts(
  content: HTMLElement,
  projection: D11IntakeProjection,
  locale: Locale,
): void {
  const credential = document.createElement("section");
  credential.className = "d11-safe-facts";
  const credentialTitle = document.createElement("h2");
  credentialTitle.textContent = translate(locale, "d11.credential", {});
  credential.append(credentialTitle);
  for (const handle of projection.credentialHandles) {
    const row = document.createElement("p");
    row.textContent = `${handle.providerId} · ${handle.maskedHandle} · ${handle.status}`;
    credential.append(row);
  }
  const ingress = button(projection.credentialIngress.message, "credentialIngress");
  ingress.title = projection.credentialIngress.code;
  ingress.disabled = !projection.credentialIngress.available;
  credential.append(ingress);

  const recent = document.createElement("section");
  recent.className = "d11-safe-facts";
  const recentTitle = document.createElement("h2");
  recentTitle.textContent = translate(locale, "d11.history", {});
  const recentStatus = document.createElement("p");
  recentStatus.textContent = projection.recentWork.message;
  recentStatus.title = projection.recentWork.code;
  recent.append(recentTitle, recentStatus);
  content.append(credential, recent);
}
