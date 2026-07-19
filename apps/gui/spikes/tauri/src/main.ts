import "../../../../../docs/viden-design/Viden/tokens.css";
import { invoke } from "@tauri-apps/api/core";
import { createFixtureD1Slice, D1Slice, renderD1Slice, type ProjectionState } from "./app";

export type ProjectionLoader = () => Promise<ProjectionState>;

export async function bootstrapD1(
  root: HTMLElement,
  loadProjection: ProjectionLoader = () => invoke<ProjectionState>("d1_fixture_projection"),
): Promise<D1Slice> {
  const projection = await loadProjection();
  const app = createFixtureD1Slice(projection);
  renderD1Slice(root, app);
  return app;
}

const root = document.querySelector<HTMLElement>("#app");
if (!root) {
  throw new Error("missing #app mount point");
}
await bootstrapD1(root);
