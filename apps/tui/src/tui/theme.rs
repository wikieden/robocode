use crossterm::style::Color;
use viden_core::{ResolvedUiPreferences, TuiColorDepth};

use super::{
    palette::Palette,
    preferences::{ColorDepth, ColorMode, Skin, TerminalCapabilities, resolve_appearance},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TuiTheme {
    pub(super) name: &'static str,
    pub(super) background: Color,
    pub(super) surface: Color,
    pub(super) overlay: Color,
    pub(super) chip: Color,
    pub(super) text: Color,
    pub(super) frame: Color,
    pub(super) title: Color,
    pub(super) accent: Color,
    pub(super) success: Color,
    pub(super) warning: Color,
    pub(super) error: Color,
    pub(super) muted: Color,
    palette: &'static Palette,
    depth: ColorDepth,
}

impl TuiTheme {
    pub(super) fn aurora() -> Self {
        Self::from_key(Skin::Aurora, ColorMode::Dark, ColorDepth::Truecolor)
    }

    #[cfg(test)]
    pub(super) fn aurora_cyan() -> Self {
        Self::aurora()
    }

    pub(super) fn ice() -> Self {
        Self::from_key(Skin::Ice, ColorMode::Dark, ColorDepth::Truecolor)
    }

    pub(super) fn mono() -> Self {
        Self::from_key(Skin::Mono, ColorMode::Dark, ColorDepth::Truecolor)
    }

    pub(super) fn amber() -> Self {
        Self::from_key(Skin::Amber, ColorMode::Dark, ColorDepth::Truecolor)
    }

    pub(super) fn phosphor() -> Self {
        Self::from_key(Skin::Phosphor, ColorMode::Dark, ColorDepth::Truecolor)
    }

    fn from_key(skin: Skin, mode: ColorMode, depth: ColorDepth) -> Self {
        let palette = Palette::find(skin, mode).unwrap_or_else(|| {
            Palette::find(Skin::Aurora, ColorMode::Dark)
                .expect("generated palettes include the safe fallback")
        });
        Self::from_palette(palette, depth)
    }

    pub(super) fn from_palette(palette: &'static Palette, depth: ColorDepth) -> Self {
        let colors = palette.for_depth(depth);
        Self {
            name: profile_name(palette.skin, palette.mode),
            background: colors.background,
            surface: colors.surface,
            overlay: colors.overlay,
            chip: colors.chip,
            text: colors.text,
            frame: colors.frame,
            title: colors.title,
            accent: colors.accent,
            success: colors.success,
            warning: colors.warning,
            error: colors.error,
            muted: colors.muted,
            palette,
            depth,
        }
    }

    pub(super) fn builtins() -> [Self; 8] {
        std::array::from_fn(|index| {
            Self::from_palette(&Palette::all()[index], ColorDepth::Truecolor)
        })
    }

    pub(super) fn names() -> &'static [&'static str] {
        &[
            "aurora",
            "aurora-light",
            "ice",
            "ice-light",
            "mono",
            "mono-light",
            "amber",
            "phosphor",
        ]
    }

    pub(super) fn named(name: &str) -> Self {
        parse_profile(name)
            .map(|(skin, mode)| Self::from_key(skin, mode, ColorDepth::Truecolor))
            .unwrap_or_else(Self::aurora)
    }

    pub(super) fn is_known(name: &str) -> bool {
        parse_profile(name).is_some()
    }

    pub(super) fn from_name_or_env(name: Option<&str>) -> Self {
        name.map(Self::named).unwrap_or_else(Self::from_env)
    }

    pub(super) fn from_preferences(preferences: &ResolvedUiPreferences) -> Self {
        let appearance = resolve_appearance(
            preferences,
            ColorDepth::Truecolor,
            TerminalCapabilities::default(),
        );
        Self::from_palette(appearance.palette, appearance.color_depth)
    }

    pub(super) fn with_color_depth(self, depth: TuiColorDepth) -> Self {
        Self::from_palette(self.palette, ColorDepth::from(depth))
    }

    pub(super) fn from_env() -> Self {
        std::env::var("VIDEN_TUI_THEME")
            .ok()
            .map(|name| Self::named(&name))
            .unwrap_or_else(Self::aurora)
    }

    pub(super) fn next(&self) -> Self {
        let themes = Self::builtins();
        let index = themes
            .iter()
            .position(|theme| theme.name == self.name)
            .unwrap_or(0);
        themes[(index + 1) % themes.len()].clone()
    }

    pub(super) fn depth(&self) -> ColorDepth {
        self.depth
    }
}

fn profile_name(skin: Skin, mode: ColorMode) -> &'static str {
    match (skin, mode) {
        (Skin::Aurora, ColorMode::Dark) => "aurora",
        (Skin::Aurora, ColorMode::Light) => "aurora-light",
        (Skin::Ice, ColorMode::Dark) => "ice",
        (Skin::Ice, ColorMode::Light) => "ice-light",
        (Skin::Mono, ColorMode::Dark) => "mono",
        (Skin::Mono, ColorMode::Light) => "mono-light",
        (Skin::Amber, ColorMode::Dark) => "amber",
        (Skin::Phosphor, ColorMode::Dark) => "phosphor",
        _ => "aurora",
    }
}

fn parse_profile(name: &str) -> Option<(Skin, ColorMode)> {
    let normalized = name.trim().to_ascii_lowercase();
    let normalized = normalized.replace([':', '/'], "-");
    match normalized.as_str() {
        "aurora" | "aurora-dark" | "aurora-cyan" => Some((Skin::Aurora, ColorMode::Dark)),
        "aurora-light" => Some((Skin::Aurora, ColorMode::Light)),
        "ice" | "ice-dark" | "monochrome-ice" => Some((Skin::Ice, ColorMode::Dark)),
        "ice-light" => Some((Skin::Ice, ColorMode::Light)),
        "mono" | "mono-dark" => Some((Skin::Mono, ColorMode::Dark)),
        "mono-light" => Some((Skin::Mono, ColorMode::Light)),
        "amber" | "amber-dark" | "ember-gold" => Some((Skin::Amber, ColorMode::Dark)),
        "phosphor" | "phosphor-dark" | "plasma-violet" => Some((Skin::Phosphor, ColorMode::Dark)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_themes_cover_the_eight_registered_profiles() {
        let names: Vec<_> = TuiTheme::builtins()
            .into_iter()
            .map(|theme| theme.name)
            .collect();

        assert_eq!(
            names,
            vec![
                "aurora",
                "aurora-light",
                "ice",
                "ice-light",
                "mono",
                "mono-light",
                "amber",
                "phosphor"
            ]
        );
        assert_eq!(TuiTheme::named("unknown").name, "aurora");
        assert_eq!(TuiTheme::named("aurora").next().name, "aurora-light");
        assert!(!TuiTheme::is_known("amber-light"));
        assert!(!TuiTheme::is_known("phosphor-light"));
    }

    #[test]
    fn core_color_mode_changes_the_effective_palette() {
        let dark = TuiTheme::from_preferences(&viden_core::ResolvedUiPreferences {
            skin: Skin::Ice,
            mode: ColorMode::Dark,
            ..viden_core::ResolvedUiPreferences::default()
        });
        let light = TuiTheme::from_preferences(&viden_core::ResolvedUiPreferences {
            skin: Skin::Ice,
            mode: ColorMode::Light,
            ..viden_core::ResolvedUiPreferences::default()
        });

        assert_ne!(dark.background, light.background);
        assert_eq!(dark.name, "ice");
        assert_eq!(light.name, "ice-light");
    }

    #[test]
    fn core_color_depth_selects_a_non_rgb_terminal_palette() {
        let theme = TuiTheme::aurora().with_color_depth(viden_core::TuiColorDepth::Ansi16);

        assert_eq!(theme.depth(), ColorDepth::Ansi16);
        assert!(!matches!(theme.background, Color::Rgb { .. }));
        assert!(!matches!(theme.accent, Color::Rgb { .. }));
    }
}
