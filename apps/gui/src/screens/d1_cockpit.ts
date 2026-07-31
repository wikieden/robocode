import { translate } from "../i18n/catalog";
import { renderActivityRail } from "../components/activity_rail";
import {
  orderedAgentAdapters,
  renderAgentMenu,
  type AgentMenuController,
  type AgentMenuSelection,
} from "../components/agent_menu";
import { renderCockpitTopbar } from "../components/cockpit_topbar";
import { shouldRouteComposerMutation, shouldSubmitComposer } from "../components/composer";
import { renderContextDock } from "../components/context_dock";
import { renderLaneRail } from "../components/lane_rail";
import { renderLaneWorkSurface } from "../components/lane_work_surface";
import {
  renderPermissionDock,
  type PermissionIntent,
} from "../components/permission_dock";
import { appendTranscriptRows, transcriptAtBottom } from "../components/transcript";
import { appendTypedWorkCards } from "../components/tool_row";
import { renderLiveWorkBar } from "../components/live_work";
import { renderWelcomeCenter } from "../components/welcome_center";
import type { D1Intent } from "../models/composer";
import { BoundedTranscript } from "../models/transcript";
import type { D1CockpitProjection, D6RecoveryProjection } from "../models/workspace";
import { renderD6Recovery } from "./d6_recovery";
import "./d1_cockpit.css";

export type { D1Intent } from "../models/composer";
export { BoundedTranscript } from "../models/transcript";
export type { D1CockpitProjection } from "../models/workspace";

export interface D1IntentResult {
  projection: D1CockpitProjection;
  pendingCommandId: string | null;
  outcome: {
    state: "idle" | "pending" | "confirmed" | "rejected";
    reason: string | null;
  };
}

type SendD1Intent = (intent: D1Intent) => Promise<D1IntentResult>;
type PollD1 = (selectedLaneId?: string, waitForEvent?: boolean) => Promise<D1IntentResult>;
type SendPermissionIntent = (intent: PermissionIntent) => Promise<unknown>;
type RecoverD6 = () => Promise<D6RecoveryProjection>;

export interface D1Controller {
  applyProjection: (projection: D1CockpitProjection) => void;
  applyResult: (result: D1IntentResult) => void;
  transcript: BoundedTranscript;
  dispose: () => void;
}

export interface D1RenderOptions {
  onOpenProject?: () => void | Promise<void>;
  onCreateLane?: () => void;
  showWelcome?: boolean;
  poll?: boolean;
}

type FocusedConversation =
  | { kind: "native"; laneId: string }
  | { kind: "acp"; laneId: string; sessionId: string };

function conversationForLane(
  projection: D1CockpitProjection,
  laneId: string | null,
): FocusedConversation | null {
  if (!laneId || projection.contextDock.laneAgent?.laneId !== laneId) return null;
  const sessions = projection.agentSessions.filter((candidate) => candidate.laneId === laneId);
  if (sessions.length > 1) return null;
  const session = sessions[0];
  if (session && session.sessionId !== projection.contextDock.laneAgent.sessionId) return null;
  return session
    ? { kind: "acp", laneId, sessionId: session.sessionId }
    : { kind: "native", laneId };
}

function composerMutationBlockReason(
  projection: D1CockpitProjection,
  laneId: string | null,
):
  | "d1.mutation.noOwner"
  | "d1.mutation.duplicateSession"
  | "d1.mutation.ownerMismatch"
  | "d1.mutation.staleLane"
  | null {
  if (!laneId || !projection.lanes.some((lane) => lane.id === laneId)) {
    return "d1.mutation.staleLane";
  }
  if (projection.contextDock.laneAgent?.laneId !== laneId) {
    return "d1.mutation.noOwner";
  }
  const sessions = projection.agentSessions.filter((session) => session.laneId === laneId);
  if (sessions.length > 1) {
    return "d1.mutation.duplicateSession";
  }
  if (sessions[0] && sessions[0].sessionId !== projection.contextDock.laneAgent.sessionId) {
    return "d1.mutation.ownerMismatch";
  }
  return null;
}

function button(label: string, marker?: string): HTMLButtonElement {
  const element = document.createElement("button");
  element.type = "button";
  element.className = "d1-action";
  element.textContent = label;
  if (marker) element.dataset[marker] = "true";
  return element;
}

function appendUnavailableTranscriptRows(
  root: HTMLElement,
  unavailableFeatures: D1CockpitProjection["unavailableFeatures"],
  locale: D1CockpitProjection["preferences"]["locale"],
): void {
  for (const kind of ["user", "assistant"] as const) {
    const feature = unavailableFeatures.find((candidate) => candidate.id === `transcript_${kind}`);
    if (!feature) continue;
    const placeholder = document.createElement("article");
    placeholder.dataset.centerStep = kind;
    placeholder.dataset.typedEmpty = `transcript-${kind}`;
    placeholder.textContent = translate(
      locale,
      kind === "user" ? "d1.transcript.userUnavailable" : "d1.transcript.assistantUnavailable",
      {},
    );
    placeholder.title = feature.code;
    root.append(placeholder);
  }
}

export function renderD1Cockpit(
  root: HTMLElement,
  initial: D1CockpitProjection,
  send: SendD1Intent,
  poll: PollD1,
  sendPermission?: SendPermissionIntent,
  recoverD6?: RecoverD6,
  options: D1RenderOptions = {},
): D1Controller {
  let projection = initial;
  let selectedLaneId = initial.selectedLaneId;
  let focusedConversation = conversationForLane(initial, initial.selectedLaneId);
  let transcriptLaneId = initial.selectedLaneId;
  let projectionKey = JSON.stringify(initial);
  let locale = initial.preferences.locale;
  let draft = "";
  let composing = false;
  let disposed = false;
  let pollTimer: number | null = null;
  let pollInFlight = false;
  let sending = false;
  let pendingCommandId: string | null = null;
  let submittedDraft: string | null = null;
  let errorMessage: string | null = null;
  let contextDrawerOpen = false;
  let laneRailOpen = false;
  let laneRailFocusTarget: "rail" | "toggle" | null = null;
  let menuController: AgentMenuController | null = null;
  let agentMenuOpen = false;
  let remountingAgentMenu = false;
  let newLaneSelection: AgentMenuSelection | undefined;
  let pendingAgentTaskDraft = "";
  let creatingLane = false;
  const commandSlotWaiters: Array<() => void> = [];
  let pendingLaneStart:
    | { laneId: string; task: string; agentId: string | null }
    | null = null;
  let agentDiscoveryStarted = false;
  let agentDiscoveryComplete = true;
  let agentQueryComplete = false;
  let discoveryDispatching = false;
  let discoveryInFlight:
    | { kind: "query" }
    | { kind: "probe"; agentId: string }
    | null = null;
  const attemptedAgentProbes = new Set<string>();
  const transcript = new BoundedTranscript(240);
  transcript.replace(initial.transcript);

  const shouldShowWelcome = (): boolean =>
    options.showWelcome ??
    (projection.recovery.state === "empty" && !!options.onOpenProject && !options.onCreateLane);

  const handleWindowKeydown = (event: KeyboardEvent): void => {
    if (
      !shouldShowWelcome() ||
      event.repeat ||
      (!event.metaKey && !event.ctrlKey) ||
      event.key.toLowerCase() !== "o"
    ) {
      return;
    }
    event.preventDefault();
    root.querySelector<HTMLButtonElement>("[data-open-project]")?.click();
  };
  window.addEventListener("keydown", handleWindowKeydown);
  const handleWindowResize = (): void => {
    const grid = root.querySelector<HTMLElement>("[data-cockpit-grid]");
    if (grid) grid.dataset.cockpitLayout = window.innerWidth <= 1100 ? "narrow" : "desktop";
  };
  window.addEventListener("resize", handleWindowResize);

  const controller: D1Controller = {
    transcript,
    applyProjection: (next) => {
      if (disposed) return;
      const nextKey = JSON.stringify(next);
      if (nextKey === projectionKey) return;
      projection = next;
      if (!selectedLaneId) selectedLaneId = next.selectedLaneId;
      focusedConversation = conversationForLane(next, selectedLaneId);
      projectionKey = nextKey;
      locale = next.preferences.locale;
      if (next.selectedLaneId === selectedLaneId) {
        if (transcriptLaneId !== selectedLaneId) transcript.reset(next.transcript);
        else transcript.replace(next.transcript);
        transcriptLaneId = selectedLaneId;
      }
      render(false);
      queueMicrotask(maybeResumeLaneStart);
    },
    applyResult: (result) => {
      if (disposed) return;
      const previousPending = pendingCommandId;
      pendingCommandId = result.pendingCommandId;
      if (result.outcome.state === "confirmed" && submittedDraft !== null) {
        // Do not erase edits typed after the submitted content entered pending state.
        if (draft === submittedDraft) draft = "";
        submittedDraft = null;
        errorMessage = null;
      } else if (result.outcome.state === "rejected") {
        submittedDraft = null;
        errorMessage = result.outcome.reason ?? "Core rejected the command.";
        if (
          previousPending &&
          pendingLaneStart &&
          !result.projection.lanes.some((lane) => lane.id === pendingLaneStart?.laneId)
        ) {
          pendingLaneStart = null;
        }
      }
      const nextKey = JSON.stringify(result.projection);
      const projectionChanged = nextKey !== projectionKey;
      if (projectionChanged) {
        projection = result.projection;
        if (!selectedLaneId) selectedLaneId = result.projection.selectedLaneId;
        focusedConversation = conversationForLane(result.projection, selectedLaneId);
        projectionKey = nextKey;
        locale = result.projection.preferences.locale;
        if (result.projection.selectedLaneId === selectedLaneId) {
          if (transcriptLaneId !== selectedLaneId) transcript.reset(result.projection.transcript);
          else transcript.replace(result.projection.transcript);
          transcriptLaneId = selectedLaneId;
        }
      }
      if (pendingLaneStart && result.projection.permissionDock.request) {
        // The pre-registration Lane approval belongs to the center dock. Keep
        // its primary actions mouse-operable by dismissing the floating rail
        // that launched the New Lane overlay.
        laneRailOpen = false;
        laneRailFocusTarget = null;
      }
      const discoveryChanged = observeAgentDiscoveryResult(result);
      if (
        projectionChanged ||
        previousPending !== pendingCommandId ||
        result.outcome.state === "confirmed" ||
        result.outcome.state === "rejected" ||
        discoveryChanged
      ) {
        render(false);
      }
      queueMicrotask(maybeResumeLaneStart);
    },
    dispose: () => {
      disposed = true;
      agentMenuOpen = false;
      menuController?.close();
      if (pollTimer !== null) window.clearTimeout(pollTimer);
      commandSlotWaiters.splice(0).forEach((resolve) => resolve());
      window.removeEventListener("keydown", handleWindowKeydown);
      window.removeEventListener("resize", handleWindowResize);
    },
  };

  const commandSlotAvailable = (): boolean =>
    !sending &&
    !pendingCommandId &&
    !discoveryDispatching &&
    !discoveryInFlight &&
    !pollInFlight;

  const releaseCommandSlotWaiters = (): void => {
    if (!commandSlotAvailable()) return;
    commandSlotWaiters.splice(0).forEach((resolve) => resolve());
  };

  const waitForCommandSlot = async (): Promise<boolean> => {
    if (!commandSlotAvailable()) {
      await new Promise<void>((resolve) => commandSlotWaiters.push(resolve));
    }
    return !disposed;
  };

  const schedulePoll = (): void => {
    if (disposed || pollTimer !== null || pollInFlight) return;
    pollTimer = window.setTimeout(() => {
      pollTimer = null;
      if (disposed || !document.contains(root)) return;
      if (sending || discoveryDispatching) {
        schedulePoll();
        return;
      }
      pollInFlight = true;
      void poll(selectedLaneId ?? undefined)
        .then((result) => {
          if (!disposed) controller.applyResult(result);
        })
        .catch(() => undefined)
        .finally(() => {
          pollInFlight = false;
          if (disposed) return;
          releaseCommandSlotWaiters();
          queueMicrotask(maybeResumeLaneStart);
          queueMicrotask(advanceAgentDiscovery);
          schedulePoll();
        });
    }, 250);
  };

  const sendAndWait = async (
    intent: D1Intent,
    onRejected?: () => void,
  ): Promise<D1IntentResult | null> => {
    if (sending || pendingCommandId) return null;
    sending = true;
    try {
      let result = await send(intent);
      controller.applyResult(result);
      for (
        let attempt = 0;
        result.pendingCommandId &&
        result.projection.permissionDock.request === null &&
        attempt < 24;
        attempt += 1
      ) {
        result = await poll(selectedLaneId ?? undefined, true);
        controller.applyResult(result);
      }
      return result;
    } catch (error: unknown) {
      root.dataset.d1Error = String(error);
      errorMessage = String(error);
      submittedDraft = null;
      onRejected?.();
      render(true);
      return null;
    } finally {
      sending = false;
      schedulePoll();
    }
  };

  const sendIntent = (intent: D1Intent, onRejected?: () => void): void => {
    void sendAndWait(intent, onRejected);
  };

  const submitComposer = (content: string): void => {
    const mutationBlock = composerMutationBlockReason(projection, selectedLaneId);
    if (!shouldRouteComposerMutation(content, mutationBlock)) {
      if (mutationBlock) {
        errorMessage = translate(locale, mutationBlock, {});
        render(true);
      }
      return;
    }
    if (!selectedLaneId) return;
    submittedDraft = content;
    errorMessage = null;
    const route = conversationForLane(projection, selectedLaneId);
    if (route?.kind === "acp" && !projection.composer.busy) {
      sendIntent(
        {
          type: "send_agent_session_input",
          laneId: route.laneId,
          sessionId: route.sessionId,
          content,
        },
        () => {
          draft = content;
        },
      );
      return;
    }
    sendIntent({ type: "submit", laneId: selectedLaneId, content }, () => {
      draft = content;
    });
  };

  const startLane = async (task: string, agentId: string | null): Promise<boolean> => {
    // Native Lane creation remains available while ACP discovery runs, but
    // both use Core's one-command-at-a-time D1 adapter boundary.
    if (!(await waitForCommandSlot())) return false;
    const previewResult = await sendAndWait({ type: "preview_default_lane", preset: "coder" });
    const preview = previewResult?.projection.starterLanePreviews.at(-1);
    if (
      !preview ||
      previewResult?.outcome.state === "rejected" ||
      preview.diagnostics.length > 0
    ) {
      errorMessage =
        previewResult?.outcome.reason ??
        preview?.diagnostics[0] ??
        projection.workspaceEligibility?.diagnostic ??
        "Core did not publish a creatable Lane preview.";
      render(false);
      return false;
    }
    pendingLaneStart = { laneId: preview.laneId, task, agentId };
    const createResult = await sendAndWait({
      type: "create_starter_lane",
      laneId: preview.laneId,
      preset: "coder",
      branch: preview.branch,
      previewId: preview.previewId,
      contentSha256: preview.contentSha256,
    });
    if (!createResult || createResult.outcome.state === "rejected") {
      pendingLaneStart = null;
      return false;
    }
    await maybeResumeLaneStart();
    return true;
  };

  async function maybeResumeLaneStart(): Promise<void> {
    const pending = pendingLaneStart;
    if (!pending || !projection.lanes.some((lane) => lane.id === pending.laneId)) return;
    if (!commandSlotAvailable()) return;
    // Approval resolves asynchronously; resume only after Core projects the
    // exact confirmed Lane and releases the command slot, while retaining its
    // sole Agent task locally.
    pendingLaneStart = null;
    selectedLaneId = pending.laneId;
    if (pending.agentId === null) {
      focusedConversation = { kind: "native", laneId: pending.laneId };
      submittedDraft = pending.task;
      await sendAndWait({ type: "submit", laneId: pending.laneId, content: pending.task });
      return;
    }
    const result = await sendAndWait({
      type: "start_agent_session",
      laneId: pending.laneId,
      agentId: pending.agentId,
      model: null,
      task: pending.task,
    });
    const session = result?.projection.agentSessions
      .filter(
        (candidate) =>
          candidate.laneId === pending.laneId && candidate.agentId === pending.agentId,
      )
      .at(-1);
    if (session) {
      focusedConversation = {
        kind: "acp",
        laneId: session.laneId,
        sessionId: session.sessionId,
      };
      render(true);
    }
  };

  function discoveryIsProbing(): boolean {
    return agentDiscoveryStarted && !agentDiscoveryComplete;
  }

  function mountAgentMenu(): void {
    if (!agentMenuOpen || disposed) return;
    const anchor = root.querySelector<HTMLButtonElement>("[data-create-lane]");
    if (!anchor) return;
    menuController = renderAgentMenu(
      anchor,
      {
        locale,
        canCreateLane: projection.workspaceEligibility?.canCreateLane === true,
        probing: discoveryIsProbing(),
        eligibilityDiagnostic: projection.workspaceEligibility?.diagnostic ?? null,
        selected: newLaneSelection,
        taskDraft: pendingAgentTaskDraft,
        submitting: creatingLane,
        adapters: projection.agentAdapters,
      },
      (selection) => {
        newLaneSelection = selection;
      },
      () => {
        agentMenuOpen = false;
        menuController = null;
        creatingLane = false;
        if (!remountingAgentMenu) {
          pendingAgentTaskDraft = "";
          newLaneSelection = undefined;
        }
      },
      async (selection, task) => {
        pendingAgentTaskDraft = task;
        creatingLane = true;
        const accepted = await startLane(
          task,
          selection.kind === "native" ? null : selection.agentId,
        );
        creatingLane = false;
        if (!accepted) {
          render(false);
          return false;
        }
        // A Core approval can redraw and remount this async popover before
        // creation completes. Close the current controller explicitly once the
        // accepted request has yielded to that operator decision.
        agentMenuOpen = false;
        menuController?.close();
        menuController = null;
        pendingAgentTaskDraft = "";
        newLaneSelection = undefined;
        return true;
      },
      (task) => {
        pendingAgentTaskDraft = task;
      },
    );
  }

  function observeAgentDiscoveryResult(result: D1IntentResult): boolean {
    if (!discoveryInFlight || result.pendingCommandId !== null) return false;
    const completed = discoveryInFlight;
    discoveryInFlight = null;
    if (completed.kind === "query") {
      agentQueryComplete = result.outcome.state !== "rejected";
      if (!agentQueryComplete) agentDiscoveryComplete = true;
    }
    queueMicrotask(advanceAgentDiscovery);
    return true;
  }

  async function dispatchAgentDiscovery(
    command: { kind: "query" } | { kind: "probe"; agentId: string },
  ): Promise<void> {
    if (
      disposed ||
      discoveryDispatching ||
      discoveryInFlight ||
      pollInFlight ||
      sending ||
      pendingCommandId
    ) {
      return;
    }
    discoveryDispatching = true;
    discoveryInFlight = command;
    if (command.kind === "probe") attemptedAgentProbes.add(command.agentId);
    sending = true;
    try {
      const result = await send(
        command.kind === "query"
          ? { type: "query_agent_adapters" }
          : { type: "probe_agent_adapter", agentId: command.agentId },
      );
      if (disposed) return;
      controller.applyResult(result);
    } catch (error: unknown) {
      if (disposed) return;
      const failed = discoveryInFlight;
      discoveryInFlight = null;
      root.dataset.d1Error = String(error);
      errorMessage = String(error);
      if (failed?.kind === "query") agentDiscoveryComplete = true;
      render(false);
    } finally {
      sending = false;
      discoveryDispatching = false;
      if (!disposed) {
        releaseCommandSlotWaiters();
        queueMicrotask(advanceAgentDiscovery);
        schedulePoll();
      }
    }
  }

  function advanceAgentDiscovery(): void {
    if (
      disposed ||
      !agentDiscoveryStarted ||
      agentDiscoveryComplete ||
      discoveryDispatching ||
      discoveryInFlight ||
      pollInFlight ||
      sending ||
      creatingLane ||
      pendingCommandId
    ) {
      return;
    }
    if (!agentQueryComplete) {
      void dispatchAgentDiscovery({ kind: "query" });
      return;
    }
    const next = orderedAgentAdapters(projection.agentAdapters).find(
      (adapter) =>
        adapter.startability === "probe_required" &&
        !attemptedAgentProbes.has(adapter.agentId),
    );
    if (next) {
      void dispatchAgentDiscovery({ kind: "probe", agentId: next.agentId });
      return;
    }
    agentDiscoveryComplete = true;
    render(false);
  }

  function openAgentMenu(): void {
    if (agentMenuOpen) {
      menuController?.close();
      return;
    }
    agentMenuOpen = true;
    if (!agentDiscoveryStarted || agentDiscoveryComplete) {
      agentDiscoveryStarted = true;
      agentDiscoveryComplete = false;
      agentQueryComplete = false;
      discoveryInFlight = null;
      attemptedAgentProbes.clear();
      mountAgentMenu();
      advanceAgentDiscovery();
      return;
    }
    mountAgentMenu();
  }

  const render = (focusComposer = false): void => {
    const reopenAgentMenu = agentMenuOpen;
    if (menuController) {
      remountingAgentMenu = true;
      agentMenuOpen = false;
      menuController.close();
      remountingAgentMenu = false;
      menuController = null;
      agentMenuOpen = reopenAgentMenu;
    }
    const previousComposer = root.querySelector<HTMLTextAreaElement>("[data-composer]");
    const restoreComposerFocus = previousComposer === document.activeElement;
    const selectionStart = previousComposer?.selectionStart ?? draft.length;
    const selectionEnd = previousComposer?.selectionEnd ?? selectionStart;
    const frame = document.createElement("section");
    frame.className = "frame d1-frame";
    frame.dataset.screen = "d1-cockpit";
    const showWelcome = shouldShowWelcome();
    if (showWelcome) frame.dataset.d1State = "welcome";

    frame.dataset.nativeWindowShell = "true";
    const topbar = renderCockpitTopbar(projection, locale, showWelcome);
    const titlebar = topbar.element;

    const body = document.createElement("div");
    body.className = "d1-body";
    body.dataset.cockpitGrid = "true";
    body.dataset.cockpitLayout = window.innerWidth <= 1100 ? "narrow" : "desktop";
    if (showWelcome) body.classList.add("d1-body-welcome");

    const activity = renderActivityRail(locale, {
      lanesAvailable: !showWelcome,
      lanesOpen: laneRailOpen,
      onToggleLanes: () => {
        laneRailOpen = !laneRailOpen;
        laneRailFocusTarget = laneRailOpen ? "rail" : "toggle";
        render(false);
      },
    });
    const lanes = renderLaneRail({
      projection,
      locale,
      open: laneRailOpen,
      selectedLaneId,
      onCreateLane: () => {
        if (options.onCreateLane) options.onCreateLane();
        else void openAgentMenu();
      },
      onDismiss: () => {
        laneRailOpen = false;
        laneRailFocusTarget = "toggle";
        render(false);
      },
      onSelectLane: (laneId) => {
        selectedLaneId = laneId;
        transcriptLaneId = null;
        transcript.reset([]);
        focusedConversation = conversationForLane(projection, laneId);
        render(false);
      },
      onRetryAgent: (sessionId, laneId) => {
        selectedLaneId = laneId;
        focusedConversation = conversationForLane(projection, laneId);
        sendIntent({ type: "retry_agent_session", laneId, sessionId });
      },
    });
    const workSurface = document.createElement("section");
    workSurface.className = "d1-work-surface";
    const showRecovery = !["live", "gate_queue_clear", "empty"].includes(projection.recovery.state);
    if (showWelcome) {
      renderWelcomeCenter(workSurface, locale, options.onOpenProject);
    } else if (showRecovery) {
      renderD6Recovery(
        workSurface,
        projection.recovery,
        recoverD6 ?? (async () => projection.recovery),
        locale,
        options.onOpenProject,
      );
    } else {
      const transcriptRegion = document.createElement("section");
      transcriptRegion.className = "d1-transcript";
      transcriptRegion.dataset.centerSequence = "true";
      transcriptRegion.setAttribute("aria-label", translate(locale, "d1.transcript", {}));
      transcriptRegion.setAttribute("role", "log");
      transcriptRegion.setAttribute("aria-live", "polite");
      transcriptRegion.setAttribute("aria-relevant", "additions text");
      transcriptRegion.setAttribute("aria-busy", String(projection.composer.busy));
      transcriptRegion.tabIndex = 0;
      const projectionMatchesSelectedLane = projection.selectedLaneId === selectedLaneId;
      const visibleRows = projectionMatchesSelectedLane
        ? transcript.visible(transcriptRegion.clientHeight || 720, 36)
        : [];
      appendUnavailableTranscriptRows(transcriptRegion, projection.unavailableFeatures, locale);
      appendTranscriptRows(transcriptRegion, visibleRows);
      if (projectionMatchesSelectedLane) {
        appendTypedWorkCards(transcriptRegion, projection.contextDock.checklist, locale);
      }
      if (!projectionMatchesSelectedLane) {
        const switching = document.createElement("p");
        switching.dataset.typedEmpty = "transcript-switching";
        switching.textContent = translate(locale, "d1.transcript.switching", {});
        transcriptRegion.append(switching);
      }
      const liveWork = projectionMatchesSelectedLane ? renderLiveWorkBar(projection, locale) : null;
      if (liveWork) transcriptRegion.append(liveWork);
      const streamState = document.createElement("div");
      streamState.className = "d1-stream-state";
      streamState.setAttribute("role", "status");
      streamState.textContent = projection.composer.busy
        ? translate(locale, "d1.stream.active", {
            lane: selectedLaneId ?? "—",
          })
        : translate(locale, "d1.stream.idle", {});
      transcriptRegion.append(streamState);
      transcriptRegion.addEventListener("scroll", () => {
        const atBottom = transcriptAtBottom(transcriptRegion);
        const first = transcriptRegion.querySelector<HTMLElement>("[data-row-id]")?.dataset.rowId;
        transcript.setFollowLatest(atBottom, first);
      });
      if (transcript.newOutputCount > 0) {
        const latest = button(
          translate(locale, "d1.newOutput", { count: String(transcript.newOutputCount) }),
          "newOutput",
        );
        latest.addEventListener("click", () => {
          transcript.setFollowLatest(true);
          render(false);
        });
        transcriptRegion.append(latest);
      }
      workSurface.append(transcriptRegion);
    }

    const permissionHost = document.createElement("div");
    permissionHost.className = "d1-permission-host";
    const projectionMatchesSelectedLane = projection.selectedLaneId === selectedLaneId;
    if (projectionMatchesSelectedLane && projection.permissionDock.request) {
      renderPermissionDock(
        permissionHost,
        projection.permissionDock,
        sendPermission ?? (async () => undefined),
        locale,
      );
    }

    const composerRegion = document.createElement("section");
    composerRegion.className = "d1-composer";
    const composerLabel = document.createElement("label");
    composerLabel.textContent = projection.composer.busy
      ? translate(locale, "d1.composer.queue", {})
      : translate(locale, "d1.composer.prompt", {});
    const composer = document.createElement("textarea");
    composer.dataset.composer = "true";
    composer.rows = 3;
    composer.value = draft;
    composer.disabled = !projection.composer.editable;
    composer.placeholder = translate(locale, "d1.composer.placeholder", {});
    composer.setAttribute("aria-label", translate(locale, "d1.composer.inputLabel", {}));
    composer.addEventListener("compositionstart", () => {
      composing = true;
    });
    composer.addEventListener("compositionend", () => {
      composing = false;
      draft = composer.value;
    });
    composer.addEventListener("input", () => {
      draft = composer.value;
    });
    composer.addEventListener("keydown", (event) => {
      if (!shouldSubmitComposer(event, composing)) return;
      event.preventDefault();
      submitComposer(composer.value);
    });
    composerLabel.append(composer);
    composerRegion.append(composerLabel);
    const submit = button(translate(locale, "d1.composer.send", {}), "composerSend");
    submit.setAttribute("aria-label", translate(locale, "d1.composer.sendLabel", {}));
    submit.disabled = composer.disabled;
    submit.addEventListener("click", () => submitComposer(composer.value));
    composerRegion.append(submit);
    const focusedAcpSessionId =
      focusedConversation?.kind === "acp" ? focusedConversation.sessionId : null;
    const focusedAcp = focusedAcpSessionId
      ? projection.agentSessions.find(
          (session) => session.sessionId === focusedAcpSessionId,
        )
      : null;
    const canCancelAcp = focusedAcp
      ? ["starting", "running", "waiting_approval"].includes(focusedAcp.status)
      : false;
    const mutationBlock = composerMutationBlockReason(projection, selectedLaneId);
    if (!mutationBlock && ((projection.composer.canCancel && selectedLaneId) || canCancelAcp)) {
      const cancel = button(translate(locale, "d1.cancel", {}), "cancelTurn");
      cancel.addEventListener("click", () => {
        const commandBlock = composerMutationBlockReason(projection, selectedLaneId);
        const route = conversationForLane(projection, selectedLaneId);
        if (commandBlock) {
          errorMessage = translate(locale, commandBlock, {});
          render(false);
        } else if (route?.kind === "acp") {
          sendIntent({
            type: "cancel_agent_session",
            laneId: route.laneId,
            sessionId: route.sessionId,
          });
        } else if (selectedLaneId) {
          sendIntent({ type: "cancel", laneId: selectedLaneId });
        }
      });
      composerRegion.append(cancel);
    }
    if (errorMessage) {
      const rejection = document.createElement("p");
      rejection.dataset.d1Rejection = "true";
      rejection.setAttribute("role", "alert");
      rejection.textContent = errorMessage;
      composerRegion.append(rejection);
    }
    if (mutationBlock) {
      const blocked = document.createElement("p");
      blocked.dataset.mutationBlocked = "true";
      blocked.setAttribute("role", "status");
      blocked.textContent = translate(locale, mutationBlock, {});
      composerRegion.append(blocked);
    }
    const main = renderLaneWorkSurface({
      work: workSurface,
      permission: permissionHost,
      composer: composerRegion,
      showWelcome,
      ariaLabel: translate(locale, "d1.workSurface", {}),
    });
    const right = renderContextDock(projection, locale, projectionMatchesSelectedLane);
    right.dataset.drawerOpen = String(contextDrawerOpen);
    topbar.contextDrawerToggle.setAttribute("aria-expanded", String(contextDrawerOpen));
    topbar.contextDrawerToggle.addEventListener("click", () => {
      contextDrawerOpen = !contextDrawerOpen;
      right.dataset.drawerOpen = String(contextDrawerOpen);
      topbar.contextDrawerToggle.setAttribute("aria-expanded", String(contextDrawerOpen));
      topbar.contextDrawerToggle.classList.toggle("on", contextDrawerOpen);
      if (contextDrawerOpen) right.focus();
    });
    right.tabIndex = -1;

    if (showWelcome) {
      body.append(activity, main);
    } else {
      body.append(activity, lanes, main, right);
    }
    const status = document.createElement("footer");
    status.className = "statusbar d1-status";
    status.dataset.shellLandmark = "statusbar";
    status.dataset.statusbar = "true";
    status.textContent = `${projection.environment.workMode} · ${projection.environment.permissionLevel} · ${projection.preferences.skin}/${projection.preferences.mode}`;
    frame.append(titlebar, body, status);
    root.replaceChildren(frame);

    if (agentMenuOpen) mountAgentMenu();

    if (laneRailFocusTarget) {
      const focusTarget =
        laneRailFocusTarget === "rail"
          ? root.querySelector<HTMLElement>("#d1-lane-rail [data-create-lane]")
          : root.querySelector<HTMLElement>("[data-lanes-toggle]");
      laneRailFocusTarget = null;
      focusTarget?.focus();
    } else if (focusComposer || restoreComposerFocus) {
      const nextComposer = root.querySelector<HTMLTextAreaElement>("[data-composer]");
      nextComposer?.focus();
      nextComposer?.setSelectionRange(selectionStart, selectionEnd);
    }
  };

  render(true);
  if (options.poll !== false) schedulePoll();
  return controller;
}
