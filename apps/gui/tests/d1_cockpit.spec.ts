// @vitest-environment jsdom

import { beforeEach, describe, expect, test, vi } from "vitest";

import {
  renderD1Cockpit,
  type D1Intent,
  type D1IntentResult,
  type D1RenderOptions,
} from "../src/screens/d1_cockpit";
import { D1_PROJECTION } from "./support/d1_projection";

function setup(projection = D1_PROJECTION, options: D1RenderOptions = {}) {
  document.body.innerHTML = '<main id="app"></main>';
  const root = document.querySelector<HTMLElement>("#app");
  if (!root) throw new Error("missing root");
  const result: D1IntentResult = {
    projection,
    pendingCommandId: null,
    outcome: { state: "confirmed", reason: null },
  };
  const send = vi.fn(async (_intent: D1Intent) => result);
  const poll = vi.fn(async () => result);
  const controller = renderD1Cockpit(root, projection, send, poll, undefined, undefined, options);
  return { root, send, poll, controller };
}

const EMPTY_PROJECTION = {
  ...D1_PROJECTION,
  selectedLaneId: null,
  lanes: [],
  liveWork: { tasks: [], tools: [], approvals: [], queuedInputs: [], evidence: [] },
  transcript: [],
  composer: {
    editable: false,
    busy: false,
    canCancel: false,
    canSubmitImmediately: false,
  },
  recovery: {
    connection: "live" as const,
    state: "empty" as const,
    detail: null,
    hint: null,
    recoverable: false,
    businessSuccessBlocked: false,
    usedTokens: null,
    hardTokenLimit: null,
    missingCapabilities: ["runtime.recent_work"],
    actions: [],
  },
};

describe("D1 canonical streaming cockpit", () => {
  beforeEach(() => {
    document.documentElement.lang = "en";
  });

  test("renders activity, Lane, transcript, composer, Environment, and Live Work regions", () => {
    const { root } = setup();

    expect(root.querySelector('[data-screen="d1-cockpit"]')).not.toBeNull();
    expect(root.querySelector('nav[aria-label="Activity"]')).not.toBeNull();
    expect(root.querySelector('nav[aria-label="Lanes"]')).not.toBeNull();
    expect(root.querySelector('[aria-label="Transcript"]')?.textContent).toContain("Working");
    const transcript = root.querySelector('[aria-label="Transcript"]');
    expect(transcript?.getAttribute("role")).toBe("log");
    expect(transcript?.getAttribute("aria-live")).toBe("polite");
    expect(transcript?.getAttribute("aria-relevant")).toBe("additions text");
    expect(transcript?.getAttribute("aria-busy")).toBe("true");
    expect(root.querySelector('[aria-label="Environment"]')?.textContent).toContain("deepseek");
    expect(root.querySelector('[aria-label="Live Work"]')?.textContent).toContain("cargo");
    expect(root.querySelectorAll('[data-unavailable-feature]')).toHaveLength(4);
    expect(document.activeElement).toBe(root.querySelector("[data-composer]"));
  });

  test("turns the empty D1 workspace into a branded welcome center with only real entry points", () => {
    const onOpenProject = vi.fn();
    const { root, controller } = setup(EMPTY_PROJECTION, { onOpenProject, poll: false });

    const welcome = root.querySelector<HTMLElement>('[data-d1-welcome]');
    expect(welcome?.getAttribute("aria-label")).toBe("Welcome to Viden");
    expect(welcome?.querySelector<HTMLImageElement>("img")?.src).toContain("image/svg+xml");
    expect(welcome?.querySelector<HTMLImageElement>("img")?.alt).toBe("");
    expect(welcome?.textContent).toContain("Your local-first agent workspace");
    expect(welcome?.textContent).toContain("Get started");
    expect(welcome?.textContent).toContain("Recent projects");
    expect(welcome?.textContent).toContain("GUI-CORE-007");
    expect(root.querySelector('[aria-label="Transcript"]')).toBeNull();
    expect(root.querySelector("[data-composer]")).toBeNull();
    expect(root.querySelector(".d1-lanes")).toBeNull();
    expect(root.querySelector(".d1-right")).toBeNull();

    root.querySelector<HTMLButtonElement>("[data-open-project]")?.click();
    expect(onOpenProject).toHaveBeenCalledTimes(1);
    controller.dispose();
  });

  test("treats an opened project with no Lanes as a project cockpit, not as welcome", () => {
    const onCreateLane = vi.fn();
    const { root, controller } = setup(
      EMPTY_PROJECTION,
      { onCreateLane, poll: false } as unknown as D1RenderOptions,
    );

    expect(root.querySelector("[data-d1-welcome]")).toBeNull();
    const createLane = root.querySelector<HTMLButtonElement>("[data-create-lane]");
    expect(createLane).not.toBeNull();
    createLane?.click();
    expect(onCreateLane).toHaveBeenCalledTimes(1);
    controller.dispose();
  });

  test("opens project from the visible shortcut and removes it when the cockpit is disposed", () => {
    const onOpenProject = vi.fn();
    const { controller } = setup(EMPTY_PROJECTION, { onOpenProject, poll: false });

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "o", metaKey: true }));
    expect(onOpenProject).toHaveBeenCalledTimes(1);

    controller.dispose();
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "o", metaKey: true }));
    expect(onOpenProject).toHaveBeenCalledTimes(1);
  });

  test("localizes the welcome center from Core-owned presentation preferences", () => {
    const { root, controller } = setup(
      {
        ...EMPTY_PROJECTION,
        preferences: { ...EMPTY_PROJECTION.preferences, locale: "zh-CN" },
      },
      { onOpenProject: vi.fn(), poll: false },
    );

    expect(root.querySelector('[data-d1-welcome]')?.getAttribute("aria-label")).toBe(
      "欢迎使用 Viden",
    );
    expect(root.textContent).toContain("本地优先的智能开发工作区");
    expect(root.textContent).toContain("打开项目");
    expect(root.textContent).toContain("最近项目");
    controller.dispose();
  });

  test("keeps composer editable while busy and Enter queues through one typed intent", () => {
    const { root, send } = setup();
    const composer = root.querySelector<HTMLTextAreaElement>("[data-composer]");
    if (!composer) throw new Error("missing composer");
    expect(composer.disabled).toBe(false);
    composer.value = "继续验证";
    composer.dispatchEvent(new InputEvent("input", { bubbles: true }));
    composer.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));

    expect(send).toHaveBeenCalledWith({
      type: "submit",
      laneId: "lane-core",
      content: "继续验证",
    });
  });

  test("restores the sole ACP Agent as the Lane conversation", () => {
    const session = {
      sessionId: "acp-restored",
      laneId: "lane-core",
      agentId: "codex-acp",
      model: null,
      status: "running",
      task: "continue the review",
      diagnostic: null,
    };
    const { root, send } = setup({
      ...D1_PROJECTION,
      agentSessions: [session],
    });
    const composer = root.querySelector<HTMLTextAreaElement>("[data-composer]")!;
    composer.value = "continue";
    composer.dispatchEvent(new InputEvent("input", { bubbles: true }));
    composer.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));

    expect(send).toHaveBeenCalledWith({
      type: "send_agent_session_input",
      sessionId: "acp-restored",
      content: "continue",
    });
  });

  test("Cancel is offered only from typed canCancel state", () => {
    const enabled = setup();
    enabled.root.querySelector<HTMLButtonElement>("[data-cancel-turn]")?.click();
    expect(enabled.send).toHaveBeenCalledWith({ type: "cancel", laneId: "lane-core" });

    const disabled = setup({
      ...D1_PROJECTION,
      composer: { ...D1_PROJECTION.composer, canCancel: false },
    });
    expect(disabled.root.querySelector("[data-cancel-turn]")).toBeNull();
  });

  test("focuses the exact Lane selected by the D4 receipt", () => {
    const { root } = setup();
    const selected = root.querySelector<HTMLElement>('[data-lane-id="lane-core"]');
    expect(selected?.getAttribute("aria-current")).toBe("true");
  });

  test("supports keyboard-only Lane rail traversal without changing Core facts", () => {
    const projection = {
      ...D1_PROJECTION,
      lanes: [
        D1_PROJECTION.lanes[0]!,
        { ...D1_PROJECTION.lanes[0]!, id: "lane-review", role: "reviewer" },
      ],
    };
    const { root } = setup(projection);
    const first = root.querySelector<HTMLElement>('[data-lane-id="lane-core"]')!;
    const second = root.querySelector<HTMLElement>('[data-lane-id="lane-review"]')!;
    first.focus();
    first.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    expect(document.activeElement).toBe(second);
    expect(second.getAttribute("aria-current")).toBe("false");
  });

  test("streaming projection refresh preserves composer draft, focus, and selection", () => {
    const { root, controller } = setup();
    const composer = root.querySelector<HTMLTextAreaElement>("[data-composer]")!;
    composer.value = "keep this draft";
    composer.dispatchEvent(new InputEvent("input", { bubbles: true }));
    composer.focus();
    composer.setSelectionRange(5, 9);

    controller.applyProjection({
      ...D1_PROJECTION,
      transcript: [
        ...D1_PROJECTION.transcript,
        { id: "stream-next", kind: "assistant_stream", content: "More output" },
      ],
    });

    const refreshed = root.querySelector<HTMLTextAreaElement>("[data-composer]")!;
    expect(refreshed.value).toBe("keep this draft");
    expect(document.activeElement).toBe(refreshed);
    expect([refreshed.selectionStart, refreshed.selectionEnd]).toEqual([5, 9]);
  });

  test("idle identical projections do not replace the rendered viewport", () => {
    const { root, controller } = setup();
    const transcript = root.querySelector<HTMLElement>('[aria-label="Transcript"]')!;
    controller.applyProjection(D1_PROJECTION);
    expect(root.querySelector('[aria-label="Transcript"]')).toBe(transcript);
  });

  test("transport rejection restores the exact composer draft", async () => {
    document.body.innerHTML = '<main id="app"></main>';
    const root = document.querySelector<HTMLElement>("#app")!;
    const send = vi.fn(async () => {
      throw new Error("transport unavailable");
    });
    const confirmed: D1IntentResult = {
      projection: D1_PROJECTION,
      pendingCommandId: null,
      outcome: { state: "confirmed", reason: null },
    };
    renderD1Cockpit(root, D1_PROJECTION, send, async () => confirmed);
    const composer = root.querySelector<HTMLTextAreaElement>("[data-composer]")!;
    composer.value = "do not lose me";
    composer.dispatchEvent(new InputEvent("input", { bubbles: true }));
    composer.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));

    await vi.waitFor(() => {
      expect(root.querySelector<HTMLTextAreaElement>("[data-composer]")?.value).toBe(
        "do not lose me",
      );
    });
  });

  test("keeps the submitted draft pending until ordered Core facts confirm it", async () => {
    document.body.innerHTML = '<main id="app"></main>';
    const root = document.querySelector<HTMLElement>("#app")!;
    const pending: D1IntentResult = {
      projection: D1_PROJECTION,
      pendingCommandId: "queue-pending",
      outcome: { state: "pending", reason: null },
    };
    const send = vi.fn(async () => pending);
    const controller = renderD1Cockpit(root, D1_PROJECTION, send, async () => pending);
    const composer = root.querySelector<HTMLTextAreaElement>("[data-composer]")!;
    composer.value = "wait for Core";
    composer.dispatchEvent(new InputEvent("input", { bubbles: true }));
    composer.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));

    await vi.waitFor(() => {
      expect(root.querySelector<HTMLTextAreaElement>("[data-composer]")?.value).toBe(
        "wait for Core",
      );
    });

    controller.applyResult({
      projection: D1_PROJECTION,
      pendingCommandId: null,
      outcome: { state: "confirmed", reason: null },
    });
    expect(root.querySelector<HTMLTextAreaElement>("[data-composer]")?.value).toBe("");
  });

  test("Core rejection preserves the draft and renders the typed reason", async () => {
    document.body.innerHTML = '<main id="app"></main>';
    const root = document.querySelector<HTMLElement>("#app")!;
    const rejected: D1IntentResult = {
      projection: D1_PROJECTION,
      pendingCommandId: null,
      outcome: { state: "rejected", reason: "Core denied the queue" },
    };
    const send = vi.fn(async () => rejected);
    renderD1Cockpit(root, D1_PROJECTION, send, async () => rejected);
    const composer = root.querySelector<HTMLTextAreaElement>("[data-composer]")!;
    composer.value = "do not lose me";
    composer.dispatchEvent(new InputEvent("input", { bubbles: true }));
    composer.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));

    await vi.waitFor(() => {
      expect(root.querySelector<HTMLTextAreaElement>("[data-composer]")?.value).toBe(
        "do not lose me",
      );
      expect(root.querySelector("[data-d1-rejection]")?.textContent).toBe(
        "Core denied the queue",
      );
    });
  });

  test("waits for the Core Lane receipt before submitting the native task", async () => {
    document.body.innerHTML = '<main id="app"></main>';
    const root = document.querySelector<HTMLElement>("#app")!;
    const sent: D1Intent[] = [];
    const preview = {
      previewId: "preview-7",
      contentSha256: "a".repeat(64),
      laneId: "lane-7",
      branch: "codex/lane-7",
      diagnostics: [],
    };
    const lane = { ...D1_PROJECTION.lanes[0]!, id: "lane-7", branch: "codex/lane-7" };
    const send = vi.fn(async (intent: D1Intent): Promise<D1IntentResult> => {
      sent.push(intent);
      const next =
        intent.type === "preview_default_lane"
          ? { ...D1_PROJECTION, starterLanePreviews: [preview] }
          : intent.type === "create_starter_lane" || intent.type === "submit"
            ? {
                ...D1_PROJECTION,
                selectedLaneId: "lane-7",
                lanes: [...D1_PROJECTION.lanes, lane],
                starterLanePreviews: [preview],
              }
            : D1_PROJECTION;
      return {
        projection: next,
        pendingCommandId: null,
        outcome: { state: "confirmed", reason: null },
      };
    });
    renderD1Cockpit(root, D1_PROJECTION, send, async () => {
      throw new Error("unexpected poll");
    });

    root.querySelector<HTMLButtonElement>("[data-create-lane]")?.click();
    await vi.waitFor(() =>
      expect(root.querySelector('[role="menu"]')?.getAttribute("aria-busy")).toBe("false"),
    );
    root.querySelector<HTMLButtonElement>("[data-native-agent]")?.click();
    const task = root.querySelector<HTMLTextAreaElement>("[data-lane-task]")!;
    task.value = "fix the parser";
    task.dispatchEvent(new InputEvent("input", { bubbles: true }));
    root.querySelector<HTMLButtonElement>("[data-lane-task-submit]")?.click();

    await vi.waitFor(() => {
      expect(sent.at(-1)).toEqual({
        type: "submit",
        laneId: "lane-7",
        content: "fix the parser",
      });
    });
    expect(sent.slice(1, 4)).toEqual([
      { type: "preview_default_lane", preset: "coder" },
      {
        type: "create_starter_lane",
        laneId: "lane-7",
        preset: "coder",
        branch: "codex/lane-7",
        previewId: "preview-7",
        contentSha256: "a".repeat(64),
      },
      { type: "submit", laneId: "lane-7", content: "fix the parser" },
    ]);
  });

  test("resumes the native task after an approved Lane arrives on a later Core poll", async () => {
    document.body.innerHTML = '<main id="app"></main>';
    const root = document.querySelector<HTMLElement>("#app")!;
    const sent: D1Intent[] = [];
    const preview = {
      previewId: "preview-approved",
      contentSha256: "b".repeat(64),
      laneId: "lane-approved",
      branch: "codex/lane-approved",
      diagnostics: [],
    };
    const previewed: D1IntentResult = {
      projection: { ...D1_PROJECTION, starterLanePreviews: [preview] },
      pendingCommandId: null,
      outcome: { state: "confirmed", reason: null },
    };
    const pendingCreate: D1IntentResult = {
      projection: previewed.projection,
      pendingCommandId: "create-awaiting-approval",
      outcome: { state: "pending", reason: null },
    };
    const lane = {
      ...D1_PROJECTION.lanes[0]!,
      id: "lane-approved",
      branch: "codex/lane-approved",
    };
    const created: D1IntentResult = {
      projection: {
        ...D1_PROJECTION,
        selectedLaneId: "lane-approved",
        lanes: [...D1_PROJECTION.lanes, lane],
        starterLanePreviews: [preview],
      },
      pendingCommandId: null,
      outcome: { state: "confirmed", reason: null },
    };
    const send = vi.fn(async (intent: D1Intent): Promise<D1IntentResult> => {
      sent.push(intent);
      if (intent.type === "preview_default_lane") return previewed;
      if (intent.type === "create_starter_lane") return pendingCreate;
      return created;
    });
    const controller = renderD1Cockpit(
      root,
      D1_PROJECTION,
      send,
      async () => pendingCreate,
      undefined,
      undefined,
      { poll: false },
    );

    root.querySelector<HTMLButtonElement>("[data-create-lane]")?.click();
    await vi.waitFor(() =>
      expect(root.querySelector('[role="menu"]')?.getAttribute("aria-busy")).toBe("false"),
    );
    root.querySelector<HTMLButtonElement>("[data-native-agent]")?.click();
    const task = root.querySelector<HTMLTextAreaElement>("[data-lane-task]")!;
    task.value = "read README after approval";
    task.dispatchEvent(new InputEvent("input", { bubbles: true }));
    root.querySelector<HTMLButtonElement>("[data-lane-task-submit]")?.click();

    await vi.waitFor(() =>
      expect(sent.some((intent) => intent.type === "create_starter_lane")).toBe(true),
    );
    expect(sent.some((intent) => intent.type === "submit")).toBe(false);

    controller.applyResult(created);

    await vi.waitFor(() => {
      expect(sent.at(-1)).toEqual({
        type: "submit",
        laneId: "lane-approved",
        content: "read README after approval",
      });
    });
  });

  test("preserves the native task draft across ordered Core projection redraws", async () => {
    document.body.innerHTML = '<main id="app"></main>';
    const root = document.querySelector<HTMLElement>("#app")!;
    const send = vi.fn(async (): Promise<D1IntentResult> => ({
      projection: D1_PROJECTION,
      pendingCommandId: null,
      outcome: { state: "confirmed", reason: null },
    }));
    const controller = renderD1Cockpit(
      root,
      D1_PROJECTION,
      send,
      async () => {
        throw new Error("unexpected poll");
      },
      undefined,
      undefined,
      { poll: false },
    );

    root.querySelector<HTMLButtonElement>("[data-create-lane]")?.click();
    await vi.waitFor(() =>
      expect(root.querySelector('[role="menu"]')?.getAttribute("aria-busy")).toBe("false"),
    );
    root.querySelector<HTMLButtonElement>("[data-native-agent]")?.click();
    const task = root.querySelector<HTMLTextAreaElement>("[data-lane-task]")!;
    task.value = "read README only";
    task.dispatchEvent(new InputEvent("input", { bubbles: true }));

    controller.applyProjection({
      ...D1_PROJECTION,
      environment: { ...D1_PROJECTION.environment, tokenTotal: 1 },
    });

    expect(root.querySelector<HTMLTextAreaElement>("[data-lane-task]")?.value).toBe(
      "read README only",
    );
  });

  test("creates a dedicated Lane for ACP and keeps one Agent per Lane", async () => {
    document.body.innerHTML = '<main id="app"></main>';
    const root = document.querySelector<HTMLElement>("#app")!;
    const sent: D1Intent[] = [];
    const initial = {
      ...D1_PROJECTION,
      selectedLaneId: null,
      lanes: [],
      composer: {
        ...D1_PROJECTION.composer,
        editable: false,
        canCancel: false,
      },
    };
    const preview = {
      previewId: "preview-acp",
      contentSha256: "c".repeat(64),
      laneId: "lane-acp",
      branch: "codex/lane-acp",
      diagnostics: [],
    };
    const lane = {
      ...D1_PROJECTION.lanes[0]!,
      id: "lane-acp",
      branch: "codex/lane-acp",
    };
    const session = {
      sessionId: "acp-1",
      laneId: "lane-acp",
      agentId: "codex-acp",
      model: null,
      status: "running",
      task: "review the diff",
      diagnostic: null,
    };
    const send = vi.fn(async (intent: D1Intent): Promise<D1IntentResult> => {
      sent.push(intent);
      const next =
        intent.type === "preview_default_lane"
          ? { ...initial, starterLanePreviews: [preview] }
          : intent.type === "create_starter_lane"
            ? {
                ...initial,
                selectedLaneId: "lane-acp",
                lanes: [lane],
                starterLanePreviews: [preview],
              }
            : intent.type === "start_agent_session" ||
                intent.type === "send_agent_session_input"
              ? {
                  ...initial,
                  selectedLaneId: "lane-acp",
                  lanes: [lane],
                  starterLanePreviews: [preview],
                  agentSessions: [session],
                }
              : initial;
      return {
        projection: next,
        pendingCommandId: null,
        outcome: { state: "confirmed", reason: null },
      };
    });
    renderD1Cockpit(root, initial, send, async () => {
      throw new Error("unexpected poll");
    });

    root.querySelector<HTMLButtonElement>("[data-create-lane]")?.click();
    await vi.waitFor(() =>
      expect(root.querySelector('[role="menu"]')?.getAttribute("aria-busy")).toBe("false"),
    );
    root.querySelector<HTMLButtonElement>('[data-agent-id="codex-acp"]')?.click();
    const task = root.querySelector<HTMLTextAreaElement>("[data-lane-task]")!;
    task.value = "review the diff";
    task.dispatchEvent(new InputEvent("input", { bubbles: true }));
    root.querySelector<HTMLButtonElement>("[data-lane-task-submit]")?.click();

    await vi.waitFor(() => {
      expect(sent.at(-1)).toEqual({
        type: "start_agent_session",
        laneId: "lane-acp",
        agentId: "codex-acp",
        model: null,
        task: "review the diff",
      });
    });
    expect(sent.slice(1, 4)).toEqual([
      { type: "preview_default_lane", preset: "coder" },
      {
        type: "create_starter_lane",
        laneId: "lane-acp",
        preset: "coder",
        branch: "codex/lane-acp",
        previewId: "preview-acp",
        contentSha256: "c".repeat(64),
      },
      {
        type: "start_agent_session",
        laneId: "lane-acp",
        agentId: "codex-acp",
        model: null,
        task: "review the diff",
      },
    ]);
    expect(root.querySelector("[data-agent-session-id]")).toBeNull();

    const composer = root.querySelector<HTMLTextAreaElement>("[data-composer]")!;
    composer.value = "continue";
    composer.dispatchEvent(new InputEvent("input", { bubbles: true }));
    composer.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    await vi.waitFor(() => {
      expect(sent.at(-1)).toEqual({
        type: "send_agent_session_input",
        sessionId: "acp-1",
        content: "continue",
      });
    });
    root.querySelector<HTMLButtonElement>("[data-cancel-turn]")?.click();
    await vi.waitFor(() => {
      expect(sent.at(-1)).toEqual({ type: "cancel_agent_session", sessionId: "acp-1" });
    });
  });

  test("drains an async ACP probe queue from ordered Core poll results", async () => {
    document.body.innerHTML = '<main id="app"></main>';
    const root = document.querySelector<HTMLElement>("#app")!;
    const sent: D1Intent[] = [];
    const queriedAdapters = [
      {
        agentId: "custom-acp",
        displayName: "Custom ACP",
        startability: "probe_required",
        diagnostics: [],
      },
      {
        agentId: "kiro-cli",
        displayName: "Kiro",
        startability: "probe_required",
        diagnostics: [],
      },
      {
        agentId: "codex-acp",
        displayName: "Codex",
        startability: "probe_required",
        diagnostics: [],
      },
      {
        agentId: "claude-acp",
        displayName: "Claude",
        startability: "probe_required",
        diagnostics: [],
      },
    ];
    let current = { ...D1_PROJECTION, agentAdapters: [] as typeof queriedAdapters };
    let probePolls = 0;
    let releaseCodexProbe = false;
    const send = vi.fn(async (intent: D1Intent): Promise<D1IntentResult> => {
      sent.push(intent);
      if (intent.type === "query_agent_adapters") {
        current = { ...current, agentAdapters: queriedAdapters };
      } else if (intent.type === "probe_agent_adapter") {
        if (intent.agentId === "codex-acp") {
          return {
            projection: current,
            pendingCommandId: "probe-codex-pending",
            outcome: { state: "pending", reason: null },
          };
        }
        current = {
          ...current,
          agentAdapters: current.agentAdapters.map((adapter) =>
            adapter.agentId === intent.agentId
              ? { ...adapter, startability: "ready" }
              : adapter,
          ),
        };
      } else if (intent.type === "start_agent_session") {
        current = {
          ...current,
          agentSessions: [
            {
              sessionId: "acp-probed",
              laneId: intent.laneId,
              agentId: intent.agentId,
              model: null,
              status: "running",
              task: intent.task,
              diagnostic: null,
            },
          ],
        };
      }
      return {
        projection: current,
        pendingCommandId: null,
        outcome: { state: "confirmed", reason: null },
      };
    });
    const poll = vi.fn(async (): Promise<D1IntentResult> => {
      probePolls += 1;
      if (!releaseCodexProbe || probePolls <= 3) {
        return {
          projection: current,
          pendingCommandId: "probe-codex-pending",
          outcome: { state: "pending", reason: null },
        };
      }
      current = {
        ...current,
        agentAdapters: current.agentAdapters.map((adapter) =>
          adapter.agentId === "codex-acp"
            ? { ...adapter, startability: "ready" }
            : adapter,
        ),
      };
      return {
        projection: current,
        pendingCommandId: null,
        outcome: { state: "confirmed", reason: null },
      };
    });
    renderD1Cockpit(root, current, send, poll);

    root.querySelector<HTMLButtonElement>("[data-create-lane]")?.click();
    await vi.waitFor(() => {
      expect(sent).toEqual([
        { type: "query_agent_adapters" },
        { type: "probe_agent_adapter", agentId: "codex-acp" },
      ]);
      expect(root.querySelector('[role="menu"]')?.getAttribute("aria-busy")).toBe("true");
      expect(
        Array.from(root.querySelectorAll('[role="menuitem"]')).every(
          (item) => item.getAttribute("aria-disabled") === "true",
        ),
      ).toBe(true);
    });

    root
      .querySelector<HTMLElement>('[role="menu"]')
      ?.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(root.querySelector('[role="menu"]')).toBeNull();
    root.querySelector<HTMLButtonElement>("[data-create-lane]")?.click();
    await vi.waitFor(() =>
      expect(root.querySelector('[role="menu"]')?.getAttribute("aria-busy")).toBe("true"),
    );
    expect(sent).toEqual([
      { type: "query_agent_adapters" },
      { type: "probe_agent_adapter", agentId: "codex-acp" },
    ]);
    await vi.waitFor(() => expect(probePolls).toBeGreaterThanOrEqual(3), { timeout: 2_000 });
    expect(sent).toEqual([
      { type: "query_agent_adapters" },
      { type: "probe_agent_adapter", agentId: "codex-acp" },
    ]);
    releaseCodexProbe = true;

    await vi.waitFor(
      () => {
        expect(probePolls).toBeGreaterThanOrEqual(4);
        expect(sent.slice(0, 5)).toEqual([
          { type: "query_agent_adapters" },
          { type: "probe_agent_adapter", agentId: "codex-acp" },
          { type: "probe_agent_adapter", agentId: "claude-acp" },
          { type: "probe_agent_adapter", agentId: "kiro-cli" },
          { type: "probe_agent_adapter", agentId: "custom-acp" },
        ]);
        expect(root.querySelector('[role="menu"]')?.getAttribute("aria-busy")).toBe("false");
        expect(
          Array.from(root.querySelectorAll<HTMLElement>("[data-agent-id]")).map(
            (item) => item.dataset.agentId,
          ),
        ).toEqual(["codex-acp", "claude-acp", "kiro-cli", "custom-acp"]);
      },
      { timeout: 2_000 },
    );

    expect(root.querySelector("[data-agent-session-id]")).toBeNull();
  });

  test("serializes discovery behind a stale periodic poll before accepting terminal facts", async () => {
    document.body.innerHTML = '<main id="app"></main>';
    const root = document.querySelector<HTMLElement>("#app")!;
    const sent: D1Intent[] = [];
    const adapter = {
      agentId: "codex-acp",
      displayName: "Codex",
      startability: "probe_required",
      diagnostics: [] as string[],
    };
    let current: D1IntentResult["projection"] = { ...D1_PROJECTION, agentAdapters: [] };
    let resolveStalePoll!: (result: D1IntentResult) => void;
    const stalePoll = new Promise<D1IntentResult>((resolve) => {
      resolveStalePoll = resolve;
    });
    let pollCall = 0;
    const poll = vi.fn(async (): Promise<D1IntentResult> => {
      pollCall += 1;
      if (pollCall === 1) return stalePoll;
      if (pollCall === 2) {
        current = { ...current, agentAdapters: [adapter] };
        return {
          projection: current,
          pendingCommandId: null,
          outcome: { state: "confirmed", reason: null },
        };
      }
      current = {
        ...current,
        agentAdapters: [{ ...adapter, startability: "ready" }],
      };
      return {
        projection: current,
        pendingCommandId: null,
        outcome: { state: "confirmed", reason: null },
      };
    });
    const send = vi.fn(async (intent: D1Intent): Promise<D1IntentResult> => {
      sent.push(intent);
      return {
        projection: current,
        pendingCommandId:
          intent.type === "query_agent_adapters" ? "query-pending" : "probe-pending",
        outcome: { state: "pending", reason: null },
      };
    });
    renderD1Cockpit(root, current, send, poll);

    await vi.waitFor(() => expect(poll).toHaveBeenCalledTimes(1));
    root.querySelector<HTMLButtonElement>("[data-create-lane]")?.click();
    await Promise.resolve();
    expect(sent).toEqual([]);

    resolveStalePoll({
      projection: current,
      pendingCommandId: null,
      outcome: { state: "idle", reason: null },
    });

    await vi.waitFor(
      () => {
        expect(sent).toEqual([
          { type: "query_agent_adapters" },
          { type: "probe_agent_adapter", agentId: "codex-acp" },
        ]);
        expect(root.querySelector('[role="menu"]')?.getAttribute("aria-busy")).toBe("false");
        expect(
          root.querySelector('[data-agent-id="codex-acp"]')?.getAttribute("aria-disabled"),
        ).toBe("false");
      },
      { timeout: 2_000 },
    );
  });

  test("drops a deferred discovery result after the cockpit is disposed", async () => {
    document.body.innerHTML = '<main id="app"></main>';
    const root = document.querySelector<HTMLElement>("#app")!;
    const sent: D1Intent[] = [];
    let resolveQuery!: (result: D1IntentResult) => void;
    const deferredQuery = new Promise<D1IntentResult>((resolve) => {
      resolveQuery = resolve;
    });
    const send = vi.fn(async (intent: D1Intent): Promise<D1IntentResult> => {
      sent.push(intent);
      return deferredQuery;
    });
    const controller = renderD1Cockpit(
      root,
      D1_PROJECTION,
      send,
      async () => ({
        projection: D1_PROJECTION,
        pendingCommandId: null,
        outcome: { state: "idle", reason: null },
      }),
      undefined,
      undefined,
      { poll: false },
    );

    root.querySelector<HTMLButtonElement>("[data-create-lane]")?.click();
    await vi.waitFor(() => expect(sent).toEqual([{ type: "query_agent_adapters" }]));
    controller.dispose();
    root.innerHTML = '<p data-next-page="true">next page</p>';
    const disposedMarkup = root.innerHTML;
    resolveQuery({
      projection: {
        ...D1_PROJECTION,
        agentAdapters: [
          {
            agentId: "codex-acp",
            displayName: "Codex",
            startability: "probe_required",
            diagnostics: [],
          },
        ],
      },
      pendingCommandId: null,
      outcome: { state: "confirmed", reason: null },
    });
    await new Promise((resolve) => window.setTimeout(resolve, 0));
    expect(root.innerHTML).toBe(disposedMarkup);
    expect(sent).toEqual([{ type: "query_agent_adapters" }]);
  });

  test("continues after a failed probe without repeating or skipping adapters", async () => {
    document.body.innerHTML = '<main id="app"></main>';
    const root = document.querySelector<HTMLElement>("#app")!;
    const sent: D1Intent[] = [];
    let current: D1IntentResult["projection"] = {
      ...D1_PROJECTION,
      agentAdapters: [
        {
          agentId: "claude-acp",
          displayName: "Claude",
          startability: "probe_required",
          diagnostics: [],
        },
        {
          agentId: "codex-acp",
          displayName: "Codex",
          startability: "probe_required",
          diagnostics: [],
        },
      ],
    };
    const send = vi.fn(async (intent: D1Intent): Promise<D1IntentResult> => {
      sent.push(intent);
      if (intent.type === "probe_agent_adapter" && intent.agentId === "codex-acp") {
        throw new Error("codex probe transport failed");
      }
      if (intent.type === "probe_agent_adapter") {
        current = {
          ...current,
          agentAdapters: current.agentAdapters.map((adapter) =>
            adapter.agentId === intent.agentId
              ? { ...adapter, startability: "unavailable", diagnostics: ["not signed in"] }
              : adapter,
          ),
        };
      }
      return {
        projection: current,
        pendingCommandId: null,
        outcome: { state: "confirmed", reason: null },
      };
    });
    renderD1Cockpit(root, current, send, async () => ({
      projection: current,
      pendingCommandId: null,
      outcome: { state: "idle", reason: null },
    }));

    root.querySelector<HTMLButtonElement>("[data-create-lane]")?.click();
    await vi.waitFor(() =>
      expect(root.querySelector('[role="menu"]')?.getAttribute("aria-busy")).toBe("false"),
    );

    expect(sent).toEqual([
      { type: "query_agent_adapters" },
      { type: "probe_agent_adapter", agentId: "codex-acp" },
      { type: "probe_agent_adapter", agentId: "claude-acp" },
    ]);
    expect(
      root.querySelector('[data-agent-id="claude-acp"]')?.getAttribute("aria-disabled"),
    ).toBe("true");
  });

  test("renders and retries the sole failed Agent as a Lane-level action", async () => {
    const failed = {
      sessionId: "acp-failed",
      laneId: "lane-core",
      agentId: "codex-acp",
      model: null,
      status: "failed",
      task: "review",
      diagnostic: "recoverable",
    };
    const projection = { ...D1_PROJECTION, agentSessions: [failed] };
    const { root, send } = setup(projection);

    const lane = root.querySelector<HTMLElement>('[data-lane-id="lane-core"]');
    expect(lane?.dataset.laneAgentId).toBe("codex-acp");
    expect(lane?.textContent).toContain("codex-acp");
    expect(root.querySelector("[data-agent-session-id]")).toBeNull();
    root.querySelector<HTMLButtonElement>(
      '[data-retry-lane-agent="lane-core"]',
    )?.click();

    expect(send).toHaveBeenCalledWith({
      type: "retry_agent_session",
      sessionId: "acp-failed",
    });
  });
});
