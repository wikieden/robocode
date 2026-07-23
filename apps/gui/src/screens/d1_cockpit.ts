import { translate } from "../i18n/catalog";
import { D1_ACTIVITY_ITEMS } from "../components/activity_rail";
import {
  orderedAgentAdapters,
  renderAgentMenu,
  type AgentMenuController,
  type AgentMenuSelection,
} from "../components/agent_menu";
import { shouldSubmitComposer } from "../components/composer";
import { environmentValues } from "../components/environment";
import { adjacentLaneId } from "../components/lane_rail";
import { approvalRowText, taskRowText } from "../components/live_work";
import {
  renderPermissionDock,
  type PermissionIntent,
} from "../components/permission_dock";
import { toolRowText } from "../components/tool_row";
import { transcriptAtBottom } from "../components/transcript";
import { renderWelcomeCenter } from "../components/welcome_center";
import { renderLaneTaskPrompt } from "../components/lane_task_prompt";
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
type PollD1 = (selectedLaneId?: string) => Promise<D1IntentResult>;
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
  if (!laneId) return null;
  const session = projection.agentSessions.find((candidate) => candidate.laneId === laneId);
  return session
    ? { kind: "acp", laneId, sessionId: session.sessionId }
    : { kind: "native", laneId };
}

function button(label: string, marker?: string): HTMLButtonElement {
  const element = document.createElement("button");
  element.type = "button";
  element.className = "d1-action";
  element.textContent = label;
  if (marker) element.dataset[marker] = "true";
  return element;
}

function definition(list: HTMLDListElement, label: string, value: string): void {
  const term = document.createElement("dt");
  term.textContent = label;
  const detail = document.createElement("dd");
  detail.textContent = value;
  list.append(term, detail);
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
  let menuController: AgentMenuController | null = null;
  let agentMenuOpen = false;
  let pendingAgentSelection: AgentMenuSelection | null = null;
  let pendingAgentTaskDraft = "";
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

  const controller: D1Controller = {
    transcript,
    applyProjection: (next) => {
      if (disposed) return;
      const nextKey = JSON.stringify(next);
      if (nextKey === projectionKey) return;
      projection = next;
      if (!selectedLaneId || !next.lanes.some((lane) => lane.id === selectedLaneId)) {
        selectedLaneId = next.selectedLaneId ?? next.lanes[0]?.id ?? null;
      }
      focusedConversation = conversationForLane(next, selectedLaneId);
      projectionKey = nextKey;
      locale = next.preferences.locale;
      transcript.replace(next.transcript);
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
      }
      const nextKey = JSON.stringify(result.projection);
      const projectionChanged = nextKey !== projectionKey;
      if (projectionChanged) {
        projection = result.projection;
        if (
          !selectedLaneId ||
          !result.projection.lanes.some((lane) => lane.id === selectedLaneId)
        ) {
          selectedLaneId =
            result.projection.selectedLaneId ?? result.projection.lanes[0]?.id ?? null;
        }
        focusedConversation = conversationForLane(result.projection, selectedLaneId);
        projectionKey = nextKey;
        locale = result.projection.preferences.locale;
        transcript.replace(result.projection.transcript);
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
      window.removeEventListener("keydown", handleWindowKeydown);
    },
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
      for (let attempt = 0; result.pendingCommandId && attempt < 24; attempt += 1) {
        result = await poll(selectedLaneId ?? undefined);
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

  const startLane = async (task: string, agentId: string | null): Promise<void> => {
    const previewResult = await sendAndWait({ type: "preview_default_lane", preset: "coder" });
    const preview = previewResult?.projection.starterLanePreviews.at(-1);
    if (!preview || preview.diagnostics.length > 0) {
      errorMessage =
        preview?.diagnostics[0] ??
        projection.workspaceEligibility?.diagnostic ??
        "Core did not publish a creatable Lane preview.";
      render(false);
      return;
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
      return;
    }
    await maybeResumeLaneStart();
  };

  async function maybeResumeLaneStart(): Promise<void> {
    const pending = pendingLaneStart;
    if (!pending || !projection.lanes.some((lane) => lane.id === pending.laneId)) return;
    // Approval resolves asynchronously; resume only after Core projects the
    // exact created Lane, while retaining its sole Agent task locally.
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

  function mountLaneTaskPrompt(): void {
    const selection = pendingAgentSelection;
    if (!selection || disposed || root.querySelector("[data-lane-task]")) return;
    const title =
      selection.kind === "native"
        ? translate(locale, "d1.task.nativeTitle", {})
        : translate(locale, "d1.task.acpTitle", {
            agent:
              projection.agentAdapters.find(
                (adapter) => adapter.agentId === selection.agentId,
              )?.displayName ?? selection.agentId,
          });
    renderLaneTaskPrompt(
      root,
      locale,
      title,
      (task) => {
        pendingAgentSelection = null;
        pendingAgentTaskDraft = "";
        void startLane(task, selection.kind === "native" ? null : selection.agentId);
      },
      () => {
        pendingAgentSelection = null;
        pendingAgentTaskDraft = "";
      },
      pendingAgentTaskDraft,
      (task) => {
        pendingAgentTaskDraft = task;
      },
    );
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
        adapters: projection.agentAdapters,
      },
      (selection) => {
        pendingAgentSelection = selection;
        pendingAgentTaskDraft = "";
        mountLaneTaskPrompt();
      },
      () => {
        agentMenuOpen = false;
        menuController = null;
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
      agentMenuOpen = false;
      menuController.close();
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

    const titlebar = document.createElement("header");
    titlebar.className = "vbar d1-titlebar";
    titlebar.dataset.tauriDragRegion = "true";
    const title = document.createElement("h1");
    title.textContent = translate(locale, showWelcome ? "d1.welcome.windowTitle" : "d1.title", {});
    title.dataset.tauriDragRegion = "true";
    const context = document.createElement("p");
    context.textContent = showWelcome
      ? translate(locale, "d1.welcome.noProject", {})
      : `${projection.environment.cwd} · ${projection.environment.model}`;
    context.dataset.tauriDragRegion = "true";
    titlebar.append(title, context);

    const body = document.createElement("div");
    body.className = "d1-body";
    if (showWelcome) body.classList.add("d1-body-welcome");

    const activity = document.createElement("nav");
    activity.className = "d1-activity";
    activity.setAttribute("aria-label", translate(locale, "d1.activity", {}));
    for (const activityItem of D1_ACTIVITY_ITEMS) {
      const item = button("");
      const icon = document.createElement("span");
      icon.className = "d1-activity-icon";
      icon.ariaHidden = "true";
      icon.textContent = activityItem.icon;
      item.append(icon);
      item.title = translate(locale, activityItem.key, {});
      item.setAttribute("aria-label", translate(locale, activityItem.key, {}));
      if (activityItem.key === "d1.activity.work") {
        item.setAttribute("aria-current", "page");
      } else {
        item.disabled = true;
      }
      activity.append(item);
    }

    const lanes = document.createElement("nav");
    lanes.className = "d1-lanes";
    lanes.setAttribute("aria-label", translate(locale, "d1.lanes", {}));
    const laneTitle = document.createElement("h2");
    laneTitle.textContent = translate(locale, "d1.lanes", {});
    lanes.append(laneTitle);
    const createLane = button(translate(locale, "d1.lane.create", {}), "createLane");
    createLane.classList.add("d1-create-lane");
    createLane.setAttribute("aria-haspopup", "menu");
    createLane.setAttribute("aria-expanded", "false");
    createLane.addEventListener("click", () => {
      if (options.onCreateLane) options.onCreateLane();
      else void openAgentMenu();
    });
    lanes.append(createLane);
    projection.lanes.forEach((lane, index) => {
      const boundSession = projection.agentSessions.find(
        (candidate) => candidate.laneId === lane.id,
      );
      const item = button("");
      item.className = "d1-lane";
      item.dataset.laneId = lane.id;
      item.dataset.laneAgentId = boundSession?.agentId ?? "viden";
      item.setAttribute("aria-current", String(lane.id === selectedLaneId));
      item.innerHTML = `<span class="d1-lane-status" data-status="${lane.status}"></span><span><strong></strong><small></small></span>`;
      item.querySelector("strong")!.textContent = lane.id;
      item.querySelector("small")!.textContent = boundSession
        ? `${boundSession.agentId} · ${boundSession.status}`
        : `Viden · ${lane.status}`;
      item.addEventListener("keydown", (event) => {
        if (!['ArrowUp', 'ArrowDown'].includes(event.key)) return;
        event.preventDefault();
        const target = adjacentLaneId(
          projection.lanes.map((candidate) => candidate.id),
          index,
          event.key === "ArrowDown" ? "next" : "previous",
        );
        if (target) {
          Array.from(root.querySelectorAll<HTMLElement>("[data-lane-id]"))
            .find((candidate) => candidate.dataset.laneId === target)
            ?.focus();
        }
      });
      item.addEventListener("click", () => {
        selectedLaneId = lane.id;
        focusedConversation = conversationForLane(projection, lane.id);
        render(false);
      });
      lanes.append(item);
      if (
        boundSession &&
        ["failed", "cancelled"].includes(boundSession.status)
      ) {
        const retry = button(translate(locale, "d1.session.retry", {}));
        retry.className = "d1-lane-agent-retry";
        retry.dataset.retryLaneAgent = lane.id;
        retry.addEventListener("click", () => {
          selectedLaneId = lane.id;
          focusedConversation = conversationForLane(projection, lane.id);
          sendIntent({
            type: "retry_agent_session",
            sessionId: boundSession.sessionId,
          });
        });
        lanes.append(retry);
      }
    });

    const main = document.createElement("main");
    main.className = "d1-main";
    if (showWelcome) main.classList.add("d1-main-welcome");
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
      transcriptRegion.setAttribute("aria-label", translate(locale, "d1.transcript", {}));
      transcriptRegion.setAttribute("role", "log");
      transcriptRegion.setAttribute("aria-live", "polite");
      transcriptRegion.setAttribute("aria-relevant", "additions text");
      transcriptRegion.setAttribute("aria-busy", String(projection.composer.busy));
      transcriptRegion.tabIndex = 0;
      const visibleRows = transcript.visible(transcriptRegion.clientHeight || 720, 36);
      for (const row of visibleRows) {
        const article = document.createElement("article");
        article.className = "d1-row";
        article.dataset.rowId = row.id;
        article.dataset.kind = row.kind;
        const kind = document.createElement("span");
        kind.className = "d1-row-kind";
        kind.textContent = row.kind.replaceAll("_", " ");
        const content = document.createElement("pre");
        content.textContent = row.content;
        article.append(kind, content);
        transcriptRegion.append(article);
      }
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
    if (projection.permissionDock.request) {
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
      const content = composer.value;
      if (!selectedLaneId || !content.trim()) return;
      submittedDraft = content;
      errorMessage = null;
      if (focusedConversation?.kind === "acp") {
        sendIntent(
          {
            type: "send_agent_session_input",
            sessionId: focusedConversation.sessionId,
            content,
          },
          () => {
            draft = content;
          },
        );
      } else {
        sendIntent({ type: "submit", laneId: selectedLaneId, content }, () => {
          draft = content;
        });
      }
    });
    composerLabel.append(composer);
    composerRegion.append(composerLabel);
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
    if ((projection.composer.canCancel && selectedLaneId) || canCancelAcp) {
      const cancel = button(translate(locale, "d1.cancel", {}), "cancelTurn");
      cancel.addEventListener("click", () => {
        if (focusedConversation?.kind === "acp") {
          sendIntent({
            type: "cancel_agent_session",
            sessionId: focusedConversation.sessionId,
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
    main.append(workSurface);
    if (!showWelcome) {
      if (projection.permissionDock.request) main.append(permissionHost);
      main.append(composerRegion);
    }

    const right = document.createElement("aside");
    right.className = "d1-right";
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
    for (const task of projection.liveWork.tasks) {
      const item = document.createElement("div");
      item.className = "d1-work-item";
      item.textContent = taskRowText(task);
      liveWork.append(item);
    }
    for (const tool of projection.liveWork.tools) {
      const item = document.createElement("div");
      item.className = "d1-work-item";
      item.textContent = toolRowText(tool);
      liveWork.append(item);
    }
    for (const approval of projection.liveWork.approvals) {
      const item = document.createElement("div");
      item.className = "d1-work-item warning";
      item.textContent = approvalRowText(approval);
      liveWork.append(item);
    }
    for (const input of projection.liveWork.queuedInputs) {
      const item = document.createElement("div");
      item.className = "d1-work-item";
      item.textContent = `${translate(locale, "d1.queued", {})} · ${input.contentPreview}`;
      liveWork.append(item);
    }
    for (const evidence of projection.liveWork.evidence) {
      const item = document.createElement("div");
      item.className = "d1-work-item";
      item.textContent = `${evidence.kind} · ${evidence.summary}`;
      liveWork.append(item);
    }
    for (const unavailable of projection.unavailableFeatures) {
      const item = document.createElement("div");
      item.className = "d1-unavailable";
      item.dataset.unavailableFeature = unavailable.id;
      item.setAttribute("aria-disabled", "true");
      item.textContent = `${translate(locale, "d1.unavailable", {})} · ${unavailable.id} · ${unavailable.code}`;
      item.title = unavailable.message;
      liveWork.append(item);
    }
    right.append(environment, liveWork);

    if (showWelcome) {
      body.append(activity, main);
    } else {
      body.append(activity, lanes, main, right);
    }
    const status = document.createElement("footer");
    status.className = "d1-status";
    status.textContent = `${projection.environment.workMode} · ${projection.environment.permissionLevel} · ${projection.preferences.skin}/${projection.preferences.mode}`;
    frame.append(titlebar, body, status);
    root.replaceChildren(frame);

    if (agentMenuOpen) queueMicrotask(mountAgentMenu);

    if (focusComposer || restoreComposerFocus) {
      const nextComposer = root.querySelector<HTMLTextAreaElement>("[data-composer]");
      nextComposer?.focus();
      nextComposer?.setSelectionRange(selectionStart, selectionEnd);
    }
    mountLaneTaskPrompt();
  };

  render(true);
  if (options.poll !== false) schedulePoll();
  return controller;
}
