import type { D1CockpitProjection } from "../models/workspace";

export function toolRowText(tool: D1CockpitProjection["liveWork"]["tools"][number]): string {
  return `${tool.name} · ${tool.inputPreview}`;
}
