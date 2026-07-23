import { DENSITIES, VALID_SKIN_MODE_PAIRS } from "../ui/theme";
import "./component_gallery.css";

export type VisualScreen = "d1" | "d11" | "d4" | "d6" | "gallery";
export type VisualLocale = "en" | "zh-CN";
export type VisualMotion = "system" | "reduced";
export type VisualViewport = "desktop" | "narrow" | "scaled-font";

export interface VisualEvidenceCase {
  id: string;
  screen: VisualScreen;
  locale: VisualLocale;
  skinMode: (typeof VALID_SKIN_MODE_PAIRS)[number];
  density: (typeof DENSITIES)[number];
  motion: VisualMotion;
  viewport: VisualViewport;
  width: number;
  height: number;
  fontScale: number;
  designReference: string;
  nonColorCues: true;
}

const DESIGN_REFERENCES: Record<VisualScreen, string> = {
  d1: "docs/viden-design/Viden/GUI/Viden - 桌面驾驶舱 (GUI).html",
  d11: "docs/viden-design/Viden/GUI/pages/Viden - D11 首启与项目接入 (GUI).html",
  d4: "docs/viden-design/Viden/GUI/pages/Viden - D4 Lane创建流程 (GUI).html",
  d6: "docs/viden-design/Viden/GUI/pages/Viden - D6 空态与错误态 (GUI).html",
  gallery: "docs/viden-design/Viden/GUI/Viden - 组件库 (GUI).html",
};

const SCREENS = Object.keys(DESIGN_REFERENCES) as VisualScreen[];
const LOCALES: readonly VisualLocale[] = ["en", "zh-CN"];

function viewportMetrics(viewport: VisualViewport): Pick<VisualEvidenceCase, "width" | "height" | "fontScale"> {
  if (viewport === "narrow") return { width: 760, height: 900, fontScale: 1 };
  if (viewport === "scaled-font") return { width: 1440, height: 900, fontScale: 1.25 };
  return { width: 1440, height: 900, fontScale: 1 };
}

function visualCase(
  screen: VisualScreen,
  locale: VisualLocale,
  skinMode: VisualEvidenceCase["skinMode"] = "aurora/dark",
  density: VisualEvidenceCase["density"] = "regular",
  motion: VisualMotion = "system",
  viewport: VisualViewport = "desktop",
): VisualEvidenceCase {
  const id = [screen, locale, skinMode.replace("/", "-"), density, motion, viewport].join("--");
  return {
    id,
    screen,
    locale,
    skinMode,
    density,
    motion,
    viewport,
    ...viewportMetrics(viewport),
    designReference: DESIGN_REFERENCES[screen],
    nonColorCues: true,
  };
}

/**
 * The matrix is deliberately pairwise rather than a 480-shot Cartesian product.
 * Every screen is bilingual and covers the layout/motion boundaries, while D1 and
 * the gallery carry the complete token/density axes used by all subordinate screens.
 */
export function buildVisualMatrix(): VisualEvidenceCase[] {
  const cases = new Map<string, VisualEvidenceCase>();
  const add = (entry: VisualEvidenceCase) => cases.set(entry.id, entry);

  for (const screen of SCREENS) {
    for (const locale of LOCALES) {
      for (const viewport of ["desktop", "narrow", "scaled-font"] as const) {
        add(visualCase(screen, locale, "aurora/dark", "regular", "system", viewport));
      }
      add(visualCase(screen, locale, "aurora/dark", "regular", "reduced", "desktop"));
    }
  }

  for (const screen of ["d1", "gallery"] as const) {
    for (const locale of LOCALES) {
      for (const skinMode of VALID_SKIN_MODE_PAIRS) {
        add(visualCase(screen, locale, skinMode));
      }
      for (const density of DENSITIES) {
        add(visualCase(screen, locale, "aurora/dark", density));
      }
    }
  }

  return [...cases.values()].sort((left, right) => left.id.localeCompare(right.id));
}

type GalleryLocale = Record<string, string>;

const GALLERY_COPY: Record<VisualLocale, GalleryLocale> = {
  en: {
    title: "Component gallery",
    subtitle: "Keyboard, state, density, and accessibility contract",
    actions: "Actions",
    create: "Create Lane",
    disabled: "Unavailable action",
    inputs: "Inputs",
    projectPath: "Project path",
    projectPlaceholder: "/workspace/project",
    validation: "Choose an existing project directory.",
    lanes: "Lane states",
    running: "Running",
    approval: "Needs approval",
    live: "Streaming response in starter-coder",
    provider: "Provider unavailable. Retry after checking credentials.",
    permission: "Permission decision",
    once: "Allow once",
    deny: "Deny",
    transcript: "Transcript row",
    assistant: "Assistant",
    content: "The bounded transcript keeps the selected row visible.",
    status: "Connected · reduced-motion ready",
  },
  "zh-CN": {
    title: "组件画廊",
    subtitle: "键盘、状态、密度与可访问性契约",
    actions: "操作",
    create: "创建 Lane",
    disabled: "不可用操作",
    inputs: "输入",
    projectPath: "项目路径",
    projectPlaceholder: "/工作区/项目",
    validation: "请选择已存在的项目目录。",
    lanes: "Lane 状态",
    running: "运行中",
    approval: "需要审批",
    live: "starter-coder 正在流式输出",
    provider: "Provider 不可用，请检查凭据后重试。",
    permission: "权限决策",
    once: "仅允许一次",
    deny: "拒绝",
    transcript: "转录行",
    assistant: "助手",
    content: "有界转录保持选中行可见。",
    status: "已连接 · 支持减少动效",
  },
};

function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function card(title: string, component: string): { card: HTMLElement; body: HTMLElement } {
  const wrapper = element("section", "gallery-card");
  wrapper.dataset.component = component;
  const heading = element("h2", "gallery-card-title", title);
  const body = element("div", "gallery-card-body");
  wrapper.append(heading, body);
  return { card: wrapper, body };
}

export function renderComponentGallery(root: HTMLElement, locale: VisualLocale): void {
  const copy = GALLERY_COPY[locale];
  document.documentElement.lang = locale;

  const frame = element("section", "frame component-gallery");
  frame.dataset.screen = "component-gallery";

  const titlebar = element("header", "winbar gallery-titlebar");
  const heading = element("h1", "gallery-title", copy.title);
  const subtitle = element("p", "gallery-subtitle", copy.subtitle);
  titlebar.append(heading, subtitle);

  const grid = element("main", "gallery-grid");

  const actions = card(copy.actions, "actions");
  const create = element("button", "gitchip gallery-primary", copy.create);
  create.type = "button";
  create.setAttribute("aria-label", copy.create);
  const disabled = element("button", "gitchip", copy.disabled);
  disabled.type = "button";
  disabled.disabled = true;
  disabled.setAttribute("aria-label", copy.disabled);
  actions.body.append(create, disabled);

  const inputs = card(copy.inputs, "inputs");
  const inputLabel = element("label", "gallery-field-label", copy.projectPath);
  const input = element("input", "gallery-input");
  input.type = "text";
  input.placeholder = copy.projectPlaceholder;
  input.setAttribute("aria-label", copy.projectPath);
  input.setAttribute("aria-describedby", "gallery-project-hint");
  input.setAttribute("aria-invalid", "true");
  const hint = element("p", "gallery-field-hint", copy.validation);
  hint.id = "gallery-project-hint";
  inputLabel.append(input);
  inputs.body.append(inputLabel, hint);

  const lanes = card(copy.lanes, "lane-rows");
  const running = element("div", "wslane s-work");
  running.setAttribute("role", "status");
  running.append(element("strong", "lid", "starter-coder"), element("span", "nm", copy.running));
  const approval = element("div", "wslane s-need");
  approval.setAttribute("role", "status");
  approval.append(element("strong", "lid", "starter-reviewer"), element("span", "nm", copy.approval));
  lanes.body.append(running, approval);

  const streaming = card(copy.live, "live-region");
  const live = element("p", "gallery-live", copy.live);
  live.setAttribute("role", "status");
  live.setAttribute("aria-live", "polite");
  streaming.body.append(live);

  const provider = card(copy.provider, "provider-alert");
  const alert = element("p", "gallery-alert", copy.provider);
  alert.setAttribute("role", "alert");
  provider.body.append(alert);

  const permission = card(copy.permission, "permission");
  const permissionSurface = element("div", "gperm");
  const permissionHeading = element("div", "gperm-hd", copy.permission);
  const permissionOptions = element("div", "gperm-opts");
  const allowOnce = element("button", "gperm-opt on", copy.once);
  allowOnce.type = "button";
  const deny = element("button", "gperm-opt deny", copy.deny);
  deny.type = "button";
  permissionOptions.append(allowOnce, deny);
  permissionSurface.append(permissionHeading, permissionOptions);
  permission.body.append(permissionSurface);

  const transcript = card(copy.transcript, "transcript-row");
  const row = element("article", "d1-row");
  row.append(element("span", "d1-row-kind", copy.assistant), element("p", "gallery-transcript-content", copy.content));
  transcript.body.append(row);

  const badge = card(copy.status, "statusbar");
  const status = element("p", "gallery-state-label", copy.status);
  status.setAttribute("role", "status");
  badge.body.append(status);

  grid.append(
    actions.card,
    inputs.card,
    lanes.card,
    streaming.card,
    provider.card,
    permission.card,
    transcript.card,
    badge.card,
  );

  const statusbar = element("footer", "statusbar", copy.status);
  frame.append(titlebar, grid, statusbar);
  root.replaceChildren(frame);
}
