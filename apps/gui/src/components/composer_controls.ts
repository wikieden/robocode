import { translate, type Locale, type MessageKey } from "../i18n/catalog";
import type { ComposerControlIntent } from "../models/composer";
import type { D1CockpitProjection } from "../models/workspace";

/// Composer meta-row selectors for work mode, permission level, and model.
///
/// Each selector dispatches its Core command and re-renders from the returned
/// snapshot only: the pills never show the optimistic click, so a coupled
/// change Core made (Plan forcing ReadOnly, ReadOnly forcing Plan) is always
/// what the operator sees. Popovers follow the agent-menu conventions:
/// Escape closes, an outside click closes, and arrow keys move focus.

export type ComposerControlKind = "work_mode" | "permission" | "model";

export interface ModelGroup {
  providerId: string;
  label: string;
  models: string[];
}

/// The Core work modes and permission levels of frontend-contract-v1, by the
/// CLI names Core publishes in its snapshot. Rendering an option list is
/// presentation; Core remains the only authority on what a selection does.
export const WORK_MODES = ["plan", "build", "review", "explore"] as const;
export const PERMISSION_LEVELS = [
  "ask",
  "auto_edit",
  "auto",
  "read_only",
  "full_access",
] as const;

const WORK_MODE_KEYS: Record<string, MessageKey> = {
  plan: "d1.controls.workMode.plan",
  build: "d1.controls.workMode.build",
  review: "d1.controls.workMode.review",
  explore: "d1.controls.workMode.explore",
};

const PERMISSION_KEYS: Record<string, MessageKey> = {
  ask: "d1.controls.permission.ask",
  auto_edit: "d1.controls.permission.auto_edit",
  auto: "d1.controls.permission.auto",
  read_only: "d1.controls.permission.read_only",
  full_access: "d1.controls.permission.full_access",
};

/// Localizes a Core CLI name; an unknown name stays visible as itself rather
/// than being hidden behind a wrong label.
export function workModeLabel(locale: Locale, mode: string): string {
  const key = WORK_MODE_KEYS[mode];
  return key ? translate(locale, key, {}) : mode;
}

export function permissionLabel(locale: Locale, level: string): string {
  const key = PERMISSION_KEYS[level];
  return key ? translate(locale, key, {}) : level;
}

/// Model options are only what Core published: the active provider's current
/// model plus every adapter model list. Nothing is fabricated, so an empty
/// result simply leaves the selector with no options.
export function modelGroups(
  projection: Pick<D1CockpitProjection, "environment" | "contextDock" | "agentAdapters">,
): ModelGroup[] {
  const groups: ModelGroup[] = [];
  const provider = projection.contextDock.provider;
  const providerId = provider?.providerId ?? projection.environment.providerId;
  const model = provider?.model ?? projection.environment.model;
  if (providerId && providerId !== "—" && model && model !== "—") {
    groups.push({ providerId, label: providerId, models: [model] });
  }
  for (const adapter of projection.agentAdapters) {
    const models = adapter.models ?? [];
    if (models.length === 0) continue;
    groups.push({
      providerId: adapter.agentId,
      label: adapter.displayName,
      models: [...models],
    });
  }
  return groups;
}

export interface ComposerControlsModel {
  locale: Locale;
  workMode: string;
  permissionLevel: string;
  providerId: string;
  model: string;
  groups: ModelGroup[];
  /** False while the composer is not editable or no workspace is open. */
  enabled: boolean;
  /** True while a control command is in flight or another command holds the slot. */
  busy: boolean;
  open: ComposerControlKind | null;
}

export interface ComposerControlsHandle {
  element: HTMLElement;
  /** Removes the outside-click listener. Always call before dropping the element. */
  dispose: () => void;
}

interface OptionSpec {
  key: string;
  label: string;
  detail: string | null;
  selected: boolean;
  intent: ComposerControlIntent;
}

export function renderComposerControls(
  model: ComposerControlsModel,
  onToggle: (control: ComposerControlKind | null, restoreFocus: boolean) => void,
  onControl: (intent: ComposerControlIntent) => void,
): ComposerControlsHandle {
  const row = document.createElement("div");
  row.className = "d1-cmeta";
  row.dataset.composerMeta = "true";
  row.setAttribute("aria-busy", String(model.busy));

  const disabled = !model.enabled || model.busy;

  const optionButtons = (popover: HTMLElement): HTMLButtonElement[] =>
    Array.from(popover.querySelectorAll<HTMLButtonElement>("[data-control-option]"));

  const focusAt = (popover: HTMLElement, index: number): void => {
    const options = optionButtons(popover);
    if (options.length === 0) return;
    const target = options[((index % options.length) + options.length) % options.length]!;
    options.forEach((candidate) => (candidate.tabIndex = -1));
    target.tabIndex = 0;
    target.focus();
  };

  const popoverFor = (kind: ComposerControlKind, label: string, options: OptionSpec[]): HTMLElement => {
    const popover = document.createElement("div");
    popover.className = "d1-control-popover";
    popover.dataset.controlPopover = kind;
    popover.setAttribute("role", "listbox");
    popover.setAttribute("aria-label", label);
    // Focusable so an option-less popover can still receive Escape.
    popover.tabIndex = -1;
    for (const option of options) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "d1-control-option";
      button.setAttribute("role", "option");
      button.setAttribute("aria-selected", String(option.selected));
      button.dataset.controlOption = "true";
      button.dataset.controlOptionKey = option.key;
      button.tabIndex = option.selected ? 0 : -1;
      const name = document.createElement("span");
      name.className = "d1-control-option-name";
      name.textContent = option.label;
      button.append(name);
      if (option.detail) {
        const detail = document.createElement("small");
        detail.className = "d1-control-option-detail";
        detail.textContent = option.detail;
        button.append(detail);
      }
      button.addEventListener("click", () => onControl(option.intent));
      popover.append(button);
    }
    if (options.length === 0) {
      const empty = document.createElement("p");
      empty.className = "d1-control-empty";
      empty.dataset.controlEmpty = "true";
      empty.textContent = translate(model.locale, "d1.controls.noOptions", {});
      popover.append(empty);
    }
    popover.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        onToggle(null, true);
        return;
      }
      const target = event.target as HTMLElement | null;
      if (!(target instanceof HTMLButtonElement) || !target.dataset.controlOption) return;
      const current = optionButtons(popover).indexOf(target);
      if (event.key === "ArrowDown") {
        event.preventDefault();
        focusAt(popover, current + 1);
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        focusAt(popover, current - 1);
      } else if (event.key === "Home") {
        event.preventDefault();
        focusAt(popover, 0);
      } else if (event.key === "End") {
        event.preventDefault();
        focusAt(popover, optionButtons(popover).length - 1);
      }
    });
    return popover;
  };

  const pill = (
    kind: ComposerControlKind,
    label: string,
    text: string,
    options: OptionSpec[],
  ): HTMLElement => {
    const wrap = document.createElement("div");
    wrap.className = "d1-control";
    wrap.dataset.control = kind;
    const toggle = document.createElement("button");
    toggle.type = "button";
    toggle.className = "d1-mpill";
    toggle.dataset.controlToggle = kind;
    toggle.disabled = disabled;
    toggle.setAttribute("aria-haspopup", "listbox");
    toggle.setAttribute("aria-expanded", String(model.open === kind));
    toggle.setAttribute("aria-label", label);
    toggle.textContent = `${text} ▾`;
    toggle.addEventListener("click", () => {
      onToggle(model.open === kind ? null : kind, model.open === kind);
    });
    wrap.append(toggle);
    if (model.open === kind) {
      wrap.append(popoverFor(kind, label, options));
    }
    row.append(wrap);
    return wrap;
  };

  pill(
    "work_mode",
    translate(model.locale, "d1.controls.workMode", {}),
    `⛭ ${workModeLabel(model.locale, model.workMode)}`,
    WORK_MODES.map((mode) => ({
      key: `work_mode:${mode}`,
      label: workModeLabel(model.locale, mode),
      detail: null,
      selected: mode === model.workMode,
      intent: { type: "set_work_mode", mode },
    })),
  );

  pill(
    "permission",
    translate(model.locale, "d1.controls.permission", {}),
    `⛨ PERM ${permissionLabel(model.locale, model.permissionLevel)}`,
    PERMISSION_LEVELS.map((level) => ({
      key: `permission:${level}`,
      label: permissionLabel(model.locale, level),
      detail: null,
      selected: level === model.permissionLevel,
      intent: { type: "set_permission_level", level },
    })),
  );

  pill(
    "model",
    translate(model.locale, "d1.controls.model", {}),
    `${model.providerId} · ${model.model}`,
    model.groups.flatMap((group) =>
      group.models.map((candidate) => ({
        key: `model:${group.providerId}:${candidate}`,
        label: candidate,
        detail: group.label,
        selected: group.providerId === model.providerId && candidate === model.model,
        intent: {
          type: "select_model" as const,
          providerId: group.providerId,
          model: candidate,
        },
      })),
    ),
  );

  const outside = (event: MouseEvent): void => {
    if (model.open === null) return;
    if (!row.contains(event.target as Node)) onToggle(null, false);
  };
  document.addEventListener("mousedown", outside);

  return {
    element: row,
    dispose: () => document.removeEventListener("mousedown", outside),
  };
}
