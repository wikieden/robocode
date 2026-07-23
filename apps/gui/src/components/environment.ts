import type { D1CockpitProjection } from "../models/workspace";

export function environmentValues(environment: D1CockpitProjection["environment"]): string[] {
  return [
    environment.providerId,
    environment.model,
    environment.workMode,
    environment.permissionLevel,
    String(environment.tokenTotal),
    environment.costMicroUsd === null
      ? "—"
      : `$${(environment.costMicroUsd / 1_000_000).toFixed(6)}`,
  ];
}
