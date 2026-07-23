import { translate, type Locale } from "../i18n/catalog";
import "./welcome_center.css";

const BRAND_MARK_URL = new URL(
  "../../../../docs/viden-design/Viden/brand-assets/icon.svg",
  import.meta.url,
).href;

export function renderWelcomeCenter(
  root: HTMLElement,
  locale: Locale,
  openProject?: () => void | Promise<void>,
): void {
  const welcome = document.createElement("section");
  welcome.className = "d1-welcome";
  welcome.dataset.d1Welcome = "true";
  welcome.setAttribute("aria-label", translate(locale, "d1.welcome.title", {}));

  const content = document.createElement("div");
  content.className = "d1-welcome-content";

  const brand = document.createElement("header");
  brand.className = "d1-welcome-brand";
  const mark = document.createElement("img");
  mark.src = BRAND_MARK_URL;
  mark.alt = "";
  mark.width = 92;
  mark.height = 92;
  const brandCopy = document.createElement("div");
  const eyebrow = document.createElement("p");
  eyebrow.className = "d1-welcome-eyebrow";
  eyebrow.textContent = translate(locale, "d1.welcome.eyebrow", {});
  const title = document.createElement("h2");
  title.textContent = translate(locale, "d1.welcome.title", {});
  const subtitle = document.createElement("p");
  subtitle.className = "d1-welcome-subtitle";
  subtitle.textContent = translate(locale, "d1.welcome.subtitle", {});
  brandCopy.append(eyebrow, title, subtitle);
  brand.append(mark, brandCopy);

  const start = document.createElement("section");
  start.className = "d1-welcome-section";
  const startTitle = document.createElement("h3");
  startTitle.textContent = translate(locale, "d1.welcome.getStarted", {});
  const open = document.createElement("button");
  open.type = "button";
  open.className = "d1-welcome-action";
  open.dataset.openProject = "true";
  open.setAttribute("aria-keyshortcuts", "Meta+O Control+O");
  open.disabled = !openProject;
  const openLabel = document.createElement("span");
  openLabel.textContent = translate(locale, "d1.welcome.openProject", {});
  const shortcut = document.createElement("kbd");
  shortcut.textContent = "⌘ O";
  open.append(openLabel, shortcut);
  open.addEventListener("click", () => {
    if (!openProject || open.disabled) return;
    open.disabled = true;
    open.setAttribute("aria-busy", "true");
    welcome.querySelector("[data-open-project-error]")?.remove();
    void Promise.resolve(openProject())
      .catch((error: unknown) => {
        const message = document.createElement("p");
        message.className = "d1-welcome-error";
        message.dataset.openProjectError = "true";
        message.setAttribute("role", "alert");
        message.textContent = String(error);
        start.append(message);
      })
      .finally(() => {
        if (!welcome.isConnected) return;
        open.disabled = false;
        open.removeAttribute("aria-busy");
      });
  });
  start.append(startTitle, open);

  const recent = document.createElement("section");
  recent.className = "d1-welcome-section";
  const recentTitle = document.createElement("h3");
  recentTitle.textContent = translate(locale, "d1.welcome.recent", {});
  const unavailable = document.createElement("div");
  unavailable.className = "d1-welcome-unavailable";
  unavailable.dataset.unavailableFeature = "recent-work";
  unavailable.setAttribute("aria-disabled", "true");
  const unavailableTitle = document.createElement("strong");
  unavailableTitle.textContent = translate(locale, "d1.welcome.recentUnavailable", {});
  const unavailableDetail = document.createElement("span");
  unavailableDetail.textContent = translate(locale, "d1.welcome.recentHint", {});
  const contract = document.createElement("code");
  contract.textContent = "GUI-CORE-007";
  unavailable.append(unavailableTitle, unavailableDetail, contract);
  recent.append(recentTitle, unavailable);

  content.append(brand, start, recent);
  welcome.append(content);
  root.replaceChildren(welcome);
}
