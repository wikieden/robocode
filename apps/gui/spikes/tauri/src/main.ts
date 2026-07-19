import "../../../../../docs/viden-design/Viden/tokens.css";
import { invoke } from "@tauri-apps/api/core";
import { D1Slice, renderD1Slice, type ProjectionState } from "./app";

const root = document.querySelector<HTMLElement>("#app");
if (!root) {
  throw new Error("missing #app mount point");
}
const projection = await invoke<ProjectionState>("d1_fixture_projection");
renderD1Slice(root, new D1Slice(projection));
