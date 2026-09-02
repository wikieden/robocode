import type { Locale } from "../i18n/catalog";
import "./d14_audit_timeline.css";

/// D14 audit and timeline.
///
/// Two host-computed modes over two different Core surfaces:
///
/// - **Audit mode** (default) is the Core audit contract: `QueryAudit` ->
///   `AuditPageLoaded`. It answers "who changed what, on which objects, with
///   what outcome" from the append-only audit store.
/// - **Raw replay mode** (diagnostic) is the Core replay stream paged through
///   its cursor. It answers "which ordered events did Core emit", stays usable
///   when the audit capability is absent, renders rows in cursor order, keeps
///   an undecodable event visible, and shows a replay failure instead of a
///   shorter, complete-looking trail.
///
/// This file is render-only. Every fact — including whether audit mode is
/// available at all and what an audit view is scoped to — comes from the host
/// projection; nothing here derives, sorts, or localizes a Core value.

export interface D14Row {
  sequence: number;
  streamId: string;
  kind: string;
  known: boolean;
  timestamp: number | null;
  projectId: string;
  laneId: string | null;
  sessionId: string | null;
  taskId: string | null;
}

export interface D14AuditTimelineProjection {
  rows: D14Row[];
  nextCursor: string | null;
  complete: boolean;
}

export interface D14Controller {
  applyProjection: (projection: D14AuditTimelineProjection) => void;
}

/// One object an audit record linked, exactly as Core published it.
export interface D14AuditObject {
  kind: string;
  id: string;
}

export interface D14AuditArg {
  key: string;
  value: string;
}

export interface D14AuditRow {
  auditId: string;
  timestamp: number;
  /** Core project the record belongs to; empty when Core claimed no scope. */
  projectId: string;
  /** Lane the record belongs to, when Core named one. */
  laneId: string | null;
  /** `operator`, `agent`, `system`, or `unknown` for an actor this build cannot name. */
  actorKind: string;
  agentId: string | null;
  /** Core's stable dotted key (`gate.decided`). Rendered raw, never localized. */
  action: string;
  objects: D14AuditObject[];
  /** `success`, `denied`, `failed`, or `unknown`. */
  outcome: string;
  args: D14AuditArg[];
}

export interface D14AuditScope {
  kind: string;
  id: string;
}

export interface D14AuditProjection {
  outcome: { state: string; reason: string | null };
  rows: D14AuditRow[];
  nextBefore: string | null;
  complete: boolean;
  /** Whether a page has actually arrived. Absence is not emptiness. */
  loaded: boolean;
  pendingCommandId: string | null;
  /** False when Core's handshake published no `runtime.audit`. */
  capabilityAvailable: boolean;
  scope: D14AuditScope | null;
}

export type D14Mode = "audit" | "raw";

/// The host reads D14 needs. Each one is a Core-owned read; the screen owns no
/// query, no cursor arithmetic, and no capability decision of its own.
export interface D14Ports {
  /** One `QueryAudit` for the newest page, optionally object-scoped. */
  queryAudit: (scope: D14AuditScope | null) => Promise<D14AuditProjection>;
  /** One `QueryAudit` for the page older than Core's own cursor. */
  loadOlderAudit: () => Promise<D14AuditProjection>;
  /** One raw replay page; `null` starts from the stream head. */
  loadRaw: (after: string | null) => Promise<D14AuditTimelineProjection>;
}

const AUDIT_CAPABILITY = "runtime.audit";

type Copy = Record<string, string>;

const COPY: Record<Locale, Copy> = {
  en: {
    title: "Raw event replay (diagnostic)",
    more: "Load older events",
    complete: "Core reports the replay is complete.",
    empty: "Core replayed no event.",
    unknown: "unknown event kind",
    failed: "Replay failed; the trail below is incomplete.",
    lane: "lane",
    auditTitle: "Audit trail",
    modeAudit: "Audit trail",
    modeRaw: "Raw event replay (diagnostic)",
    auditMore: "Load older records",
    auditComplete: "Core reports the audit timeline is complete.",
    auditEmpty: "Core recorded no audit entry for this view.",
    auditLoading: "Waiting for Core to answer the audit query.",
    auditFailed: "Core refused the audit query.",
    auditUnavailable:
      "Core did not publish {capability}; the audit trail is unavailable and this is the raw event replay instead.",
    scopePrefix: "Scoped to",
    scopeClear: "Remove scope",
  },
  "zh-CN": {
    title: "原始事件回放（诊断）",
    more: "加载更早事件",
    complete: "Core 报告回放已完整。",
    empty: "Core 未回放任何事件。",
    unknown: "未知事件类型",
    failed: "回放失败，下方轨迹不完整。",
    lane: "lane",
    auditTitle: "审计轨迹",
    modeAudit: "审计轨迹",
    modeRaw: "原始事件回放（诊断）",
    auditMore: "加载更早记录",
    auditComplete: "Core 报告审计时间线已完整。",
    auditEmpty: "Core 未为该视图记录任何审计条目。",
    auditLoading: "等待 Core 回应审计查询。",
    auditFailed: "Core 拒绝了该审计查询。",
    auditUnavailable: "Core 未发布 {capability}，审计轨迹不可用，此处显示的是原始事件回放。",
    scopePrefix: "范围",
    scopeClear: "移除范围",
  },
};

export function renderD14AuditTimeline(
  root: HTMLElement,
  initial: D14AuditTimelineProjection,
  locale: Locale,
  loadMore?: (after: string) => Promise<D14AuditTimelineProjection>,
): D14Controller {
  let projection = initial;
  let failure: string | null = null;
  let busy = false;
  const copy = COPY[locale];

  const render = (): void => {
    const stage = document.createElement("section");
    stage.className = "d14-stage";
    stage.dataset.route = "d14";
    stage.setAttribute("aria-busy", String(busy));

    const head = document.createElement("div");
    head.className = "d14-head";
    const heading = document.createElement("h2");
    heading.className = "d14-title";
    heading.textContent = copy.title;
    const count = document.createElement("span");
    count.className = "d14-count";
    count.dataset.d14Rows = String(projection.rows.length);
    count.textContent = String(projection.rows.length);
    head.append(heading, count);
    stage.append(head);

    // A replay failure is louder than the rows it truncated.
    if (failure) {
      const error = document.createElement("p");
      error.className = "d14-error";
      error.dataset.d14Error = "true";
      error.setAttribute("role", "status");
      error.textContent = `${copy.failed} ${failure}`;
      stage.append(error);
    }

    const list = document.createElement("ol");
    list.className = "d14-list";
    if (projection.rows.length === 0) {
      const empty = document.createElement("li");
      empty.className = "d14-muted";
      empty.dataset.d14Empty = "true";
      empty.textContent = copy.empty;
      list.append(empty);
    }
    for (const row of projection.rows) {
      const item = document.createElement("li");
      item.className = "d14-row";
      item.dataset.d14Sequence = String(row.sequence);
      item.dataset.d14Kind = row.kind;
      if (!row.known) item.dataset.d14Unknown = "true";

      const seq = document.createElement("span");
      seq.className = "d14-seq";
      seq.textContent = `#${row.sequence}`;
      const kind = document.createElement("span");
      kind.className = "d14-kind";
      kind.textContent = row.known ? row.kind : `${row.kind} · ${copy.unknown}`;
      const scope = document.createElement("span");
      scope.className = "d14-scope";
      scope.textContent = [row.projectId, row.laneId && `${copy.lane} ${row.laneId}`]
        .filter((part): part is string => Boolean(part))
        .join(" · ");
      item.append(seq, kind, scope);

      if (row.timestamp !== null) {
        const time = document.createElement("time");
        time.className = "d14-time";
        time.dateTime = new Date(row.timestamp * 1000).toISOString();
        time.textContent = String(row.timestamp);
        item.append(time);
      }
      list.append(item);
    }
    stage.append(list);

    const footer = document.createElement("div");
    footer.className = "d14-foot";
    if (projection.complete) {
      const done = document.createElement("span");
      done.className = "d14-muted";
      done.dataset.d14Complete = "true";
      done.textContent = copy.complete;
      footer.append(done);
    } else if (projection.nextCursor && loadMore) {
      const cursor = projection.nextCursor;
      const button = document.createElement("button");
      button.type = "button";
      button.className = "d14-more";
      button.dataset.d14More = cursor;
      button.disabled = busy;
      button.textContent = copy.more;
      button.addEventListener("click", () => {
        if (busy) return;
        busy = true;
        failure = null;
        render();
        void loadMore(cursor)
          .then((next) => {
            // Pages append in Core cursor order; nothing is reordered here.
            projection = { ...next, rows: [...projection.rows, ...next.rows] };
          })
          .catch((error: unknown) => {
            failure = error instanceof Error ? error.message : String(error);
          })
          .finally(() => {
            busy = false;
            render();
          });
      });
      footer.append(button);
    }
    stage.append(footer);
    root.replaceChildren(stage);
  };

  render();
  return {
    applyProjection: (next) => {
      projection = next;
      failure = null;
      render();
    },
  };
}

/// Renders a Core unix-seconds audit timestamp as `YYYY-MM-DD HH:MM:SS UTC`.
///
/// UTC is deliberate and the zone is spelled out rather than implied: an audit
/// record is evidence that gets compared across machines, and a locale-shifted
/// clock would make two readers disagree about one fact. The TUI's audit
/// overlay fixes its clock to UTC for the same reason
/// (`apps/tui/src/tui/audit_panel.rs`). The format is intentionally not routed
/// through the i18n catalog: it is a technical timestamp, not prose.
///
/// Raw replay mode keeps its own epoch readout — it is the diagnostic event
/// log, and its value is the Core stream position rather than a wall clock.
export function formatAuditTimestamp(unixSeconds: number): string {
  const at = new Date(unixSeconds * 1000);
  // A timestamp outside the Date range must not blank the timeline; Core's own
  // unformatted value is then the honest thing to show.
  if (Number.isNaN(at.getTime())) return String(unixSeconds);
  const iso = at.toISOString();
  return `${iso.slice(0, 10)} ${iso.slice(11, 19)} UTC`;
}

/// Renders one audit-mode view into `root`.
///
/// Every value here is Core's: `action`, object kinds and ids, argument keys
/// and values, and the outcome/actor labels the host derived from Core's typed
/// enums. Only the chrome around them is localized.
function renderAuditMode(
  root: HTMLElement,
  projection: D14AuditProjection,
  copy: Copy,
  handlers: {
    onLoadOlder: () => void;
    onClearScope: () => void;
    busy: boolean;
  },
): void {
  const stage = document.createElement("section");
  stage.className = "d14-stage d14-audit";
  stage.dataset.d14Panel = "audit";
  stage.setAttribute("aria-busy", String(handlers.busy || projection.outcome.state === "pending"));

  const head = document.createElement("div");
  head.className = "d14-head";
  const heading = document.createElement("h2");
  heading.className = "d14-title";
  heading.textContent = copy.auditTitle;
  const count = document.createElement("span");
  count.className = "d14-count";
  count.dataset.d14AuditRows = String(projection.rows.length);
  count.textContent = String(projection.rows.length);
  head.append(heading, count);

  if (projection.scope) {
    // The scope is a Core query filter, not a display filter: removing it
    // re-queries rather than re-filtering rows the client already holds.
    const chip = document.createElement("button");
    chip.type = "button";
    chip.className = "d14-scope-chip";
    chip.dataset.d14ScopeClear = `${projection.scope.kind}:${projection.scope.id}`;
    chip.disabled = handlers.busy;
    chip.title = copy.scopeClear;
    chip.setAttribute("aria-label", copy.scopeClear);
    chip.textContent = `${copy.scopePrefix} ${projection.scope.kind} · ${projection.scope.id} ✕`;
    chip.addEventListener("click", handlers.onClearScope);
    head.append(chip);
  }
  stage.append(head);

  // Core's refusal is rendered verbatim; the client composes no reason of its own.
  if (projection.outcome.state === "rejected") {
    const error = document.createElement("p");
    error.className = "d14-error";
    error.dataset.d14AuditError = "true";
    error.setAttribute("role", "status");
    error.textContent = `${copy.auditFailed} ${projection.outcome.reason ?? ""}`.trim();
    stage.append(error);
  }

  const list = document.createElement("ol");
  list.className = "d14-list";
  // Emptiness is only claimed once a page actually arrived; a read that has not
  // answered yet says it is waiting.
  if (projection.rows.length === 0) {
    const note = document.createElement("li");
    note.className = "d14-muted";
    if (projection.loaded) {
      note.dataset.d14AuditEmpty = "true";
      note.textContent = copy.auditEmpty;
      list.append(note);
    } else if (projection.outcome.state === "pending") {
      note.dataset.d14AuditLoading = "true";
      note.textContent = copy.auditLoading;
      list.append(note);
    }
  }
  for (const row of projection.rows) {
    const item = document.createElement("li");
    item.className = "d14-arow";
    item.dataset.d14AuditId = row.auditId;
    item.dataset.d14Action = row.action;
    item.dataset.d14Actor = row.actorKind;
    item.dataset.d14Outcome = row.outcome;

    const time = document.createElement("time");
    time.className = "d14-time";
    const at = new Date(row.timestamp * 1000);
    if (!Number.isNaN(at.getTime())) time.dateTime = at.toISOString();
    time.textContent = formatAuditTimestamp(row.timestamp);

    const action = document.createElement("span");
    action.className = "d14-a-action";
    action.textContent = row.action;

    const actor = document.createElement("span");
    actor.className = "d14-a-actor";
    actor.textContent = row.agentId ? `${row.actorKind} · ${row.agentId}` : row.actorKind;

    const outcome = document.createElement("span");
    outcome.className = "d14-a-outcome";
    outcome.textContent = row.outcome;

    item.append(action, actor, outcome);

    if (row.objects.length > 0) {
      const objects = document.createElement("span");
      objects.className = "d14-a-objects";
      for (const object of row.objects) {
        const chip = document.createElement("span");
        chip.className = "d14-a-object";
        chip.dataset.d14Object = `${object.kind}:${object.id}`;
        chip.textContent = `${object.kind} · ${object.id}`;
        objects.append(chip);
      }
      item.append(objects);
    }
    if (row.args.length > 0) {
      const args = document.createElement("span");
      args.className = "d14-a-args";
      for (const arg of row.args) {
        const chip = document.createElement("span");
        chip.className = "d14-a-arg";
        chip.dataset.d14Arg = arg.key;
        chip.textContent = `${arg.key}=${arg.value}`;
        args.append(chip);
      }
      item.append(args);
    }
    item.append(time);
    list.append(item);
  }
  stage.append(list);

  const footer = document.createElement("div");
  footer.className = "d14-foot";
  if (projection.complete) {
    const done = document.createElement("span");
    done.className = "d14-muted";
    done.dataset.d14AuditComplete = "true";
    done.textContent = copy.auditComplete;
    footer.append(done);
  } else if (projection.nextBefore) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "d14-more";
    button.dataset.d14AuditMore = projection.nextBefore;
    button.disabled = handlers.busy;
    button.textContent = copy.auditMore;
    button.addEventListener("click", handlers.onLoadOlder);
    footer.append(button);
  }
  stage.append(footer);
  root.replaceChildren(stage);
}

/// D14's two-mode shell.
///
/// The host decides availability; this only renders the toggle and delegates to
/// the mode's own view. Audit mode is the default and the primary surface; raw
/// replay is the diagnostic fallback and is the only mode offered when Core
/// published no `runtime.audit`.
export function renderD14(
  root: HTMLElement,
  initialAudit: D14AuditProjection,
  locale: Locale,
  ports: D14Ports,
  initialRaw: D14AuditTimelineProjection | null = null,
): D14Controller {
  const copy = COPY[locale];
  let audit = initialAudit;
  let raw = initialRaw;
  let mode: D14Mode = audit.capabilityAvailable ? "audit" : "raw";
  let busy = false;

  const stage = document.createElement("section");
  stage.className = "d14-shell";
  stage.dataset.route = "d14";
  const modes = document.createElement("div");
  modes.className = "d14-modes";
  const body = document.createElement("div");
  body.className = "d14-body";
  stage.append(modes, body);
  root.replaceChildren(stage);

  const renderRaw = (): void => {
    if (!raw) {
      // The raw page is fetched on first entry so an unused mode costs no
      // replay traffic.
      busy = true;
      renderChrome();
      void ports
        .loadRaw(null)
        .then((page) => {
          raw = page;
        })
        .finally(() => {
          busy = false;
          render();
        });
      return;
    }
    renderD14AuditTimeline(body, raw, locale, (after) => ports.loadRaw(after));
  };

  const renderChrome = (): void => {
    modes.replaceChildren();
    for (const candidate of ["audit", "raw"] as const) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "d14-mode";
      button.dataset.d14Mode = candidate;
      button.setAttribute("aria-pressed", String(mode === candidate));
      // An absent capability disables audit mode outright rather than leaving
      // an enabled control that could only ever fail.
      button.disabled = busy || (candidate === "audit" && !audit.capabilityAvailable);
      button.textContent = candidate === "audit" ? copy.modeAudit : copy.modeRaw;
      button.addEventListener("click", () => {
        if (busy || mode === candidate) return;
        mode = candidate;
        render();
      });
      modes.append(button);
    }
    if (!audit.capabilityAvailable) {
      const note = document.createElement("p");
      note.className = "d14-muted";
      note.dataset.d14CapabilityNote = AUDIT_CAPABILITY;
      note.setAttribute("role", "status");
      note.textContent = copy.auditUnavailable.replace("{capability}", AUDIT_CAPABILITY);
      modes.append(note);
    }
  };

  const run = (read: () => Promise<D14AuditProjection>): void => {
    if (busy) return;
    busy = true;
    render();
    void read()
      .then((next) => {
        audit = next;
      })
      .catch((error: unknown) => {
        // A host failure is reported the same way Core's own refusal is: as a
        // rejected outcome, never as a silently shorter timeline.
        audit = {
          ...audit,
          outcome: {
            state: "rejected",
            reason: error instanceof Error ? error.message : String(error),
          },
        };
      })
      .finally(() => {
        busy = false;
        render();
      });
  };

  function render(): void {
    renderChrome();
    if (mode === "raw") {
      renderRaw();
      return;
    }
    renderAuditMode(body, audit, copy, {
      busy,
      onLoadOlder: () => run(() => ports.loadOlderAudit()),
      onClearScope: () => run(() => ports.queryAudit(null)),
    });
  }

  render();
  return {
    applyProjection: (next) => {
      raw = next;
      if (mode === "raw") render();
    },
  };
}
