import { translate, type Locale, type MessageKey } from "../i18n/catalog";

export const D1_ACTIVITY_ITEMS = [
  { icon: "chat", key: "d1.activity.work" },
  { icon: "worktree", key: "d1.activity.search" },
  { icon: "lanes", key: "d1.activity.lanes" },
  { icon: "review", key: "d1.activity.git" },
  { icon: "evidence", key: "d1.activity.evidence" },
  { icon: "diagnostics", key: "d1.activity.diagnostics" },
  { icon: "inbox", key: "d1.activity.inbox" },
] as const satisfies ReadonlyArray<{
  icon: CanonicalGuiIcon;
  key: MessageKey;
}>;

export type CanonicalGuiIcon =
  | "chat"
  | "worktree"
  | "lanes"
  | "review"
  | "evidence"
  | "diagnostics"
  | "inbox"
  | "panel";

const SVG_NAMESPACE = "http://www.w3.org/2000/svg";

function svgNode<K extends keyof SVGElementTagNameMap>(
  name: K,
  attributes: Record<string, string>,
): SVGElementTagNameMap[K] {
  const node = document.createElementNS(SVG_NAMESPACE, name);
  for (const [key, value] of Object.entries(attributes)) node.setAttribute(key, value);
  return node;
}

/**
 * Uses the exact registered paths from the design package's `GUI/gui-icons.jsx`.
 * Keeping this adapter data-only prevents the production client from loading the
 * prototype's React/Babel runtime while preserving the canonical icon artwork.
 */
export function createCanonicalGuiIcon(name: CanonicalGuiIcon): SVGSVGElement {
  const svg = svgNode("svg", {
    class: "ic",
    viewBox: "0 0 24 24",
    "aria-hidden": "true",
  });
  if (name === "chat") {
    svg.append(
      svgNode("path", {
        d: "M21 11.5a8.38 8.38 0 0 1-8.5 8.5 8.5 8.5 0 0 1-3.8-.9L3 21l1.9-5.7A8.38 8.38 0 0 1 4 11.5 8.5 8.5 0 0 1 12.5 3 8.38 8.38 0 0 1 21 11.5z",
      }),
    );
  } else if (name === "worktree") {
    svg.append(
      svgNode("circle", { cx: "6", cy: "6", r: "2.4" }),
      svgNode("circle", { cx: "6", cy: "18", r: "2.4" }),
      svgNode("circle", { cx: "18", cy: "9", r: "2.4" }),
      svgNode("path", { d: "M6 8.4v7.2M18 11.4c0 3-3 3.6-5.4 3.6H8.4" }),
    );
  } else if (name === "lanes") {
    svg.append(
      svgNode("path", {
        d: "M4 5v14M12 5v14M20 5v14",
        "stroke-dasharray": "3 3",
      }),
      svgNode("circle", { cx: "4", cy: "10", r: "2.2" }),
      svgNode("circle", { cx: "12", cy: "14", r: "2.2" }),
      svgNode("circle", { cx: "20", cy: "8", r: "2.2" }),
    );
  } else if (name === "review") {
    svg.append(
      svgNode("path", {
        d: "M9 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h4M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4M12 4v16",
      }),
    );
  } else if (name === "evidence") {
    svg.append(
      svgNode("path", { d: "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" }),
      svgNode("path", { d: "M14 2v6h6" }),
      svgNode("path", { d: "M9 13h6M9 17h4" }),
    );
  } else if (name === "diagnostics") {
    svg.append(svgNode("path", { d: "M3 12h4l3 8 4-16 3 8h4" }));
  } else if (name === "inbox") {
    svg.append(svgNode("path", { d: "M22 12h-6l-2 3h-4l-2-3H2M5 5h14l3 7v6a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2v-6z" }));
  } else {
    svg.append(
      svgNode("rect", { x: "3", y: "4", width: "18", height: "16", rx: "2" }),
      svgNode("path", { d: "M15 4v16" }),
    );
  }
  return svg;
}

/// Rail slots that open a restored screen.
///
/// `worktree` follows the D12 design page, whose own rail highlights that slot
/// for the integration gate. The remaining three are a provisional routing
/// decision, not a design-backed one: the accepted rail has no `decide`,
/// `gallery`, or `monitor` slot, so the screens are reachable here rather than
/// staying unreachable outside a URL.
export const D1_RAIL_ROUTES: Partial<Record<string, string>> = {
  "d1.activity.search": "d12",
  "d1.activity.git": "d2",
  "d1.activity.evidence": "d14",
  "d1.activity.diagnostics": "d10",
  "d1.activity.inbox": "d13",
};

export interface ActivityRailOptions {
  lanesAvailable: boolean;
  lanesOpen: boolean;
  onToggleLanes: () => void;
  /// Opens a restored screen. Absent while no Core projection is bound, which
  /// keeps every routing slot disabled rather than opening an empty screen.
  onNavigate?: (route: string) => void;
}

export function renderActivityRail(
  locale: Locale,
  options: ActivityRailOptions,
): HTMLElement {
  const activity = document.createElement("nav");
  activity.className = "act d1-activity";
  activity.dataset.shellLandmark = "activity-rail";
  activity.dataset.cockpitRole = "activity";
  activity.setAttribute("aria-label", translate(locale, "d1.activity", {}));
  for (const activityItem of D1_ACTIVITY_ITEMS) {
    const item = document.createElement("button");
    item.type = "button";
    item.className = "actbtn d1-action";
    item.title = translate(locale, activityItem.key, {});
    item.setAttribute("aria-label", translate(locale, activityItem.key, {}));
    item.append(createCanonicalGuiIcon(activityItem.icon));
    if (activityItem.key === "d1.activity.work") {
      item.classList.add("on");
      item.setAttribute("aria-current", "page");
    } else if (activityItem.key === "d1.activity.lanes") {
      item.dataset.lanesToggle = "true";
      item.disabled = !options.lanesAvailable;
      if (options.lanesAvailable) {
        item.setAttribute("aria-controls", "d1-lane-rail");
        item.setAttribute("aria-expanded", String(options.lanesOpen));
        item.addEventListener("click", options.onToggleLanes);
        item.addEventListener("keydown", (event) => {
          if (!["Enter", " "].includes(event.key)) return;
          event.preventDefault();
          item.click();
        });
      }
    } else if (D1_RAIL_ROUTES[activityItem.key] && options.onNavigate) {
      const route = D1_RAIL_ROUTES[activityItem.key] as string;
      item.dataset.railRoute = route;
      item.addEventListener("click", () => options.onNavigate?.(route));
    } else {
      item.disabled = true;
    }
    activity.append(item);
  }
  const spacer = document.createElement("span");
  spacer.className = "actspacer";
  spacer.ariaHidden = "true";
  activity.append(spacer);
  return activity;
}
