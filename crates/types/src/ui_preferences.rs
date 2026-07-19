#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LocaleId {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "en")]
    En,
    #[serde(rename = "zh-CN")]
    ZhCn,
}

impl LocaleId {
    pub fn from_system_locale(raw: &str) -> Self {
        let normalized = raw.trim().replace('-', "_").to_ascii_lowercase();
        if normalized.starts_with("zh_cn")
            || normalized.starts_with("zh_hans_cn")
            || normalized == "zh"
        {
            Self::ZhCn
        } else if normalized.is_empty() {
            Self::System
        } else {
            Self::En
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiSkin {
    Aurora,
    Ice,
    Mono,
    Amber,
    Phosphor,
}

impl UiSkin {
    pub const ALL: [Self; 5] = [
        Self::Aurora,
        Self::Ice,
        Self::Mono,
        Self::Amber,
        Self::Phosphor,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiColorMode {
    System,
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiDensity {
    Compact,
    Regular,
    Comfy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiMotion {
    System,
    Reduced,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TuiColorDepth {
    Truecolor,
    Ansi256,
    Ansi16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UiPreferences {
    pub locale: LocaleId,
    pub skin: UiSkin,
    pub mode: UiColorMode,
    pub density: UiDensity,
    pub motion: UiMotion,
}

impl UiPreferences {
    pub fn client_default() -> Self {
        Self {
            locale: LocaleId::System,
            skin: UiSkin::Aurora,
            mode: UiColorMode::System,
            density: UiDensity::Regular,
            motion: UiMotion::System,
        }
    }

    pub fn safe_fallback(locale: LocaleId, motion: UiMotion) -> Self {
        Self {
            locale,
            skin: UiSkin::Aurora,
            mode: UiColorMode::Dark,
            density: UiDensity::Regular,
            motion,
        }
    }

    pub fn is_valid_effective_pair(skin: UiSkin, mode: UiColorMode) -> bool {
        matches!(
            (skin, mode),
            (
                UiSkin::Aurora | UiSkin::Ice | UiSkin::Mono,
                UiColorMode::Dark | UiColorMode::Light
            ) | (UiSkin::Amber | UiSkin::Phosphor, UiColorMode::Dark)
        )
    }
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self::client_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UiPreferenceDiagnostic {
    pub code: String,
    pub key: String,
    pub field: Option<String>,
    pub rejected_value: Option<String>,
}

impl UiPreferenceDiagnostic {
    pub fn new(
        code: impl Into<String>,
        key: impl Into<String>,
        field: impl Into<String>,
        rejected_value: Option<String>,
    ) -> Self {
        Self {
            code: code.into(),
            key: key.into(),
            field: Some(field.into()),
            rejected_value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ResolvedUiPreferences {
    pub locale: LocaleId,
    pub skin: UiSkin,
    pub mode: UiColorMode,
    pub density: UiDensity,
    pub motion: UiMotion,
    pub diagnostics: Vec<UiPreferenceDiagnostic>,
}

pub fn resolve_ui_preferences(
    cli: Option<UiPreferences>,
    user: Option<UiPreferences>,
    project: Option<UiPreferences>,
    client: UiPreferences,
) -> ResolvedUiPreferences {
    let requested = cli.or(user).or(project).unwrap_or(client);
    let locale = resolve_locale(requested.locale, client.locale);
    let mode = resolve_mode(requested.mode, client.mode);
    let motion = requested.motion;

    if !UiPreferences::is_valid_effective_pair(requested.skin, mode) {
        let fallback = UiPreferences::safe_fallback(locale, motion);
        return ResolvedUiPreferences {
            locale: fallback.locale,
            skin: fallback.skin,
            mode: fallback.mode,
            density: fallback.density,
            motion: fallback.motion,
            diagnostics: vec![UiPreferenceDiagnostic::new(
                "ui.invalid_skin_mode_pair",
                "skin_mode",
                "ui.mode",
                Some(format!("{:?}/{:?}", requested.skin, mode).to_ascii_lowercase()),
            )],
        };
    }

    ResolvedUiPreferences {
        locale,
        skin: requested.skin,
        mode,
        density: requested.density,
        motion,
        diagnostics: Vec::new(),
    }
}

fn resolve_locale(requested: LocaleId, client: LocaleId) -> LocaleId {
    match requested {
        LocaleId::En | LocaleId::ZhCn => requested,
        LocaleId::System => match client {
            LocaleId::En | LocaleId::ZhCn => client,
            LocaleId::System => LocaleId::En,
        },
    }
}

fn resolve_mode(requested: UiColorMode, client: UiColorMode) -> UiColorMode {
    match requested {
        UiColorMode::Dark | UiColorMode::Light => requested,
        UiColorMode::System => match client {
            UiColorMode::Dark | UiColorMode::Light => client,
            UiColorMode::System => UiColorMode::Dark,
        },
    }
}
