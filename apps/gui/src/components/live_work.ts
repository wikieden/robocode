import type { D1CockpitProjection } from "../models/workspace";

export function taskRowText(task: D1CockpitProjection["liveWork"]["tasks"][number]): string {
  return `${task.title} · ${task.status} · ${task.progress}%`;
}

export function approvalRowText(
  approval: D1CockpitProjection["liveWork"]["approvals"][number],
): string {
  return `${approval.title} · ${approval.risk}`;
}
