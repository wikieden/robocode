/**
 * Cross-project recent work, exactly as Core's `RecentWorkLoaded` fact
 * publishes it.
 *
 * These mirror the host's `RecentProjectProjection` / `RecentSessionProjection`
 * one-to-one, which in turn mirror Core's `RecentProjectSummary` /
 * `RecentSessionSummary` whitelist DTOs. The frontend must not extend them with
 * a derived title, a transcript path, or a preview: those fields are excluded
 * by the contract on purpose, and a client that invented them would be
 * publishing session content Core deliberately withheld.
 *
 * Core also owns the ordering — sessions by
 * `(last_updated_at DESC, canonical_root ASC, session_id ASC)`, projects
 * aggregated from that already-bounded list — so the GUI renders the arrays in
 * the order they arrive and never re-sorts or re-truncates them.
 */

/**
 * The frontend-contract-v1 capability Core publishes for this inventory. The
 * client names the exact id in its unavailable copy so the gap is checkable
 * against Core's handshake rather than described in prose.
 */
export const RECENT_WORK_CAPABILITY = "runtime.recent_work";

export interface RecentProjectView {
  canonicalRoot: string;
  displayName: string;
  /** Seconds since the epoch, as Core recorded it. */
  lastUpdatedAt: number;
  latestSessionId: string | null;
}

export interface RecentSessionView {
  canonicalRoot: string;
  sessionId: string;
  createdAt: number;
  lastUpdatedAt: number;
  messageCount: number;
  toolCallCount: number;
  commandCount: number;
}

export interface RecentWorkResult {
  outcome: { state: "idle" | "pending" | "confirmed" | "rejected"; reason: string | null };
  projects: RecentProjectView[];
  sessions: RecentSessionView[];
  /** Core's own inventory diagnostics, rendered verbatim. */
  diagnostics: string[];
  pendingCommandId: string | null;
  /** False when Core's handshake published no `runtime.recent_work`. */
  capabilityAvailable: boolean;
}

/**
 * What a screen may render for the Recent section.
 *
 * The three states are distinct on purpose: an absent capability, a Core
 * rejection, and a genuinely empty inventory are different facts, and
 * collapsing them into one empty list would misreport Core.
 */
export type RecentWorkState =
  | { kind: "loading" }
  | { kind: "unavailable"; reason: string }
  | { kind: "failed"; reason: string }
  | { kind: "loaded"; projects: RecentProjectView[]; diagnostics: string[] };

/**
 * Coarse "how long ago" buckets for a Core timestamp.
 *
 * Presentation only: the authority stays the epoch seconds Core published, and
 * the bucket is recomputed on every render rather than stored.
 */
export function relativeAge(
  lastUpdatedAt: number,
  now: number,
): { unit: "now" | "minutes" | "hours" | "days" | "weeks"; count: number } {
  const seconds = Math.max(0, Math.floor(now / 1000) - lastUpdatedAt);
  if (seconds < 60) return { unit: "now", count: 0 };
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return { unit: "minutes", count: minutes };
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return { unit: "hours", count: hours };
  const days = Math.floor(hours / 24);
  if (days < 7) return { unit: "days", count: days };
  return { unit: "weeks", count: Math.floor(days / 7) };
}
