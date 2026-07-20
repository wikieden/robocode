pub(super) use viden_core::{
    LocaleId, UiColorMode as ColorMode, UiDensity as Density, UiMotion as Motion, UiSkin as Skin,
};
use viden_core::{ResolvedUiPreferences, TuiColorDepth};

use super::{glyphs::GlyphSet, palette::Palette};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ColorDepth {
    Auto,
    Truecolor,
    Ansi256,
    Ansi16,
}

impl From<TuiColorDepth> for ColorDepth {
    fn from(value: TuiColorDepth) -> Self {
        match value {
            TuiColorDepth::Truecolor => Self::Truecolor,
            TuiColorDepth::Ansi256 => Self::Ansi256,
            TuiColorDepth::Ansi16 => Self::Ansi16,
        }
    }
}

impl From<ColorDepth> for TuiColorDepth {
    fn from(value: ColorDepth) -> Self {
        match value {
            ColorDepth::Auto | ColorDepth::Truecolor => Self::Truecolor,
            ColorDepth::Ansi256 => Self::Ansi256,
            ColorDepth::Ansi16 => Self::Ansi16,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TerminalCapabilities {
    pub(super) truecolor: bool,
    pub(super) ansi256: bool,
    pub(super) unicode: bool,
    pub(super) reduced_motion: bool,
}

impl TerminalCapabilities {
    pub(super) fn detect() -> Self {
        let term = std::env::var("TERM").unwrap_or_default();
        let color_term = std::env::var("COLORTERM").unwrap_or_default();
        let locale = std::env::var("LC_ALL")
            .or_else(|_| std::env::var("LC_CTYPE"))
            .or_else(|_| std::env::var("LANG"))
            .unwrap_or_default();
        Self::from_environment(&term, &color_term, &locale)
    }

    pub(super) fn from_environment(term: &str, color_term: &str, locale: &str) -> Self {
        let color_term = color_term.to_ascii_lowercase();
        let truecolor = color_term.contains("truecolor") || color_term.contains("24bit");
        let ansi256 = truecolor || term.contains("256color");
        let unicode = !matches!(locale, "C" | "POSIX") && term != "dumb";
        Self {
            truecolor,
            ansi256,
            unicode,
            reduced_motion: term == "dumb",
        }
    }

    fn resolve_depth(self, requested: ColorDepth) -> ColorDepth {
        match requested {
            ColorDepth::Auto if self.truecolor => ColorDepth::Truecolor,
            ColorDepth::Auto if self.ansi256 => ColorDepth::Ansi256,
            ColorDepth::Auto => ColorDepth::Ansi16,
            ColorDepth::Truecolor if self.truecolor => ColorDepth::Truecolor,
            ColorDepth::Truecolor if self.ansi256 => ColorDepth::Ansi256,
            ColorDepth::Truecolor => ColorDepth::Ansi16,
            ColorDepth::Ansi256 if self.ansi256 => ColorDepth::Ansi256,
            ColorDepth::Ansi256 => ColorDepth::Ansi16,
            ColorDepth::Ansi16 => ColorDepth::Ansi16,
        }
    }
}

impl Default for TerminalCapabilities {
    fn default() -> Self {
        Self {
            truecolor: true,
            ansi256: true,
            unicode: true,
            reduced_motion: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AppearanceGeometry {
    pub(super) panel_gap: usize,
    pub(super) right_rail_width: usize,
    pub(super) vertical_padding: usize,
}

impl AppearanceGeometry {
    pub(super) const fn for_density(density: Density) -> Self {
        match density {
            Density::Compact => Self {
                panel_gap: 1,
                right_rail_width: 34,
                vertical_padding: 0,
            },
            Density::Regular => Self {
                panel_gap: 2,
                right_rail_width: 38,
                vertical_padding: 1,
            },
            Density::Comfy => Self {
                panel_gap: 3,
                right_rail_width: 42,
                vertical_padding: 2,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ResolvedAppearance {
    pub(super) skin: Skin,
    pub(super) mode: ColorMode,
    pub(super) density: Density,
    pub(super) motion: Motion,
    pub(super) color_depth: ColorDepth,
    pub(super) capabilities: TerminalCapabilities,
    pub(super) geometry: AppearanceGeometry,
    pub(super) glyphs: GlyphSet,
    pub(super) palette: &'static Palette,
}

impl ResolvedAppearance {
    pub(super) fn reduced_motion(self) -> bool {
        match self.motion {
            Motion::Reduced => true,
            Motion::Full => false,
            Motion::System => self.capabilities.reduced_motion,
        }
    }
}

/// Converts the Core-owned preference fact into terminal-only presentation.
/// Invalid axes fall back as one unit; locale and motion stay independent.
pub(super) fn resolve_appearance(
    resolved: &ResolvedUiPreferences,
    requested_depth: ColorDepth,
    capabilities: TerminalCapabilities,
) -> ResolvedAppearance {
    let invalid = resolved.mode == ColorMode::System
        || !viden_core::UiPreferences::is_valid_effective_pair(resolved.skin, resolved.mode)
        || resolved
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ui.invalid_skin_mode_pair");
    let (skin, mode, density) = if invalid {
        (Skin::Aurora, ColorMode::Dark, Density::Regular)
    } else {
        (resolved.skin, resolved.mode, resolved.density)
    };
    let palette = Palette::find(skin, mode)
        .expect("the generated registry covers every valid effective skin/mode pair");
    ResolvedAppearance {
        skin,
        mode,
        density,
        motion: resolved.motion,
        color_depth: capabilities.resolve_depth(requested_depth),
        capabilities,
        geometry: AppearanceGeometry::for_density(density),
        glyphs: GlyphSet::new(capabilities.unicode),
        palette,
    }
}

/// TUI-owned presentation projection. Core remains the persistence and runtime
/// authority; this type deliberately contains no storage or project source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TuiPreferences {
    pub(super) locale: LocaleId,
}

impl TuiPreferences {
    pub(super) fn from_resolved(preferences: &ResolvedUiPreferences) -> Self {
        resolve_preferences(preferences)
    }
}

impl Default for TuiPreferences {
    fn default() -> Self {
        Self {
            locale: LocaleId::En,
        }
    }
}

/// Projects the Core-resolved preference fact into TUI presentation state.
/// Precedence and persistence remain exclusively owned by Core.
pub(super) fn resolve_preferences(resolved: &ResolvedUiPreferences) -> TuiPreferences {
    TuiPreferences {
        locale: match resolved.locale {
            LocaleId::ZhCn => LocaleId::ZhCn,
            LocaleId::System | LocaleId::En => LocaleId::En,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ColorDepth, ColorMode, Density, LocaleId, Motion, Skin, TerminalCapabilities,
        resolve_appearance, resolve_preferences,
    };
    use viden_core::{ResolvedUiPreferences, UiPreferenceDiagnostic};

    #[test]
    fn locale_projection_uses_core_resolved_fact_and_safe_legacy_fallback() {
        for (locale, expected) in [
            (LocaleId::ZhCn, LocaleId::ZhCn),
            (LocaleId::En, LocaleId::En),
            (LocaleId::System, LocaleId::En),
        ] {
            let resolved = ResolvedUiPreferences {
                locale,
                ..ResolvedUiPreferences::default()
            };
            assert_eq!(resolve_preferences(&resolved).locale, expected);
        }
    }

    #[test]
    fn project_and_stored_preferences_are_not_tui_resolver_inputs() {
        let resolver_type = std::any::type_name_of_val(&resolve_preferences);
        assert!(!resolver_type.contains("Project"));
        assert!(!resolver_type.contains("UiPreferences"));
    }

    #[test]
    fn appearance_uses_the_core_resolved_fact_and_auto_detected_capabilities() {
        let resolved = ResolvedUiPreferences {
            skin: Skin::Ice,
            mode: ColorMode::Light,
            density: Density::Compact,
            motion: Motion::Reduced,
            ..ResolvedUiPreferences::default()
        };
        let capabilities = TerminalCapabilities {
            truecolor: false,
            ansi256: true,
            unicode: true,
            reduced_motion: false,
        };

        let appearance = resolve_appearance(&resolved, ColorDepth::Auto, capabilities);

        assert_eq!(appearance.skin, Skin::Ice);
        assert_eq!(appearance.mode, ColorMode::Light);
        assert_eq!(appearance.density, Density::Compact);
        assert_eq!(appearance.motion, Motion::Reduced);
        assert_eq!(appearance.color_depth, ColorDepth::Ansi256);
        assert_eq!(appearance.geometry.panel_gap, 1);
        assert!(appearance.reduced_motion());
    }

    #[test]
    fn invalid_or_partial_appearance_falls_back_atomically() {
        let invalid = ResolvedUiPreferences {
            skin: Skin::Amber,
            mode: ColorMode::Light,
            density: Density::Comfy,
            motion: Motion::Full,
            diagnostics: vec![UiPreferenceDiagnostic::new(
                "ui.invalid_skin_mode_pair",
                "skin_mode",
                "ui.mode",
                Some("amber/light".to_string()),
            )],
            ..ResolvedUiPreferences::default()
        };

        let appearance = resolve_appearance(
            &invalid,
            ColorDepth::Truecolor,
            TerminalCapabilities::default(),
        );

        assert_eq!(appearance.skin, Skin::Aurora);
        assert_eq!(appearance.mode, ColorMode::Dark);
        assert_eq!(appearance.density, Density::Regular);
        assert_eq!(appearance.motion, Motion::Full);
        assert_eq!(appearance.color_depth, ColorDepth::Truecolor);
    }

    #[test]
    fn explicit_color_depth_is_clamped_to_terminal_capabilities() {
        let capabilities = TerminalCapabilities {
            truecolor: false,
            ansi256: false,
            unicode: false,
            reduced_motion: true,
        };

        let appearance = resolve_appearance(
            &ResolvedUiPreferences::default(),
            ColorDepth::Truecolor,
            capabilities,
        );

        assert_eq!(appearance.color_depth, ColorDepth::Ansi16);
        assert!(appearance.reduced_motion());
        assert!(!appearance.glyphs.unicode);
    }

    #[test]
    fn density_geometry_is_strictly_ordered() {
        let compact = super::AppearanceGeometry::for_density(Density::Compact);
        let regular = super::AppearanceGeometry::for_density(Density::Regular);
        let comfy = super::AppearanceGeometry::for_density(Density::Comfy);

        assert!(compact.panel_gap < regular.panel_gap);
        assert!(regular.panel_gap < comfy.panel_gap);
        assert!(compact.right_rail_width < regular.right_rail_width);
        assert!(regular.right_rail_width < comfy.right_rail_width);
    }

    #[test]
    fn terminal_capabilities_detect_truecolor_ansi256_and_safe_dumb_fallback() {
        let truecolor =
            TerminalCapabilities::from_environment("xterm-256color", "truecolor", "en_US.UTF-8");
        let ansi256 = TerminalCapabilities::from_environment("screen-256color", "", "zh_CN.UTF-8");
        let dumb = TerminalCapabilities::from_environment("dumb", "", "C");

        assert!(truecolor.truecolor && truecolor.ansi256 && truecolor.unicode);
        assert!(!ansi256.truecolor && ansi256.ansi256 && ansi256.unicode);
        assert!(!dumb.truecolor && !dumb.ansi256 && !dumb.unicode);
        assert!(dumb.reduced_motion);
    }
}
