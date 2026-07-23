import { translate, type Locale } from "../i18n/catalog";
import "./agent_menu.css";

export type AgentMenuSelection = { kind: "native" } | { kind: "acp"; agentId: string };

export interface AgentMenuModel {
  locale: Locale;
  canCreateLane: boolean;
  probing: boolean;
  eligibilityDiagnostic: string | null;
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

export function renderAgentMenu(
  anchor: HTMLButtonElement,
  model: AgentMenuModel,
  onSelect: (selection: AgentMenuSelection) => void,
  onClose: () => void = () => undefined,
): AgentMenuController {
  const menu = document.createElement("div");
  menu.className = "agent-menu";
  menu.setAttribute("role", "menu");
  menu.setAttribute("aria-label", translate(model.locale, "d1.agentMenu.title", {}));
  menu.setAttribute("aria-busy", String(model.probing));

  const section = (label: string): HTMLElement => {
    const heading = document.createElement("div");
    heading.className = "agent-menu-section";
    heading.setAttribute("role", "presentation");
    heading.textContent = label;
    return heading;
  };
  const item = (
    label: string,
    status: string | null,
    enabled: boolean,
    selection: AgentMenuSelection,
  ): HTMLButtonElement => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "agent-menu-item";
    button.setAttribute("role", "menuitem");
    button.tabIndex = -1;
    button.setAttribute("aria-disabled", String(!enabled));
    if (selection.kind === "acp") button.dataset.agentId = selection.agentId;
    if (selection.kind === "native") button.dataset.nativeAgent = "true";
    const name = document.createElement("span");
    name.textContent = label;
    button.append(name);
    if (status) {
      const detail = document.createElement("small");
      detail.textContent = status;
      button.append(detail);
    }
    button.addEventListener("click", () => {
      if (!enabled) return;
      close();
      // Close first so the selection handler can hand focus to the task
      // prompt without the menu restoring focus to its trigger afterward.
      onSelect(selection);
    });
    return button;
  };

  menu.append(section(translate(model.locale, "d1.agentMenu.newLane", {})));
  menu.append(
    item(
      translate(model.locale, "d1.agentMenu.viden", {}),
      model.canCreateLane ? null : model.eligibilityDiagnostic,
      model.canCreateLane,
      { kind: "native" },
    ),
  );
  for (const adapter of orderedAgentAdapters(model.adapters)) {
    const status =
      model.probing
        ? translate(model.locale, "d1.agentMenu.probing", {})
        : adapter.startability === "ready"
        ? translate(model.locale, "d1.agentMenu.ready", {})
        : adapter.diagnostics[0] ?? adapter.startability.replaceAll("_", " ");
    menu.append(
      item(
        adapter.displayName,
        status,
        model.canCreateLane && !model.probing && adapter.startability === "ready",
        { kind: "acp", agentId: adapter.agentId },
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
    menu.append(empty);
  }

  anchor.setAttribute("aria-expanded", "true");
  anchor.parentElement?.append(menu);
  const enabledItems = (): HTMLButtonElement[] =>
    Array.from(menu.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')).filter(
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
    const items = enabledItems();
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    if (event.key === "Escape") {
      event.preventDefault();
      close();
    } else if (event.key === "ArrowDown") {
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
  focusAt(0);
  return { root: menu, close };
}
