use super::{
    composer_buffer::ComposerBuffer,
    keymap::{InputMode, OverlayKind},
    preferences::{ColorDepth, SettingsPanel},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProviderAuthMode {
    ApiKey,
    WebLogin,
    Local,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProviderOption {
    pub(super) provider_id: String,
    pub(super) display_name: String,
    pub(super) default_api_base: Option<String>,
    pub(super) default_model: Option<String>,
    pub(super) known_models: Vec<String>,
    pub(super) enabled_models: Vec<String>,
    pub(super) favorite_models: Vec<String>,
    pub(super) api_key_env: Option<String>,
    pub(super) api_base_env: Option<String>,
    pub(super) auth_modes: Vec<ProviderAuthMode>,
}

impl ProviderOption {
    pub(super) fn fixture() -> Vec<Self> {
        vec![
            Self::api_key(
                "anthropic",
                "Anthropic",
                "https://api.anthropic.com",
                "claude-sonnet-4-5",
                &["claude-opus-4-5", "claude-sonnet-4-5", "claude-haiku-4-5"],
                "ANTHROPIC_API_KEY",
            ),
            Self::api_key(
                "deepseek",
                "DeepSeek",
                "https://api.deepseek.com",
                "deepseek-v4-flash",
                &[
                    "deepseek-v4-flash",
                    "deepseek-v4-pro",
                    "deepseek-chat",
                    "deepseek-reasoner",
                ],
                "DEEPSEEK_API_KEY",
            ),
            Self::api_key(
                "dashscope-coding-plan",
                "DashScope Coding Plan",
                "https://coding.dashscope.aliyuncs.com/v1",
                "qwen3.6-plus",
                &[
                    "qwen3.6-plus",
                    "qwen3.5-plus",
                    "qwen3-coder-next",
                    "qwen3-coder-plus",
                    "kimi-k2.5",
                    "glm-5",
                    "MiniMax-M2.5",
                ],
                "DASHSCOPE_CODING_PLAN_API_KEY",
            ),
            Self::api_key(
                "dashscope-tokenplan",
                "DashScope TokenPlan",
                "https://token-plan.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
                "qwen3.6-plus",
                &[
                    "qwen3.7-max",
                    "qwen3.6-plus",
                    "qwen3.6-flash",
                    "deepseek-v4-flash",
                    "kimi-k2.6",
                    "glm-5.1",
                    "MiniMax-M2.5",
                ],
                "DASHSCOPE_API_KEY",
            ),
            Self::api_key(
                "openrouter",
                "OpenRouter",
                "https://openrouter.ai/api/v1",
                "deepseek/deepseek-v4-flash",
                &[
                    "openai/gpt-5.2",
                    "anthropic/claude-sonnet-4.5",
                    "qwen/qwen3-coder-plus",
                    "deepseek/deepseek-v4-flash",
                ],
                "OPENROUTER_API_KEY",
            ),
            Self {
                provider_id: "fallback".to_string(),
                display_name: "Fallback".to_string(),
                default_api_base: None,
                default_model: Some("fallback-local".to_string()),
                known_models: vec!["fallback-local".to_string(), "test-local".to_string()],
                enabled_models: vec!["fallback-local".to_string(), "test-local".to_string()],
                favorite_models: Vec::new(),
                api_key_env: None,
                api_base_env: None,
                auth_modes: vec![ProviderAuthMode::Local],
            },
            Self {
                provider_id: "openai".to_string(),
                display_name: "OpenAI".to_string(),
                default_api_base: Some("https://api.openai.com/v1".to_string()),
                default_model: Some("gpt-5.2".to_string()),
                known_models: vec![
                    "gpt-5.2".to_string(),
                    "gpt-5.2-codex".to_string(),
                    "gpt-5.1".to_string(),
                ],
                enabled_models: vec!["gpt-5.2".to_string()],
                favorite_models: Vec::new(),
                api_key_env: Some("OPENAI_API_KEY".to_string()),
                api_base_env: Some("VIDEN_API_BASE".to_string()),
                auth_modes: vec![ProviderAuthMode::WebLogin, ProviderAuthMode::ApiKey],
            },
        ]
    }

    fn api_key(
        provider_id: &str,
        display_name: &str,
        endpoint: &str,
        default_model: &str,
        models: &[&str],
        api_key_env: &str,
    ) -> Self {
        Self {
            provider_id: provider_id.to_string(),
            display_name: display_name.to_string(),
            default_api_base: Some(endpoint.to_string()),
            default_model: Some(default_model.to_string()),
            known_models: models.iter().map(|model| (*model).to_string()).collect(),
            enabled_models: vec![default_model.to_string()],
            favorite_models: Vec::new(),
            api_key_env: Some(api_key_env.to_string()),
            api_base_env: None,
            auth_modes: vec![ProviderAuthMode::ApiKey],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum InteractionPanel {
    Settings(Box<SettingsPanel>),
    Setup {
        selected: usize,
        draft: String,
    },
    ConnectProvider {
        search: String,
        selected: usize,
    },
    #[allow(dead_code)]
    ProviderConfig {
        provider_id: String,
        selected: usize,
    },
    ModelPicker {
        provider_id: Option<String>,
        search: String,
        selected: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TuiEntry {
    pub(super) label: String,
    pub(super) body: String,
}

/// Local navigation only. Runtime facts and side effects remain Core-owned;
/// changing a lens never confirms project, lane, session, or approval state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Lens {
    Welcome,
    Setup,
    Board,
    Session,
    Decisions,
    Gallery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OverlayState {
    pub(super) kind: OverlayKind,
    pub(super) filter: String,
    pub(super) selected: usize,
    pub(super) selected_id: Option<String>,
    pub(super) previous_overlay: Option<Box<OverlayState>>,
}

impl OverlayState {
    pub(super) fn new(kind: OverlayKind) -> Self {
        Self {
            kind,
            filter: String::new(),
            selected: 0,
            selected_id: None,
            previous_overlay: None,
        }
    }

    pub(super) fn global_jump(previous_overlay: Option<OverlayState>) -> Self {
        Self {
            previous_overlay: previous_overlay.map(Box::new),
            ..Self::new(OverlayKind::GlobalJump)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TuiUiState {
    pub(super) session_id: String,
    pub(super) lens: Lens,
    pub(super) right_rail_open: bool,
    pub(super) provider_catalog: Vec<ProviderOption>,
    pub(super) theme_name: String,
    pub(super) input: ComposerBuffer,
    pub(super) command_selection: usize,
    pub(super) command_palette_hidden_for: Option<String>,
    pub(super) approval_focus: usize,
    pub(super) approval_apply_all: bool,
    pub(super) transcript_scroll: usize,
    pub(super) entries: Vec<TuiEntry>,
    pub(super) focused_lane: Option<String>,
    pub(super) interaction_panel: Option<InteractionPanel>,
    pub(super) input_mode: InputMode,
    pub(super) overlay: Option<OverlayState>,
    pub(super) idle_ctrl_c_armed: bool,
    pub(super) color_depth: ColorDepth,
    pub(super) preference_diagnostics: Vec<String>,
}

impl Default for TuiUiState {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            lens: Lens::Welcome,
            right_rail_open: false,
            provider_catalog: Vec::new(),
            theme_name: "aurora-cyan".to_string(),
            input: ComposerBuffer::default(),
            command_selection: 0,
            command_palette_hidden_for: None,
            approval_focus: 0,
            approval_apply_all: false,
            transcript_scroll: 0,
            entries: Vec::new(),
            focused_lane: None,
            interaction_panel: None,
            input_mode: InputMode::Normal,
            overlay: None,
            idle_ctrl_c_armed: false,
            color_depth: ColorDepth::Auto,
            preference_diagnostics: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lens_defaults_to_welcome_and_right_rail_is_closed() {
        let ui = TuiUiState::default();

        assert_eq!(ui.lens, Lens::Welcome);
        assert!(!ui.right_rail_open);
    }
}
