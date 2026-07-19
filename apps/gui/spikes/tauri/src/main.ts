import "../../../../../docs/viden-design/Viden/tokens.css";
import { D1Slice, fixtureProjection, renderD1Slice } from "./app";

const root = document.querySelector<HTMLElement>("#app");
if (!root) {
  throw new Error("missing #app mount point");
}
renderD1Slice(root, new D1Slice(fixtureProjection()));
