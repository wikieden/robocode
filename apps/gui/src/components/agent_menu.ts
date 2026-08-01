import { translate, type Locale } from "../i18n/catalog";
import { createAgentLogo, resolveAgentLogoKind, type AgentLogoKind } from "./agent_logo";
import "./agent_menu.css";

export type AgentMenuSelection = { kind: "native" } | { kind: "acp"; agentId: string };

export interface AgentMenuModel {
  locale: Locale;
  canCreateLane: boolean;
  usesGitIsolation: boolean;
  probing: boolean;
  eligibilityDiagnostic: string | null;
  selected?: AgentMenuSelection;
  taskDraft?: string;
  submitting?: boolean;
  adapters: Array<{
    agentId: string;
    displayName: string;
    startability: string;
    diagnostics: string[];
  }>;
}

export interface AgentMenuController {
  root: HTMLElement;
  close: () => void;
}

const BUILT_IN_AGENT_ORDER = new Map([
  ["codex-acp", 0],
  ["claude-acp", 1],
  ["kiro-cli", 2],
  ["custom-acp", 3],
]);

export function orderedAgentAdapters<T extends { agentId: string }>(
  adapters: readonly T[],
): T[] {
  return adapters
    .map((adapter, index) => ({ adapter, index }))
    .sort((left, right) => {
      const leftRank = BUILT_IN_AGENT_ORDER.get(left.adapter.agentId) ?? 4;
      const rightRank = BUILT_IN_AGENT_ORDER.get(right.adapter.agentId) ?? 4;
      return leftRank - rightRank || left.index - right.index;
    })
    .map(({ adapter }) => adapter);
}

function selectionKey(selection: AgentMenuSelection): string {
  return selection.kind === "native" ? "native" : `acp:${selection.agentId}`;
}

function taskSlug(task: string): string {
  return (
    task
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 26) || "new-lane"
  );
}

export function renderAgentMenu(
  anchor: HTMLButtonElement,
  model: AgentMenuModel,
  onSelect: (selection: AgentMenuSelection) => void,
  onClose: () => void = () => undefined,
  onCreate: (selection: AgentMenuSelection, task: string) => Promise<boolean> | boolean = () =>
    true,
  onTaskChange: (task: string) => void = () => undefined,
  onFullSetup: () => void = () => undefined,
  onCompositionChange: (composing: boolean, task: string) => void = () => undefined,
): AgentMenuController {
  const menu = document.createElement("div");
  menu.className = "agent-menu new-lane-popover";
  menu.dataset.newLanePopover = "true";
  menu.setAttribute("role", "dialog");
  menu.setAttribute("aria-label", translate(model.locale, "d1.agentMenu.title", {}));
  menu.setAttribute("aria-busy", String(model.probing));

  let selected = model.selected ?? (model.canCreateLane ? { kind: "native" } : undefined);
  let taskDraft = model.taskDraft ?? "";
  let composing = false;

  const header = document.createElement("header");
  header.className = "agent-menu-header";
  const heading = document.createElement("h2");
  heading.className = "agent-menu-heading";
  heading.textContent = translate(model.locale, "d1.agentMenu.newLane", {});
  const scope = document.createElement("span");
  scope.className = "agent-menu-scope";
  scope.dataset.laneScope = "true";
  scope.textContent = translate(model.locale, "d1.agentMenu.scopeCurrent", {});
  const closeButton = document.createElement("button");
  closeButton.type = "button";
  closeButton.className = "agent-menu-close";
  closeButton.setAttribute("aria-label", translate(model.locale, "d1.agentMenu.close", {}));
  closeButton.textContent = "×";
  closeButton.addEventListener("click", () => close());
  header.append(heading, scope, closeButton);

  const diagnostic = document.createElement("p");
  diagnostic.className = "agent-menu-diagnostic";
  diagnostic.setAttribute("role", "status");

  const agentGrid = document.createElement("div");
  agentGrid.className = "agent-menu-agents";
  agentGrid.setAttribute("role", "radiogroup");
  agentGrid.setAttribute("aria-label", translate(model.locale, "d1.agentMenu.agentLabel", {}));
  const agentLabel = document.createElement("p");
  agentLabel.className = "agent-menu-label";
  agentLabel.textContent = translate(model.locale, "d1.agentMenu.agentLabel", {});

  const select = (next: AgentMenuSelection): void => {
    selected = next;
    onSelect(next);
    sync();
  };

  const item = (
    label: string,
    status: string | null,
    enabled: boolean,
    selection: AgentMenuSelection,
    logoKind: AgentLogoKind | null,
    badge: string,
  ): HTMLButtonElement => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "agent-menu-item";
    button.setAttribute("role", "radio");
    button.tabIndex = -1;
    button.setAttribute("aria-disabled", String(!enabled));
    button.setAttribute("aria-checked", "false");
    button.setAttribute("aria-pressed", "false");
    if (selection.kind === "acp") button.dataset.agentId = selection.agentId;
    if (selection.kind === "native") button.dataset.nativeAgent = "true";
    button.dataset.selectionKey = selectionKey(selection);
    const primary = document.createElement("span");
    primary.className = "agent-menu-primary";
    if (logoKind) primary.append(createAgentLogo(logoKind));
    const name = document.createElement("span");
    name.className = "agent-menu-name";
    name.textContent = label;
    primary.append(name);
    const kind = document.createElement("span");
    kind.className = "agent-menu-badge";
    kind.textContent = badge;
    button.append(primary, kind);
    if (status) {
      const detail = document.createElement("small");
      detail.className = "agent-menu-item-detail";
      detail.textContent = status;
      button.append(detail);
    }
    button.addEventListener("click", () => {
      if (!enabled) return;
      select(selection);
    });
    return button;
  };

  agentGrid.append(
    item(
      translate(model.locale, "d1.agentMenu.viden", {}),
      model.canCreateLane ? null : model.eligibilityDiagnostic,
      model.canCreateLane,
      { kind: "native" },
      "viden",
      translate(model.locale, "d1.agentMenu.builtIn", {}),
    ),
  );
  for (const adapter of orderedAgentAdapters(model.adapters)) {
    const status =
      model.probing
        ? translate(model.locale, "d1.agentMenu.probing", {})
        : adapter.startability === "ready"
        ? null
        : adapter.diagnostics[0] ?? adapter.startability.replaceAll("_", " ");
    agentGrid.append(
      item(
        adapter.displayName,
        status,
        model.canCreateLane && !model.probing && adapter.startability === "ready",
        { kind: "acp", agentId: adapter.agentId },
        resolveAgentLogoKind(adapter.agentId, adapter.displayName),
        "ACP",
      ),
    );
  }
  if (model.adapters.length === 0) {
    const empty = document.createElement("p");
    empty.className = "agent-menu-empty";
    empty.textContent = translate(
      model.locale,
      model.probing ? "d1.agentMenu.probing" : "d1.agentMenu.empty",
      {},
    );
    agentGrid.append(empty);
  }

  const taskLabel = document.createElement("label");
  taskLabel.className = "agent-menu-task";
  taskLabel.textContent = translate(model.locale, "d1.task.label", {});
  const task = document.createElement("textarea");
  task.dataset.laneTask = "true";
  task.rows = 4;
  task.value = taskDraft;
  task.placeholder = translate(model.locale, "d1.task.placeholder", {});
  task.setAttribute("aria-label", translate(model.locale, "d1.task.label", {}));
  taskLabel.append(task);

  const hint = document.createElement("p");
  hint.className = "agent-menu-hint";
  hint.dataset.laneHint = "true";
  const branch = document.createElement("span");
  branch.dataset.laneBranch = "true";
  const isolation = document.createElement("span");
  isolation.dataset.laneIsolation = "true";
  hint.append(branch, isolation);

  const actions = document.createElement("div");
  actions.className = "agent-menu-actions";
  const fullSetup = document.createElement("button");
  fullSetup.type = "button";
  fullSetup.className = "agent-menu-full-setup";
  fullSetup.dataset.fullSetup = "true";
  fullSetup.textContent = translate(model.locale, "d1.agentMenu.fullSetup", {});
  const create = document.createElement("button");
  create.type = "button";
  create.className = "agent-menu-create";
  create.dataset.laneTaskSubmit = "true";
  create.textContent = translate(model.locale, "d1.task.submit", {});
  actions.append(fullSetup, create);

  menu.append(header, diagnostic, agentLabel, agentGrid, taskLabel, hint, actions);

  anchor.setAttribute("aria-expanded", "true");
  const anchorRect = anchor.getBoundingClientRect();
  menu.style.setProperty("--agent-menu-anchor-inline", `${anchorRect.right}px`);
  menu.style.setProperty("--agent-menu-anchor-block", `${anchorRect.bottom}px`);
  // The canonical New Lane composer is a window-level overlay. Portalling it
  // keeps the action row outside the auto-hiding Lane rail's clipping context.
  (anchor.closest(".d1-frame")?.parentElement ?? document.body).append(menu);

  function sync(): void {
    const selectedKey = selected ? selectionKey(selected) : null;
    for (const candidate of menu.querySelectorAll<HTMLButtonElement>("[data-selection-key]")) {
      const isSelected = candidate.dataset.selectionKey === selectedKey;
      candidate.classList.toggle("on", isSelected);
      candidate.setAttribute("aria-checked", String(isSelected));
      candidate.setAttribute("aria-pressed", String(isSelected));
    }
    if (model.usesGitIsolation) {
      const slug = taskSlug(taskDraft);
      branch.textContent = `⌁ vd/${slug}`;
      isolation.textContent = translate(model.locale, "d1.agentMenu.isolationLocal", {});
    } else {
      branch.textContent = translate(model.locale, "d1.task.directWorkspaceHint", {});
      isolation.textContent = translate(model.locale, "d1.agentMenu.isolationDirect", {});
    }
    create.disabled =
      model.submitting === true ||
      model.probing ||
      selected === undefined ||
      taskDraft.trim().length === 0;
    create.textContent = translate(model.locale, "d1.task.submit", {});
    diagnostic.textContent =
      model.eligibilityDiagnostic ??
      (model.probing ? translate(model.locale, "d1.agentMenu.probing", {}) : "");
    diagnostic.hidden = diagnostic.textContent.length === 0;
  }

  const enabledItems = (): HTMLButtonElement[] =>
    Array.from(menu.querySelectorAll<HTMLButtonElement>("[data-selection-key]")).filter(
      (candidate) => candidate.getAttribute("aria-disabled") !== "true",
    );
  const focusAt = (index: number): void => {
    const items = enabledItems();
    if (items.length === 0) {
      menu.tabIndex = -1;
      menu.focus();
      return;
    }
    items.forEach((candidate) => (candidate.tabIndex = -1));
    const target = items[(index + items.length) % items.length]!;
    target.tabIndex = 0;
    target.focus();
  };
  menu.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      close();
      return;
    }
    const target = event.target as HTMLElement | null;
    if (!(target instanceof HTMLButtonElement) || !target.dataset.selectionKey) return;
    const items = enabledItems();
    const current = items.indexOf(target);
    if (event.key === "ArrowDown") {
      event.preventDefault();
      focusAt(current + 1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      focusAt(current - 1);
    } else if (event.key === "Home") {
      event.preventDefault();
      focusAt(0);
    } else if (event.key === "End") {
      event.preventDefault();
      focusAt(items.length - 1);
    }
  });
  task.addEventListener("compositionstart", () => {
    composing = true;
    onCompositionChange(true, task.value);
  });
  task.addEventListener("compositionend", () => {
    composing = false;
    taskDraft = task.value;
    onTaskChange(taskDraft);
    onCompositionChange(false, taskDraft);
  });
  task.addEventListener("input", () => {
    taskDraft = task.value;
    onTaskChange(taskDraft);
    sync();
  });
  task.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      close();
      return;
    }
    if (event.key !== "Enter" || (!event.metaKey && !event.ctrlKey) || composing) return;
    event.preventDefault();
    void submit();
  });
  fullSetup.addEventListener("click", () => {
    onFullSetup();
    close();
  });
  create.addEventListener("click", () => void submit());

  async function submit(): Promise<void> {
    const trimmed = taskDraft.trim();
    if (!selected || !trimmed || create.disabled) return;
    create.disabled = true;
    const accepted = await onCreate(selected, trimmed);
    if (accepted) close();
    else sync();
  }

  let closed = false;
  const outside = (event: MouseEvent): void => {
    if (!menu.contains(event.target as Node) && event.target !== anchor) close();
  };
  function close(): void {
    if (closed) return;
    closed = true;
    document.removeEventListener("mousedown", outside);
    menu.remove();
    anchor.setAttribute("aria-expanded", "false");
    anchor.focus();
    onClose();
  }
  document.addEventListener("mousedown", outside);
  sync();
  task.focus();
  return { root: menu, close };
}
