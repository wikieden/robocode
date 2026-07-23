export function adjacentLaneId(
  laneIds: readonly string[],
  index: number,
  direction: "previous" | "next",
): string | null {
  if (laneIds.length === 0) return null;
  const offset = direction === "next" ? 1 : -1;
  return laneIds[(index + offset + laneIds.length) % laneIds.length] ?? null;
}
