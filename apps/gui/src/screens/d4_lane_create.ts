import { translate, type Locale } from "../i18n/catalog";
import "./d4_lane_create.css";

export type D4Preset = "coder" | "reviewer" | "tester";

export interface D4StarterSeed {
  laneId: string;
  preset: D4Preset;
  branch: string | null;
  worktreePath: string | null;
}

interface D4ResolvedLane {
  id: string;
  role: string;
  route: string;
  gateStrength: string;
  mutationPolicy: string;
  worktree: string | null;
  branch: string | null;
  target: string;
  dataEgress: string;
  status: string;
  budget: {
    tokenLimit: number | null;
    costLimitMicroUsd: number | null;
    wallTimeLimitSecs: number | null;
  };
  summary: string;
}

export interface D4Preview {
  previewId: string;
  contentSha256: string;
  owner: {
    workspaceId: string;
    projectId: string;
    laneId: string | null;
    sessionId: string | null;
    taskId: string | null;
    turnId: string | null;
  };
  lane: D4ResolvedLane;
  branch: string;
  worktreePath: string;
  baseRevision: string;
  diagnostics: string[];
}

export interface D4LaneCreateProjection {
  availability: { available: boolean; capability: string; message: string };
  workMode: string;
  canCreate: boolean;
  preview: D4Preview | null;
  receipt: D4Preview | null;
  pendingApproval: { id: string; title: string; risk: string; target: string } | null;
  outcome: { state: string; reason: string | null; requiresRepreview: boolean };
  navigationLaneId: string | null;
}

export type D4Intent =
  | { type: "preview"; request: D4StarterSeed }
  | { type: "create"; request: D4StarterSeed }
  | {
      type: "respond_to_approval";
      requestId: string;
      decision: "allow_once" | "deny";
    };

export interface D4IntentResult {
  projection: D4LaneCreateProjection;
  pendingCommandId: string | null;
  pendingIntent: "preview_starter_lane" | "create_starter_lane" | null;
}

export interface D4QueueState {
  queue: readonly D4StarterSeed[];
  queueIndex: number;
  completedLaneIds: string[];
  onCancel: () => void;
  onNavigateToD1: (laneId: string) => void;
}

export interface D4Controller {
  state: { draft: D4StarterSeed; step: number };
  applyResult: (result: D4IntentResult) => Promise<void>;
}

type SendD4Intent = (intent: D4Intent) => Promise<D4IntentResult>;
type PollD4Intent = () => Promise<D4IntentResult>;

const PRESETS: readonly D4Preset[] = ["coder", "reviewer", "tester"];

function action(label: string, marker: string): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "gbtn";
  button.textContent = label;
  button.dataset[marker] = "true";
  return button;
}

function cloneSeed(seed: D4StarterSeed): D4StarterSeed {
  return { ...seed };
}

function requestKey(seed: D4StarterSeed): string {
  return JSON.stringify(seed);
}

export function renderD4LaneCreate(
  root: HTMLElement,
  initial: D4IntentResult,
  send: SendD4Intent,
  poll: PollD4Intent,
  locale: Locale,
  queueState: D4QueueState,
): D4Controller {
  let projection = initial.projection;
  let pendingCommandId = initial.pendingCommandId;
  let pendingIntent = initial.pendingIntent;
  let queueIndex = queueState.queueIndex;
  const completedLaneIds = [...queueState.completedLaneIds];
  const firstSeed = queueState.queue[queueIndex] ?? {
    laneId: "starter-coder",
    preset: "coder" as const,
    branch: null,
    worktreePath: null,
  };
  const state = { draft: cloneSeed(firstSeed), step: 0 };
  let reviewedRequestKey = projection.preview ? requestKey(state.draft) : null;
  let sending = false;
  let pollTimer: number | null = null;

  const advanceReceipt = (): boolean => {
    const laneId = projection.navigationLaneId;
    if (!laneId || completedLaneIds.includes(laneId)) return false;
    completedLaneIds.push(laneId);
    queueIndex += 1;
    if (queueIndex >= queueState.queue.length) {
      queueState.onNavigateToD1(laneId);
      return true;
    }
    state.draft = cloneSeed(queueState.queue[queueIndex]!);
    state.step = 0;
    reviewedRequestKey = null;
    projection = {
      ...projection,
      canCreate: false,
      preview: null,
      receipt: null,
      pendingApproval: null,
      navigationLaneId: null,
      outcome: { state: "idle", reason: null, requiresRepreview: false },
    };
    pendingCommandId = null;
    pendingIntent = null;
    return false;
  };

  const controller: D4Controller = {
    state,
    applyResult: async (result) => {
      projection = result.projection;
      pendingCommandId = result.pendingCommandId;
      pendingIntent = result.pendingIntent;
      if (projection.preview && pendingIntent !== "preview_starter_lane") {
        reviewedRequestKey = requestKey(state.draft);
      }
      const navigated = advanceReceipt();
      if (!navigated) render();
    },
  };

  const submit = (intent: D4Intent): void => {
    if (sending) return;
    sending = true;
    render();
    void send(intent)
      .then(controller.applyResult)
      .catch((error: unknown) => {
        sending = false;
        projection = {
          ...projection,
          outcome: {
            state: "rejected",
            reason: String(error),
            requiresRepreview: intent.type !== "preview",
          },
        };
        render();
      })
      .finally(() => {
        sending = false;
      });
  };

  const schedulePoll = (): void => {
    if (!pendingCommandId || pollTimer !== null) return;
    pollTimer = window.setTimeout(() => {
      pollTimer = null;
      void poll()
        .then(controller.applyResult)
        .catch(() => schedulePoll());
    }, 250);
  };

  const render = (): void => {
    const requestChanged =
      reviewedRequestKey !== null && reviewedRequestKey !== requestKey(state.draft);
    const requiresRepreview = projection.outcome.requiresRepreview || requestChanged;
    const creating = pendingIntent === "create_starter_lane";

    const frame = document.createElement("section");
    frame.className = "frame d4-frame";
    frame.dataset.screen = "d4-lane-create";

    const titlebar = document.createElement("header");
    titlebar.className = "vbar d4-titlebar";
    titlebar.dataset.tauriDragRegion = "true";
    const title = document.createElement("h1");
    title.textContent = translate(locale, "d4.title", {});
    title.dataset.tauriDragRegion = "true";
    titlebar.append(title);

    const layout = document.createElement("div");
    layout.className = "d4-layout";
    const navigation = document.createElement("nav");
    navigation.className = "d4-steps";
    navigation.setAttribute("aria-label", translate(locale, "d4.title", {}));
    const stepList = document.createElement("ol");
    const stepKeys = [
      "d4.step.role",
      "d4.step.runtime",
      "d4.step.gates",
      "d4.step.review",
    ] as const;
    stepKeys.forEach((key, index) => {
      const item = document.createElement("li");
      const button = action(`${index + 1}. ${translate(locale, key, {})}`, "d4Step");
      button.dataset.d4Step = String(index);
      if (state.step === index) button.setAttribute("aria-current", "step");
      button.addEventListener("click", () => {
        state.step = index;
        render();
        root.querySelector<HTMLElement>("[data-step-heading]")?.focus();
      });
      item.append(button);
      stepList.append(item);
    });
    navigation.append(stepList);

    const form = document.createElement("section");
    form.className = "d4-form";
    form.tabIndex = -1;
    const heading = document.createElement("h2");
    heading.tabIndex = -1;
    heading.dataset.stepHeading = "true";
    heading.textContent = translate(locale, stepKeys[state.step]!, {});
    form.append(heading);

    const laneLabel = document.createElement("label");
    laneLabel.textContent = translate(locale, "d4.laneId", {});
    const laneId = document.createElement("input");
    laneId.dataset.laneId = "true";
    laneId.value = state.draft.laneId;
    laneId.autocomplete = "off";
    laneId.addEventListener("input", () => {
      state.draft.laneId = laneId.value;
      render();
    });
    laneLabel.append(laneId);

    const branchLabel = document.createElement("label");
    branchLabel.textContent = translate(locale, "d4.branch", {});
    const branch = document.createElement("input");
    branch.dataset.branch = "true";
    branch.value = state.draft.branch ?? "";
    branch.autocomplete = "off";
    branch.addEventListener("input", () => {
      state.draft.branch = branch.value || null;
      render();
    });
    branchLabel.append(branch);

    const roleGroup = document.createElement("div");
    roleGroup.className = "d4-role-group";
    roleGroup.setAttribute("role", "radiogroup");
    roleGroup.setAttribute("aria-label", translate(locale, "d4.role", {}));
    PRESETS.forEach((preset, index) => {
      const role = action(translate(locale, `d4.preset.${preset}`, {}), "preset");
      role.setAttribute("role", "radio");
      role.dataset.preset = preset;
      role.setAttribute("aria-checked", String(state.draft.preset === preset));
      role.tabIndex = state.draft.preset === preset ? 0 : -1;
      const choose = () => {
        state.draft.preset = preset;
        if (state.draft.laneId.startsWith("starter-")) {
          state.draft.laneId = `starter-${preset}`;
        }
        render();
      };
      role.addEventListener("click", choose);
      role.addEventListener("keydown", (event) => {
        if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) return;
        event.preventDefault();
        const offset = event.key === "ArrowRight" || event.key === "ArrowDown" ? 1 : -1;
        state.draft.preset = PRESETS[(index + offset + PRESETS.length) % PRESETS.length]!;
        if (state.draft.laneId.startsWith("starter-")) {
          state.draft.laneId = `starter-${state.draft.preset}`;
        }
        render();
        root.querySelector<HTMLElement>(`[data-preset="${state.draft.preset}"]`)?.focus();
      });
      roleGroup.append(role);
    });
    form.append(laneLabel, branchLabel, roleGroup);

    const resolved = document.createElement("dl");
    resolved.className = "d4-resolved";
    const addResolved = (key: string, value: string, marker: string) => {
      const term = document.createElement("dt");
      term.textContent = key;
      const detail = document.createElement("dd");
      detail.textContent = value;
      detail.dataset[marker] = "true";
      resolved.append(term, detail);
    };
    addResolved(translate(locale, "d4.route", {}), projection.preview?.lane.route ?? "—", "resolvedRoute");
    addResolved(translate(locale, "d4.gate", {}), projection.preview?.lane.gateStrength ?? "—", "resolvedGate");
    addResolved(translate(locale, "d4.target", {}), projection.preview?.lane.target ?? "—", "resolvedTarget");
    addResolved(translate(locale, "d4.budget.default", {}), translate(locale, "d4.budget.default", {}), "resolvedBudget");
    addResolved(translate(locale, "d4.worktree", {}), projection.preview?.worktreePath ?? "—", "resolvedWorktree");
    addResolved(translate(locale, "d4.base", {}), projection.preview?.baseRevision ?? "—", "resolvedBase");
    form.append(resolved);

    const rail = document.createElement("aside");
    rail.className = "d4-summary";
    const summaryTitle = document.createElement("h2");
    summaryTitle.textContent = state.draft.laneId;
    const summary = document.createElement("p");
    summary.textContent = projection.preview?.lane.summary ?? translate(locale, "d4.preview", {});
    rail.append(summaryTitle, summary);

    if (!projection.availability.available) {
      const unavailable = document.createElement("p");
      unavailable.setAttribute("role", "alert");
      unavailable.textContent = `${projection.availability.capability} · ${projection.availability.message}`;
      rail.append(unavailable);
    }
    if (requiresRepreview) {
      const warning = document.createElement("p");
      warning.dataset.repreviewRequired = "true";
      warning.setAttribute("role", "alert");
      warning.textContent = projection.outcome.reason
        ? `${translate(locale, "d4.repreview", {})} ${projection.outcome.reason}`
        : translate(locale, "d4.repreview", {});
      rail.append(warning);
    }
    if (projection.pendingApproval) {
      const approval = document.createElement("section");
      approval.className = "d4-approval";
      approval.setAttribute("role", "status");
      approval.textContent = projection.pendingApproval.title;
      const allow = action(translate(locale, "d4.approval.allow", {}), "allowStarterApproval");
      allow.addEventListener("click", () => submit({
        type: "respond_to_approval",
        requestId: projection.pendingApproval!.id,
        decision: "allow_once",
      }));
      const deny = action(translate(locale, "d4.approval.deny", {}), "denyStarterApproval");
      deny.addEventListener("click", () => submit({
        type: "respond_to_approval",
        requestId: projection.pendingApproval!.id,
        decision: "deny",
      }));
      approval.append(allow, deny);
      rail.append(approval);
    }
    if (creating) {
      const waiting = document.createElement("p");
      waiting.dataset.createWaiting = "true";
      waiting.setAttribute("role", "status");
      waiting.setAttribute("aria-live", "polite");
      waiting.textContent = translate(locale, "d4.waiting", {});
      rail.append(waiting);
    }

    layout.append(navigation, form, rail);

    const footer = document.createElement("footer");
    footer.className = "wfoot d4-footer";
    if (!creating) {
      const cancel = action(translate(locale, "d4.cancel", {}), "cancelD4");
      cancel.addEventListener("click", queueState.onCancel);
      const skip = action(translate(locale, "d4.skip", {}), "skipD4");
      skip.addEventListener("click", queueState.onCancel);
      footer.append(cancel, skip);
    }
    if (state.step > 0) {
      const previous = action(translate(locale, "d4.back", {}), "previousD4Step");
      previous.addEventListener("click", () => {
        state.step -= 1;
        render();
        root.querySelector<HTMLElement>("[data-step-heading]")?.focus();
      });
      footer.append(previous);
    }
    if (state.step < 3) {
      const next = action(
        translate(locale, stepKeys[state.step + 1]!, {}),
        "nextD4Step",
      );
      next.addEventListener("click", () => {
        state.step += 1;
        render();
        root.querySelector<HTMLElement>("[data-step-heading]")?.focus();
      });
      footer.append(next);
    }
    const preview = action(translate(locale, "d4.preview", {}), "previewStarterLane");
    preview.disabled =
      !projection.availability.available || !state.draft.laneId.trim() || sending || creating;
    preview.addEventListener("click", () => {
      if (!preview.disabled) submit({ type: "preview", request: cloneSeed(state.draft) });
    });
    const create = action(translate(locale, "d4.create", {}), "createStarterLane");
    create.classList.add("primary");
    create.disabled =
      !projection.canCreate || requiresRepreview || sending || pendingCommandId !== null;
    create.addEventListener("click", () => {
      if (!create.disabled) submit({ type: "create", request: cloneSeed(state.draft) });
    });
    footer.append(preview, create);
    if (projection.workMode === "plan") {
      const reason = document.createElement("p");
      reason.dataset.createDisabledReason = "true";
      reason.textContent = translate(locale, "d4.create.planDisabled", {});
      footer.append(reason);
    }

    frame.addEventListener("keydown", (event) => {
      if (event.key === "Escape" && !creating) {
        event.preventDefault();
        queueState.onCancel();
        return;
      }
      if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        if (!create.disabled) create.click();
        return;
      }
      if (
        event.key === "Enter" &&
        state.step < 3 &&
        !(event.target instanceof HTMLButtonElement)
      ) {
        event.preventDefault();
        state.step += 1;
        render();
        root.querySelector<HTMLElement>("[data-step-heading]")?.focus();
      }
    });

    frame.append(titlebar, layout, footer);
    root.replaceChildren(frame);
    if (document.activeElement === document.body || !root.contains(document.activeElement)) {
      laneId.focus();
    }
    schedulePoll();
  };

  const alreadyNavigated = advanceReceipt();
  if (!alreadyNavigated) render();
  return controller;
}
