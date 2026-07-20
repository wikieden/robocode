pub(super) use viden_core::{
    LocaleId, UiColorMode as ColorMode, UiDensity as Density, UiMotion as Motion, UiSkin as Skin,
};
use viden_core::{
    ResolvedUiPreferences, RuntimeCommand, RuntimeEvent, RuntimeEventKind, TuiColorDepth,
    UiPreferencePatch, UiPreferences,
};

use super::{glyphs::GlyphSet, palette::Palette};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ColorDepth {
    Auto,
    Truecolor,
    Ansi256,
    Ansi16,
}

pub(super) const UI_PREFERENCE_PERSISTENCE_CAPABILITY: &str = "ui.preference_persistence";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PreferenceField {
    Locale,
    Skin,
    Mode,
    Density,
    Motion,
    ColorDepth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PreferenceValue {
    Locale(LocaleId),
    Skin(Skin),
    Mode(ColorMode),
    Density(Density),
    Motion(Motion),
    ColorDepth(ColorDepth),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PreferenceChoice {
    pub(super) value: PreferenceValue,
    pub(super) label_key: &'static str,
    pub(super) effect_key: &'static str,
    pub(super) enabled: bool,
    pub(super) invalid_reason_key: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingPreferenceCommand {
    command_id: String,
    command: RuntimeCommand,
    accepted: bool,
}

/// Selector draft for stable Settings. Core-resolved values remain the
/// baseline; only explicitly selected axes enter the typed patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SettingsPanel {
    pub(super) field: Option<PreferenceField>,
    pub(super) selected: usize,
    baseline: ResolvedUiPreferences,
    patch: UiPreferencePatch,
    color_depth: ColorDepth,
    pending: Option<PendingPreferenceCommand>,
    succeeded: bool,
    rejection_reason: Option<String>,
    diagnostics: Vec<String>,
}

impl SettingsPanel {
    pub(super) fn new(baseline: &ResolvedUiPreferences, color_depth: ColorDepth) -> Self {
        Self {
            field: None,
            selected: 0,
            baseline: baseline.clone(),
            patch: UiPreferencePatch::default(),
            color_depth,
            pending: None,
            succeeded: false,
            rejection_reason: None,
            diagnostics: baseline
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.clone())
                .collect(),
        }
    }

    pub(super) fn choices(&self, field: PreferenceField) -> Vec<PreferenceChoice> {
        let selected_skin = self.selected_skin();
        let selected_mode = self.selected_mode();
        match field {
            PreferenceField::Locale => vec![
                choice(
                    PreferenceValue::Locale(LocaleId::System),
                    "settings.value.system",
                    "settings.effect.locale.system",
                    true,
                ),
                choice(
                    PreferenceValue::Locale(LocaleId::En),
                    "settings.value.en",
                    "settings.effect.locale.en",
                    true,
                ),
                choice(
                    PreferenceValue::Locale(LocaleId::ZhCn),
                    "settings.value.zh_cn",
                    "settings.effect.locale.zh_cn",
                    true,
                ),
            ],
            PreferenceField::Skin => Skin::ALL
                .into_iter()
                .map(|skin| {
                    let enabled = selected_mode != ColorMode::Light
                        || !matches!(skin, Skin::Amber | Skin::Phosphor);
                    choice(
                        PreferenceValue::Skin(skin),
                        skin_label_key(skin),
                        "settings.effect.skin",
                        enabled,
                    )
                })
                .collect(),
            PreferenceField::Mode => [ColorMode::System, ColorMode::Dark, ColorMode::Light]
                .into_iter()
                .map(|mode| {
                    let enabled = mode != ColorMode::Light
                        || !matches!(selected_skin, Skin::Amber | Skin::Phosphor);
                    choice(
                        PreferenceValue::Mode(mode),
                        mode_label_key(mode),
                        mode_effect_key(mode),
                        enabled,
                    )
                })
                .collect(),
            PreferenceField::Density => [Density::Compact, Density::Regular, Density::Comfy]
                .into_iter()
                .map(|density| {
                    choice(
                        PreferenceValue::Density(density),
                        density_label_key(density),
                        density_effect_key(density),
                        true,
                    )
                })
                .collect(),
            PreferenceField::Motion => [Motion::System, Motion::Reduced, Motion::Full]
                .into_iter()
                .map(|motion| {
                    choice(
                        PreferenceValue::Motion(motion),
                        motion_label_key(motion),
                        motion_effect_key(motion),
                        true,
                    )
                })
                .collect(),
            PreferenceField::ColorDepth => [
                ColorDepth::Auto,
                ColorDepth::Truecolor,
                ColorDepth::Ansi256,
                ColorDepth::Ansi16,
            ]
            .into_iter()
            .map(|depth| {
                choice(
                    PreferenceValue::ColorDepth(depth),
                    color_depth_label_key(depth),
                    "settings.effect.color_depth",
                    true,
                )
            })
            .collect(),
        }
    }

    pub(super) fn select(&mut self, value: PreferenceValue) -> bool {
        let field = field_for_value(value);
        let enabled = self
            .choices(field)
            .iter()
            .find(|choice| choice.value == value)
            .is_some_and(|choice| choice.enabled);
        if !enabled || self.pending.is_some() {
            return false;
        }
        match value {
            PreferenceValue::Locale(value) => self.patch.locale = Some(value),
            PreferenceValue::Skin(value) => self.patch.skin = Some(value),
            PreferenceValue::Mode(value) => self.patch.mode = Some(value),
            PreferenceValue::Density(value) => self.patch.density = Some(value),
            PreferenceValue::Motion(value) => self.patch.motion = Some(value),
            PreferenceValue::ColorDepth(value) => self.color_depth = value,
        }
        self.succeeded = false;
        self.rejection_reason = None;
        true
    }

    pub(super) fn apply_command(&self) -> Option<RuntimeCommand> {
        (!patch_is_empty(&self.patch))
            .then_some(RuntimeCommand::SetUiPreferences { patch: self.patch })
    }

    pub(super) fn reset_command(&self) -> RuntimeCommand {
        RuntimeCommand::ResetUiPreferences
    }

    pub(super) fn begin_pending(&mut self, command_id: String, command: RuntimeCommand) {
        self.pending = Some(PendingPreferenceCommand {
            command_id,
            command,
            accepted: false,
        });
        self.succeeded = false;
        self.rejection_reason = None;
    }

    pub(super) fn observe_event(&mut self, event: &RuntimeEvent) {
        let Some(pending) = self.pending.as_mut() else {
            return;
        };
        match &event.kind {
            RuntimeEventKind::CommandAccepted {
                command_id,
                command,
            } if command_id == &pending.command_id && command == &pending.command => {
                pending.accepted = true;
            }
            RuntimeEventKind::CommandRejected { command_id, reason }
                if command_id == &pending.command_id =>
            {
                self.pending = None;
                self.rejection_reason = Some(reason.clone());
            }
            RuntimeEventKind::UiPreferencesUpdated {
                resolved,
                persisted,
                diagnostics,
            } if pending.accepted && update_matches(&pending.command, persisted.as_ref()) => {
                self.baseline = resolved.clone();
                self.patch = UiPreferencePatch::default();
                self.pending = None;
                self.succeeded = true;
                self.rejection_reason = None;
                self.diagnostics = diagnostics
                    .iter()
                    .chain(resolved.diagnostics.iter())
                    .map(|diagnostic| diagnostic.code.clone())
                    .collect();
                self.diagnostics.sort();
                self.diagnostics.dedup();
            }
            _ => {}
        }
    }

    pub(super) fn selected_locale(&self) -> LocaleId {
        self.patch.locale.unwrap_or(self.baseline.locale)
    }

    pub(super) fn selected_skin(&self) -> Skin {
        self.patch.skin.unwrap_or(self.baseline.skin)
    }

    pub(super) fn selected_mode(&self) -> ColorMode {
        self.patch.mode.unwrap_or(self.baseline.mode)
    }

    pub(super) fn selected_density(&self) -> Density {
        self.patch.density.unwrap_or(self.baseline.density)
    }

    pub(super) fn selected_motion(&self) -> Motion {
        self.patch.motion.unwrap_or(self.baseline.motion)
    }

    pub(super) fn color_depth(&self) -> ColorDepth {
        self.color_depth
    }

    pub(super) fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn has_succeeded(&self) -> bool {
        self.succeeded
    }

    pub(super) fn rejection_reason(&self) -> Option<&str> {
        self.rejection_reason.as_deref()
    }

    pub(super) fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }
}

fn choice(
    value: PreferenceValue,
    label_key: &'static str,
    effect_key: &'static str,
    enabled: bool,
) -> PreferenceChoice {
    PreferenceChoice {
        value,
        label_key,
        effect_key,
        enabled,
        invalid_reason_key: (!enabled).then_some("settings.invalid.retro_light"),
    }
}

fn field_for_value(value: PreferenceValue) -> PreferenceField {
    match value {
        PreferenceValue::Locale(_) => PreferenceField::Locale,
        PreferenceValue::Skin(_) => PreferenceField::Skin,
        PreferenceValue::Mode(_) => PreferenceField::Mode,
        PreferenceValue::Density(_) => PreferenceField::Density,
        PreferenceValue::Motion(_) => PreferenceField::Motion,
        PreferenceValue::ColorDepth(_) => PreferenceField::ColorDepth,
    }
}

fn patch_is_empty(patch: &UiPreferencePatch) -> bool {
    patch.locale.is_none()
        && patch.skin.is_none()
        && patch.mode.is_none()
        && patch.density.is_none()
        && patch.motion.is_none()
}

fn update_matches(command: &RuntimeCommand, persisted: Option<&UiPreferences>) -> bool {
    match command {
        RuntimeCommand::SetUiPreferences { patch } => persisted.is_some_and(|persisted| {
            patch.locale.is_none_or(|value| persisted.locale == value)
                && patch.skin.is_none_or(|value| persisted.skin == value)
                && patch.mode.is_none_or(|value| persisted.mode == value)
                && patch.density.is_none_or(|value| persisted.density == value)
                && patch.motion.is_none_or(|value| persisted.motion == value)
        }),
        RuntimeCommand::ResetUiPreferences => persisted.is_none(),
        _ => false,
    }
}

pub(super) const fn skin_label_key(value: Skin) -> &'static str {
    match value {
        Skin::Aurora => "settings.value.aurora",
        Skin::Ice => "settings.value.ice",
        Skin::Mono => "settings.value.mono",
        Skin::Amber => "settings.value.amber",
        Skin::Phosphor => "settings.value.phosphor",
    }
}

pub(super) const fn mode_label_key(value: ColorMode) -> &'static str {
    match value {
        ColorMode::System => "settings.value.system",
        ColorMode::Dark => "settings.value.dark",
        ColorMode::Light => "settings.value.light",
    }
}

pub(super) const fn density_label_key(value: Density) -> &'static str {
    match value {
        Density::Compact => "settings.value.compact",
        Density::Regular => "settings.value.regular",
        Density::Comfy => "settings.value.comfy",
    }
}

pub(super) const fn motion_label_key(value: Motion) -> &'static str {
    match value {
        Motion::System => "settings.value.system",
        Motion::Reduced => "settings.value.reduced",
        Motion::Full => "settings.value.full",
    }
}

pub(super) const fn color_depth_label_key(value: ColorDepth) -> &'static str {
    match value {
        ColorDepth::Auto => "settings.value.auto",
        ColorDepth::Truecolor => "settings.value.truecolor",
        ColorDepth::Ansi256 => "settings.value.ansi256",
        ColorDepth::Ansi16 => "settings.value.ansi16",
    }
}

fn mode_effect_key(value: ColorMode) -> &'static str {
    match value {
        ColorMode::System => "settings.effect.mode.system",
        ColorMode::Dark => "settings.effect.mode.dark",
        ColorMode::Light => "settings.effect.mode.light",
    }
}

fn density_effect_key(value: Density) -> &'static str {
    match value {
        Density::Compact => "settings.effect.density.compact",
        Density::Regular => "settings.effect.density.regular",
        Density::Comfy => "settings.effect.density.comfy",
    }
}

fn motion_effect_key(value: Motion) -> &'static str {
    match value {
        Motion::System => "settings.effect.motion.system",
        Motion::Reduced => "settings.effect.motion.reduced",
        Motion::Full => "settings.effect.motion.full",
    }
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
        ColorDepth, ColorMode, Density, LocaleId, Motion, PreferenceField, PreferenceValue,
        SettingsPanel, Skin, TerminalCapabilities, resolve_appearance, resolve_preferences,
    };
    use viden_core::{
        ResolvedUiPreferences, RuntimeCommand, RuntimeEvent, RuntimeEventKind,
        UiPreferenceDiagnostic, UiPreferences,
    };

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

    #[test]
    fn settings_offer_every_contract_choice_and_disable_retro_light() {
        let resolved = ResolvedUiPreferences {
            skin: Skin::Amber,
            mode: ColorMode::Dark,
            ..ResolvedUiPreferences::default()
        };
        let mut panel = SettingsPanel::new(&resolved, ColorDepth::Auto);

        assert_eq!(panel.choices(PreferenceField::Locale).len(), 3);
        assert_eq!(panel.choices(PreferenceField::Skin).len(), 5);
        assert_eq!(panel.choices(PreferenceField::Mode).len(), 3);
        assert_eq!(panel.choices(PreferenceField::Density).len(), 3);
        assert_eq!(panel.choices(PreferenceField::Motion).len(), 3);
        assert_eq!(panel.choices(PreferenceField::ColorDepth).len(), 4);

        let light = panel
            .choices(PreferenceField::Mode)
            .into_iter()
            .find(|choice| choice.value == PreferenceValue::Mode(ColorMode::Light))
            .expect("light choice");
        assert!(!light.enabled);
        assert_eq!(
            light.invalid_reason_key,
            Some("settings.invalid.retro_light")
        );

        panel.select(PreferenceValue::Skin(Skin::Phosphor));
        let light = panel
            .choices(PreferenceField::Mode)
            .into_iter()
            .find(|choice| choice.value == PreferenceValue::Mode(ColorMode::Light))
            .expect("light choice");
        assert!(!light.enabled);
    }

    #[test]
    fn settings_build_only_typed_dirty_patch_and_keep_color_depth_local() {
        let mut panel = SettingsPanel::new(&ResolvedUiPreferences::default(), ColorDepth::Auto);
        panel.select(PreferenceValue::Locale(LocaleId::System));
        panel.select(PreferenceValue::Density(Density::Comfy));
        panel.select(PreferenceValue::ColorDepth(ColorDepth::Ansi256));

        assert_eq!(panel.color_depth(), ColorDepth::Ansi256);
        assert_eq!(
            panel.apply_command(),
            Some(RuntimeCommand::SetUiPreferences {
                patch: viden_core::UiPreferencePatch {
                    locale: Some(LocaleId::System),
                    density: Some(Density::Comfy),
                    ..viden_core::UiPreferencePatch::default()
                }
            })
        );
    }

    #[test]
    fn command_acceptance_is_not_preference_success_but_matching_update_is() {
        let mut panel = SettingsPanel::new(&ResolvedUiPreferences::default(), ColorDepth::Auto);
        panel.select(PreferenceValue::Locale(LocaleId::ZhCn));
        let command = panel.apply_command().expect("typed patch");
        panel.begin_pending("tui-7".to_string(), command.clone());

        panel.observe_event(&RuntimeEvent {
            sequence: 1,
            timestamp: Some(1),
            kind: RuntimeEventKind::CommandAccepted {
                command_id: "tui-7".to_string(),
                command,
            },
        });
        assert!(panel.is_pending());
        assert!(!panel.has_succeeded());

        let persisted = UiPreferences {
            locale: LocaleId::ZhCn,
            ..UiPreferences::default()
        };
        let resolved = ResolvedUiPreferences {
            locale: LocaleId::ZhCn,
            ..ResolvedUiPreferences::default()
        };
        panel.observe_event(&RuntimeEvent {
            sequence: 2,
            timestamp: Some(2),
            kind: RuntimeEventKind::UiPreferencesUpdated {
                resolved,
                persisted: Some(persisted),
                diagnostics: vec![UiPreferenceDiagnostic::new(
                    "ui.cli_override_active",
                    "cli_override",
                    "ui.locale",
                    None,
                )],
            },
        });

        assert!(!panel.is_pending());
        assert!(panel.has_succeeded());
        assert!(
            panel
                .diagnostics()
                .contains(&"ui.cli_override_active".to_string())
        );
    }

    #[test]
    fn apply_and_reset_wait_for_matching_core_receipts() {
        let resolved = ResolvedUiPreferences {
            locale: LocaleId::ZhCn,
            ..ResolvedUiPreferences::default()
        };
        let persisted = UiPreferences {
            locale: LocaleId::ZhCn,
            ..UiPreferences::default()
        };

        let mut apply = SettingsPanel::new(&ResolvedUiPreferences::default(), ColorDepth::Auto);
        assert!(apply.select(PreferenceValue::Locale(LocaleId::ZhCn)));
        let apply_command = apply.apply_command().expect("typed Apply patch");
        apply.begin_pending("apply-1".to_string(), apply_command.clone());
        apply.observe_event(&RuntimeEvent {
            sequence: 1,
            timestamp: Some(1),
            kind: RuntimeEventKind::CommandAccepted {
                command_id: "apply-1".to_string(),
                command: apply_command,
            },
        });
        assert!(apply.is_pending());
        apply.observe_event(&RuntimeEvent {
            sequence: 2,
            timestamp: Some(2),
            kind: RuntimeEventKind::UiPreferencesUpdated {
                resolved: resolved.clone(),
                persisted: Some(persisted),
                diagnostics: Vec::new(),
            },
        });
        assert!(apply.has_succeeded());

        let mut reset = SettingsPanel::new(&resolved, ColorDepth::Auto);
        let reset_command = reset.reset_command();
        reset.begin_pending("reset-1".to_string(), reset_command.clone());
        reset.observe_event(&RuntimeEvent {
            sequence: 3,
            timestamp: Some(3),
            kind: RuntimeEventKind::CommandAccepted {
                command_id: "reset-1".to_string(),
                command: reset_command,
            },
        });
        reset.observe_event(&RuntimeEvent {
            sequence: 4,
            timestamp: Some(4),
            kind: RuntimeEventKind::UiPreferencesUpdated {
                resolved: ResolvedUiPreferences::default(),
                persisted: Some(persisted),
                diagnostics: Vec::new(),
            },
        });
        assert!(reset.is_pending(), "Reset ignores a non-default receipt");
        reset.observe_event(&RuntimeEvent {
            sequence: 5,
            timestamp: Some(5),
            kind: RuntimeEventKind::UiPreferencesUpdated {
                resolved: ResolvedUiPreferences::default(),
                persisted: None,
                diagnostics: Vec::new(),
            },
        });
        assert!(reset.has_succeeded());
        assert!(!reset.is_pending());
    }

    #[test]
    fn rejection_keeps_the_draft_and_surfaces_the_core_reason() {
        let mut panel = SettingsPanel::new(&ResolvedUiPreferences::default(), ColorDepth::Auto);
        panel.select(PreferenceValue::Skin(Skin::Ice));
        let command = panel.apply_command().expect("typed patch");
        panel.begin_pending("tui-9".to_string(), command);

        panel.observe_event(&RuntimeEvent {
            sequence: 1,
            timestamp: Some(1),
            kind: RuntimeEventKind::CommandRejected {
                command_id: "tui-9".to_string(),
                reason: "policy denied".to_string(),
            },
        });

        assert!(!panel.is_pending());
        assert_eq!(panel.rejection_reason(), Some("policy denied"));
        assert_eq!(panel.selected_skin(), Skin::Ice);
        assert!(panel.apply_command().is_some());
    }
}
