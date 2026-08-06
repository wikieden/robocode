import type { Locale } from "../i18n/catalog";
import "./d14_audit_timeline.css";

/// D14 audit and timeline.
///
/// The trail is the Core replay stream paged through its cursor, not the view
/// state. Rows are rendered in cursor order, an undecodable event still gets a
/// row, and a replay failure is shown instead of a shorter, complete-looking
/// trail.

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

type Copy = Record<string, string>;

const COPY: Record<Locale, Copy> = {
  en: {
    title: "Audit and timeline",
    more: "Load older events",
    complete: "Core reports the replay is complete.",
    empty: "Core replayed no event.",
    unknown: "unknown event kind",
    failed: "Replay failed; the trail below is incomplete.",
    lane: "lane",
  },
  "zh-CN": {
    title: "审计与时间线",
    more: "加载更早事件",
    complete: "Core 报告回放已完整。",
    empty: "Core 未回放任何事件。",
    unknown: "未知事件类型",
    failed: "回放失败，下方轨迹不完整。",
    lane: "lane",
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
