import { translate, type Locale, type MessageKey } from "../i18n/catalog";
import type { D1CockpitProjection } from "../models/workspace";
import { environmentValues } from "./environment";
import { boundedLiveWorkEntries } from "./live_work";

function definition(list: HTMLDListElement, label: string, value: string): void {
  const term = document.createElement("dt");
  term.textContent = label;
  const detail = document.createElement("dd");
  detail.textContent = value;
  list.append(term, detail);
}

function emptyState(kind: string, text: string): HTMLElement {
  const element = document.createElement("p");
  element.className = "d1-empty";
  element.dataset.typedEmpty = kind;
  element.textContent = text;
  return element;
}

function statusLabel(locale: Locale, status: string): string {
  const keys: Record<string, MessageKey> = {
    connected: "d1.context.connected",
    ready: "d1.context.ready",
    degraded: "d1.context.degraded",
    offline: "d1.context.offline",
    unavailable: "d1.unavailable",
    added: "d1.status.added",
    modified: "d1.status.modified",
    deleted: "d1.status.deleted",
    renamed: "d1.status.renamed",
    untracked: "d1.status.untracked",
    running: "d1.status.running",
    passed: "d1.status.passed",
    failed: "d1.status.failed",
    cancelled: "d1.status.cancelled",
    queued: "d1.status.queued",
  };
  const key = keys[status];
  return key ? translate(locale, key, {} as never) : status;
}

function dirtyValue(locale: Locale, dirty: boolean): string {
  return translate(locale, dirty ? "d1.context.dirty.true" : "d1.context.dirty.false", {});
}

function appendSection(
  dock: HTMLElement,
  id: string,
  title: string,
  renderBody: (body: HTMLElement) => void,
): void {
  const section = document.createElement("section");
  section.dataset.contextSection = id;
  section.setAttribute("aria-label", title);

  const heading = document.createElement("h2");
  const button = document.createElement("button");
  const body = document.createElement("div");
  const bodyId = `d1-context-section-${id}`;
  button.type = "button";
  button.dataset.contextSectionToggle = "true";
  button.setAttribute("aria-expanded", "true");
  button.setAttribute("aria-controls", bodyId);
  button.textContent = title;
  button.addEventListener("click", () => {
    const expanded = button.getAttribute("aria-expanded") === "true";
    button.setAttribute("aria-expanded", String(!expanded));
    body.hidden = expanded;
  });
  button.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    button.click();
  });
  body.id = bodyId;
  body.dataset.contextSectionBody = id;
  renderBody(body);
  heading.append(button);
  section.append(heading, body);
  dock.append(section);
}

function selectedLane(
  projection: D1CockpitProjection,
): D1CockpitProjection["lanes"][number] | null {
  return projection.lanes.find((lane) => lane.id === projection.selectedLaneId) ?? null;
}

function renderLaneAgent(
  body: HTMLElement,
  projection: D1CockpitProjection,
  locale: Locale,
): void {
  const lane = selectedLane(projection);
  const owner = projection.contextDock.laneAgent;
  if (!lane || !owner || owner.laneId !== lane.id) {
    body.append(emptyState("lane-agent", translate(locale, "d1.context.noAgentOwner", {})));
    return;
  }
  const sessions = projection.agentSessions.filter((session) => session.laneId === lane.id);
  if (sessions.length > 1 || (sessions[0] && sessions[0].sessionId !== owner.sessionId)) {
    body.append(emptyState("lane-agent", translate(locale, "d1.context.noAgentOwner", {})));
    return;
  }
  const session = sessions[0] ?? null;
  const list = document.createElement("dl");
  list.dataset.laneAgent = "true";
  definition(list, translate(locale, "d1.context.lane", {}), owner.laneId);
  definition(
    list,
    translate(locale, "d1.context.route", {}),
    session ? translate(locale, "d1.context.acp", {}) : translate(locale, "d1.context.native", {}),
  );
  definition(
    list,
    translate(locale, "d1.environment.model", {}),
    session?.model ?? projection.contextDock.provider?.model ?? "—",
  );
  definition(
    list,
    translate(locale, "d1.context.status", {}),
    statusLabel(locale, session?.status ?? lane.status),
  );
  definition(list, translate(locale, "d1.context.session", {}), owner.sessionId ?? "—");
  body.append(list);
}

export function renderContextDock(
  projection: D1CockpitProjection,
  locale: Locale,
  matchesSelectedLane = true,
): HTMLElement {
  const dock = document.createElement("aside");
  dock.id = "d1-context-dock";
  dock.className = "envp d1-right";
  dock.dataset.shellLandmark = "context-dock";
  dock.dataset.cockpitRole = "context";
  dock.dataset.contextDock = "true";
  dock.dataset.drawerOpen = "false";
  if (!matchesSelectedLane) {
    const waiting = document.createElement("p");
    waiting.dataset.contextDockWaiting = "true";
    waiting.textContent = translate(locale, "d1.context.switching", {});
    dock.append(waiting);
    return dock;
  }

  appendSection(dock, "environment", translate(locale, "d1.environment", {}), (body) => {
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
    body.append(environmentFacts);
  });
  appendSection(
    dock,
    "changes-source",
    translate(locale, "d1.context.changesSource", {}),
    (body) => {
      const source = projection.contextDock.source;
      if (!source) {
        body.append(emptyState("source", translate(locale, "d1.context.noSource", {})));
        return;
      }
      const facts = document.createElement("dl");
      definition(facts, translate(locale, "d4.branch", {}), source.branch ?? "—");
      definition(facts, translate(locale, "d4.worktree", {}), source.worktree ?? "—");
      definition(facts, translate(locale, "d1.context.ahead", {}), String(source.ahead));
      definition(facts, translate(locale, "d1.context.behind", {}), String(source.behind));
      definition(facts, translate(locale, "d1.context.dirty", {}), dirtyValue(locale, source.dirty));
      body.append(facts);
    },
  );
  appendSection(dock, "context", translate(locale, "d1.context.context", {}), (body) => {
    const context = projection.contextDock.context;
    if (!context) {
      body.append(emptyState("context", translate(locale, "d1.context.noTypedContext", {})));
      return;
    }
    const facts = document.createElement("dl");
    definition(facts, translate(locale, "d1.context.budget", {}), context.budgetId);
    definition(
      facts,
      translate(locale, "d1.environment.tokens", {}),
      `${context.usedTokens}/${context.hardTokenLimit}`,
    );
    definition(facts, translate(locale, "d1.context.remaining", {}), String(context.remainingTokens));
    body.append(facts);
  });
  appendSection(dock, "lane-agent", translate(locale, "d1.context.laneAgent", {}), (body) => {
    renderLaneAgent(body, projection, locale);
  });
  appendSection(dock, "sources", translate(locale, "d1.context.sources", {}), (body) => {
    const source = projection.contextDock.source;
    if (!source) {
      body.append(emptyState("sources", translate(locale, "d1.context.noSource", {})));
      return;
    }
    const facts = document.createElement("dl");
    definition(facts, translate(locale, "d1.status.added", {}), String(source.added));
    definition(facts, translate(locale, "d1.status.deleted", {}), String(source.deleted));
    definition(
      facts,
      translate(locale, "d1.status.modified", {}),
      dirtyValue(locale, source.dirty),
    );
    body.append(facts);
  });
  appendSection(dock, "mcp", translate(locale, "d1.context.mcp", {}), (body) => {
    const services = projection.contextDock.services.filter((service) => service.kind === "mcp");
    if (services.length === 0) {
      body.append(emptyState("mcp", translate(locale, "d1.context.noServices", {})));
      return;
    }
    for (const service of services) {
      const item = document.createElement("div");
      item.className = "d1-context-item";
      item.dataset.serviceId = service.id;
      item.textContent = `${service.label} · ${statusLabel(locale, service.status)}`;
      body.append(item);
    }
  });
  appendSection(dock, "lsp", translate(locale, "d1.context.lsp", {}), (body) => {
    const services = projection.contextDock.services.filter((service) => service.kind === "lsp");
    if (services.length === 0) {
      body.append(emptyState("lsp", translate(locale, "d1.context.noServices", {})));
      return;
    }
    for (const service of services) {
      const item = document.createElement("div");
      item.className = "d1-context-item";
      item.dataset.serviceId = service.id;
      item.textContent = `${service.label} · ${statusLabel(locale, service.status)}`;
      body.append(item);
    }
  });
  appendSection(dock, "task-checklist", translate(locale, "d1.context.taskChecklist", {}), (body) => {
    if (projection.contextDock.checklist.length === 0) {
      body.append(emptyState("task-checklist", translate(locale, "d1.context.noChecklist", {})));
      return;
    }
    for (const item of projection.contextDock.checklist) {
      const row = document.createElement("div");
      row.className = "d1-context-item";
      row.dataset.checklistItem = item.id;
      row.textContent = `${item.label} · ${statusLabel(locale, item.status)}`;
      body.append(row);
    }
  });
  for (const { text } of boundedLiveWorkEntries(projection, locale)) {
    const item = document.createElement("div");
    item.className = "d1-work-item";
    item.textContent = text;
    dock.append(item);
  }
  for (const unavailable of projection.unavailableFeatures) {
    const item = document.createElement("div");
    item.className = "d1-unavailable";
    item.dataset.unavailableFeature = unavailable.id;
    item.setAttribute("aria-disabled", "true");
    item.textContent = `${translate(locale, "d1.unavailable", {})} · ${unavailable.id}`;
    dock.append(item);
  }
  return dock;
}
