pub(super) use viden_core::LocaleId;
use viden_core::ResolvedUiPreferences;

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
    use super::{LocaleId, resolve_preferences};
    use viden_core::ResolvedUiPreferences;

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
}
