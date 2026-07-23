import { TOKEN_DENSITIES, TOKEN_SKIN_MODE_PAIRS } from "./tokens";

export const SKINS = ["aurora", "ice", "mono", "amber", "phosphor"] as const;
export const MODES = ["dark", "light"] as const;
export const DENSITIES = TOKEN_DENSITIES;
export const MOTIONS = ["system", "reduced", "full"] as const;

export type Skin = (typeof SKINS)[number];
export type EffectiveMode = (typeof MODES)[number];
export type RequestedMode = EffectiveMode | "system";
export type Density = (typeof DENSITIES)[number];
export type Motion = (typeof MOTIONS)[number];

export const VALID_SKIN_MODE_PAIRS = TOKEN_SKIN_MODE_PAIRS;

export interface PreferenceDiagnostic {
  code: string;
  key: string;
  field: string | null;
  rejectedValue: string | null;
}

export interface ThemePreferenceInput {
  skin?: unknown;
  mode?: unknown;
  density?: unknown;
  motion?: unknown;
  diagnostics?: readonly PreferenceDiagnostic[];
}

export interface ResolvedTheme {
  skin: Skin;
  mode: EffectiveMode;
  density: Density;
  motion: Motion;
  diagnostics: PreferenceDiagnostic[];
}

function isMember<T extends string>(values: readonly T[], value: unknown): value is T {
  return typeof value === "string" && values.includes(value as T);
}

function diagnostic(field: string, rejectedValue: unknown): PreferenceDiagnostic {
  return {
    code: "ui.preference_corrupt",
    key: "ui.preference",
    field,
    rejectedValue: String(rejectedValue),
  };
}

export function resolveTheme(
  preference: ThemePreferenceInput,
  systemMode?: unknown,
): ResolvedTheme {
  const diagnostics = [...(preference.diagnostics ?? [])];
  const skin = isMember(SKINS, preference.skin) ? preference.skin : "aurora";
  if (!isMember(SKINS, preference.skin)) {
    diagnostics.push(diagnostic("skin", preference.skin));
  }

  let mode: EffectiveMode;
  if (preference.mode === "system") {
    if (isMember(MODES, systemMode)) {
      mode = systemMode;
    } else {
      mode = "dark";
      if (systemMode !== undefined) {
        diagnostics.push(diagnostic("systemMode", systemMode));
      }
    }
  } else if (isMember(MODES, preference.mode)) {
    mode = preference.mode;
  } else {
    mode = "dark";
    diagnostics.push(diagnostic("mode", preference.mode));
  }

  const density = isMember(DENSITIES, preference.density)
    ? preference.density
    : "regular";
  if (!isMember(DENSITIES, preference.density)) {
    diagnostics.push(diagnostic("density", preference.density));
  }

  const motion = isMember(MOTIONS, preference.motion) ? preference.motion : "system";
  if (!isMember(MOTIONS, preference.motion)) {
    diagnostics.push(diagnostic("motion", preference.motion));
  }

  const pair = `${skin}/${mode}`;
  if (!(VALID_SKIN_MODE_PAIRS as readonly string[]).includes(pair)) {
    diagnostics.push({
      code: "ui.invalid_skin_mode_pair",
      key: "skin_mode",
      field: "mode",
      rejectedValue: pair,
    });
    return { skin: "aurora", mode: "dark", density: "regular", motion, diagnostics };
  }

  return { skin, mode, density, motion, diagnostics };
}

export function applyResolvedTheme(root: HTMLElement, theme: ResolvedTheme): void {
  Object.assign(root.dataset, {
    skin: theme.skin,
    mode: theme.mode,
    density: theme.density,
    motion: theme.motion,
  });
}
