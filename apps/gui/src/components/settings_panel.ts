import { translate, type Locale, type MessageKey } from "../i18n/catalog";
import type {
  PreferenceDraft,
  PreferenceIntentOutcome,
  PreferenceState,
} from "../preferences";
import { DENSITIES, MOTIONS, SKINS } from "../ui/theme";
import "./settings_panel.css";

/**
 * The Settings overlay: language and appearance, as a client of the Core
 * preference contract.
 *
 * Every control edits a GUI-local draft only. Nothing here writes a
 * preference, re-resolves precedence, or validates the skin/mode pair — Core
 * owns all three, so an invalid pair stays selectable and comes back as Core's
 * own rejection rather than as a client rule the operator cannot see. Rendered
 * authority changes only when a Core result is confirmed.
 *
 * Visual vocabulary: the registered design component
 * `docs/viden-design/Viden/GUI/gui-settings.jsx`. Popover behaviour: the
 * agent-menu conventions (Escape closes and returns focus, an outside click
 * closes, arrow keys move within a group).
 */

export type SettingsField = "locale" | "skin" | "mode" | "density" | "motion";

/** Requestable locales. `system` is a Core value Core resolves per host. */
export const LOCALE_OPTIONS = ["system", "en", "zh-CN"] as const;
/** Requestable modes. Core resolves `system` into dark or light. */
export const MODE_OPTIONS = ["system", "dark", "light"] as const;
/** Skins Core only accepts in dark mode; shown as guidance, never enforced here. */
const DARK_ONLY_SKINS = new Set(["amber", "phosphor"]);

const OPTION_KEYS: Record<string, MessageKey> = {
  "locale:system": "settings.locale.system",
  "locale:en": "settings.locale.en",
  "locale:zh-CN": "settings.locale.zhCN",
  "skin:aurora": "settings.skin.aurora",
  "skin:ice": "settings.skin.ice",
  "skin:mono": "settings.skin.mono",
  "skin:amber": "settings.skin.amber",
  "skin:phosphor": "settings.skin.phosphor",
  "mode:system": "settings.mode.system",
  "mode:dark": "settings.mode.dark",
  "mode:light": "settings.mode.light",
  "density:compact": "settings.density.compact",
  "density:regular": "settings.density.regular",
  "density:comfy": "settings.density.comfy",
  "motion:system": "settings.motion.system",
  "motion:reduced": "settings.motion.reduced",
  "motion:full": "settings.motion.full",
};

/** Localizes one option; an unknown value stays visible as itself. */
export function settingsOptionLabel(
  locale: Locale,
  field: SettingsField,
  value: string,
): string {
  const key = OPTION_KEYS[`${field}:${value}`];
  return key ? translate(locale, key, {}) : value;
}

/**
 * The value a field shows: the operator's unsaved choice when there is one,
 * otherwise the value Core resolved. A draft is never mistaken for authority —
 * it is only what the pending Save would ask for.
 */
export function settingsFieldValue(
  state: PreferenceState,
  field: SettingsField,
): string {
  return state.draft?.[field] ?? state.resolved[field];
}

export interface SettingsPanelModel {
  /** UI language for the panel's own copy. */
  locale: Locale;
  state: PreferenceState;
  /** False when Core's handshake has no `ui.preference_persistence`. */
  available: boolean;
  /** True while a preference command is in flight. */
  saving: boolean;
  /** The last Core outcome, or null before any command in this session. */
  outcome: PreferenceIntentOutcome | null;
}

export interface SettingsPanelHandlers {
  onDraft: (update: PreferenceDraft) => void;
  onSave: () => void;
  onCancel: () => void;
  onRestore: () => void;
  onClose: () => void;
}

export interface SettingsPanelController {
  root: HTMLElement;
  close: () => void;
}

interface FieldSpec {
  field: SettingsField;
  titleKey: MessageKey;
  detailKey: MessageKey | null;
  options: readonly string[];
  /** Skins render as the design's chip row rather than a segmented control. */
  chips?: boolean;
}

const APPEARANCE_FIELDS: FieldSpec[] = [
  {
    field: "skin",
    titleKey: "settings.skin",
    detailKey: "settings.skin.detail",
    options: SKINS,
    chips: true,
  },
  {
    field: "mode",
    titleKey: "settings.mode",
    detailKey: "settings.mode.detail",
    options: MODE_OPTIONS,
  },
  { field: "density", titleKey: "settings.density", detailKey: null, options: DENSITIES },
  {
    field: "motion",
    titleKey: "settings.motion",
    detailKey: "settings.motion.detail",
    options: MOTIONS,
  },
];

export function renderSettingsPanel(
  anchor: HTMLButtonElement,
  model: SettingsPanelModel,
  handlers: SettingsPanelHandlers,
): SettingsPanelController {
  const { locale } = model;
  const disabled = !model.available || model.saving;

  const panel = document.createElement("div");
  panel.className = "gset-panel";
  panel.dataset.settingsPanel = "true";
  panel.dataset.settingsAvailable = String(model.available);
  panel.setAttribute("role", "dialog");
  panel.setAttribute("aria-modal", "false");
  panel.setAttribute("aria-label", translate(locale, "settings.title", {}));
  panel.setAttribute("aria-busy", String(model.saving));
  panel.tabIndex = -1;

  const header = document.createElement("header");
  header.className = "gset-header";
  const heading = document.createElement("h2");
  heading.className = "gset-heading";
  heading.textContent = translate(locale, "settings.title", {});
  const source = document.createElement("span");
  source.className = "gset-source";
  source.dataset.settingsSource = "true";
  source.textContent = translate(locale, "settings.source", {});
  const close = document.createElement("button");
  close.type = "button";
  close.className = "gset-close";
  close.dataset.settingsClose = "true";
  close.setAttribute("aria-label", translate(locale, "settings.close", {}));
  close.textContent = "×";
  close.addEventListener("click", () => controller.close());
  header.append(heading, source, close);
  panel.append(header);

  if (!model.available) {
    // An absent capability is stated, not hidden: the controls below stay
    // visible and read-only so the operator can see exactly what Core would
    // own once it publishes `ui.preference_persistence`.
    const notice = document.createElement("p");
    notice.className = "gset-unavailable";
    notice.dataset.settingsUnavailable = "true";
    notice.setAttribute("role", "status");
    notice.textContent = translate(locale, "preferences.unavailable", {
      capability: "ui.preference_persistence",
    });
    panel.append(notice);
  }

  const optionGroups: HTMLElement[] = [];

  const renderField = (card: HTMLElement, spec: FieldSpec): void => {
    const row = document.createElement("div");
    row.className = "gset-row";
    const labels = document.createElement("div");
    labels.className = "gset-row-labels";
    const title = document.createElement("div");
    title.className = "gset-row-title";
    title.id = `gset-label-${spec.field}`;
    title.textContent = translate(locale, spec.titleKey, {});
    labels.append(title);
    if (spec.detailKey) {
      const detail = document.createElement("div");
      detail.className = "gset-row-detail";
      detail.textContent = translate(locale, spec.detailKey, {});
      labels.append(detail);
    }

    const group = document.createElement("div");
    group.className = spec.chips ? "gset-seg gset-skins" : "gset-seg";
    group.dataset.settingsField = spec.field;
    group.setAttribute("role", "radiogroup");
    group.setAttribute("aria-labelledby", title.id);
    const current = settingsFieldValue(model.state, spec.field);
    for (const value of spec.options) {
      const option = document.createElement("button");
      option.type = "button";
      option.className = "gset-option";
      option.dataset.settingsOption = `${spec.field}:${value}`;
      option.setAttribute("role", "radio");
      const selected = value === current;
      option.setAttribute("aria-checked", String(selected));
      option.tabIndex = selected ? 0 : -1;
      option.disabled = disabled;
      if (spec.chips) {
        const dot = document.createElement("span");
        dot.className = "gset-skin-dot";
        dot.setAttribute("aria-hidden", "true");
        option.append(dot);
      }
      const label = document.createElement("span");
      label.textContent = settingsOptionLabel(locale, spec.field, value);
      option.append(label);
      if (spec.chips && DARK_ONLY_SKINS.has(value)) {
        // Guidance only. Core is the authority on the pair, so the option
        // stays selectable and an invalid pair returns Core's own rejection.
        const note = document.createElement("small");
        note.className = "gset-skin-note";
        note.textContent = translate(locale, "settings.skin.darkOnly", {});
        option.append(note);
      }
      option.addEventListener("click", () => {
        if (disabled) return;
        handlers.onDraft({ [spec.field]: value } as PreferenceDraft);
      });
      group.append(option);
    }
    optionGroups.push(group);
    row.append(labels, group);
    card.append(row);
  };

  const card = (headingKey: MessageKey, specs: FieldSpec[]): void => {
    const element = document.createElement("section");
    element.className = "gset-card";
    const head = document.createElement("div");
    head.className = "gset-card-head";
    head.textContent = translate(locale, headingKey, {});
    element.append(head);
    for (const spec of specs) renderField(element, spec);
    panel.append(element);
  };

  card("settings.language", [
    {
      field: "locale",
      titleKey: "settings.locale",
      detailKey: "settings.locale.detail",
      options: LOCALE_OPTIONS,
    },
  ]);
  card("settings.appearance", APPEARANCE_FIELDS);

  // Diagnostics are Core's message about what it had to correct. They are
  // rendered verbatim by code rather than reworded into a client guess.
  const diagnostics = [
    ...(model.outcome?.status === "confirmed" || model.outcome?.status === "rejected"
      ? model.outcome.diagnostics
      : []),
    ...model.state.resolved.diagnostics,
  ];
  if (diagnostics.length > 0) {
    const list = document.createElement("ul");
    list.className = "gset-diagnostics";
    list.dataset.settingsDiagnostics = "true";
    for (const diagnostic of diagnostics) {
      const item = document.createElement("li");
      item.className = "gset-diagnostic";
      item.dataset.settingsDiagnostic = diagnostic.code;
      item.textContent = diagnostic.rejectedValue
        ? `${diagnostic.code} · ${diagnostic.field ?? ""} ${diagnostic.rejectedValue}`.trim()
        : diagnostic.code;
      list.append(item);
    }
    panel.append(list);
  }

  if (model.outcome?.status === "rejected") {
    const alert = document.createElement("p");
    alert.className = "gset-alert";
    alert.dataset.settingsAlert = "true";
    alert.setAttribute("role", "alert");
    alert.textContent = model.outcome.reason;
    panel.append(alert);
  } else if (model.outcome?.status === "confirmed") {
    const status = document.createElement("p");
    status.className = "gset-status";
    status.dataset.settingsStatus = model.outcome.persisted ? "saved" : "restored";
    status.setAttribute("role", "status");
    status.textContent = translate(
      locale,
      model.outcome.persisted ? "settings.saved" : "settings.restored",
      {},
    );
    panel.append(status);
  }

  const actions = document.createElement("div");
  actions.className = "gset-actions";
  const restore = document.createElement("button");
  restore.type = "button";
  restore.className = "gset-restore";
  restore.dataset.settingsRestore = "true";
  restore.textContent = translate(locale, "settings.restore", {});
  restore.disabled = disabled;
  restore.addEventListener("click", () => handlers.onRestore());
  const cancel = document.createElement("button");
  cancel.type = "button";
  cancel.className = "gset-cancel";
  cancel.dataset.settingsCancel = "true";
  cancel.textContent = translate(locale, "settings.cancel", {});
  cancel.disabled = model.saving || !model.state.dirty;
  cancel.addEventListener("click", () => handlers.onCancel());
  const save = document.createElement("button");
  save.type = "button";
  save.className = "gset-save";
  save.dataset.settingsSave = "true";
  save.textContent = translate(
    locale,
    model.saving ? "settings.saving" : "settings.save",
    {},
  );
  // Nothing to persist until the operator actually changed an axis.
  save.disabled = disabled || !model.state.dirty;
  save.addEventListener("click", () => handlers.onSave());
  actions.append(restore, cancel, save);
  panel.append(actions);

  const anchorRect = anchor.getBoundingClientRect();
  panel.style.setProperty("--gset-anchor-inline", `${anchorRect.right}px`);
  panel.style.setProperty("--gset-anchor-block", `${anchorRect.bottom}px`);
  anchor.setAttribute("aria-expanded", "true");
  // Portalled out of the rail so the overlay is not clipped by the
  // auto-hiding sidebar, matching the New Lane popover.
  (anchor.closest(".d1-frame")?.parentElement ?? document.body).append(panel);

  const focusAt = (group: HTMLElement, index: number): void => {
    const options = Array.from(
      group.querySelectorAll<HTMLButtonElement>("[data-settings-option]"),
    ).filter((option) => !option.disabled);
    if (options.length === 0) return;
    const target = options[((index % options.length) + options.length) % options.length]!;
    options.forEach((option) => (option.tabIndex = -1));
    target.tabIndex = 0;
    target.focus();
  };

  panel.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      controller.close();
      return;
    }
    const target = event.target as HTMLElement | null;
    if (!(target instanceof HTMLButtonElement) || !target.dataset.settingsOption) return;
    const group = optionGroups.find((candidate) => candidate.contains(target));
    if (!group) return;
    const options = Array.from(
      group.querySelectorAll<HTMLButtonElement>("[data-settings-option]"),
    ).filter((option) => !option.disabled);
    const current = options.indexOf(target);
    if (event.key === "ArrowRight" || event.key === "ArrowDown") {
      event.preventDefault();
      focusAt(group, current + 1);
    } else if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
      event.preventDefault();
      focusAt(group, current - 1);
    } else if (event.key === "Home") {
      event.preventDefault();
      focusAt(group, 0);
    } else if (event.key === "End") {
      event.preventDefault();
      focusAt(group, options.length - 1);
    }
  });

  // The cockpit rebuilds the rail on every Core refresh, so the gear that is
  // mounted now may not be the node this panel was anchored to. Both the
  // outside-click guard and the focus hand-back resolve the live gear instead
  // of a detached one — otherwise the gear could no longer close its own
  // panel, and Escape would drop focus to the document body.
  const liveAnchor = (): HTMLButtonElement =>
    anchor.isConnected
      ? anchor
      : (document.querySelector<HTMLButtonElement>("[data-settings-toggle]") ?? anchor);

  let closed = false;
  const outside = (event: MouseEvent): void => {
    const target = event.target;
    if (panel.contains(target as Node)) return;
    if (target instanceof Element && target.closest("[data-settings-toggle]")) return;
    controller.close();
  };
  const controller: SettingsPanelController = {
    root: panel,
    close: () => {
      if (closed) return;
      closed = true;
      document.removeEventListener("mousedown", outside);
      panel.remove();
      const gear = liveAnchor();
      gear.setAttribute("aria-expanded", "false");
      gear.focus();
      handlers.onClose();
    },
  };
  document.addEventListener("mousedown", outside);
  return controller;
}
