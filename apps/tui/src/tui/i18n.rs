#[cfg(test)]
use std::collections::BTreeSet;
use std::{collections::BTreeMap, sync::OnceLock};

pub(super) use viden_core::LocaleId;

const EN_CATALOG: &str = include_str!("../../i18n/en.json");
const ZH_CN_CATALOG: &str = include_str!("../../i18n/zh-CN.json");

static EN_VALUES: OnceLock<BTreeMap<String, String>> = OnceLock::new();
static ZH_CN_VALUES: OnceLock<BTreeMap<String, String>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Catalog {
    locale: LocaleId,
}

impl Catalog {
    pub(super) fn new(locale: LocaleId) -> Self {
        Self {
            locale: effective_locale(locale),
        }
    }

    pub(super) fn translate(self, key: &str, args: &[(&str, &str)]) -> String {
        let Some(template) = catalog_values(self.locale).get(key) else {
            return key.to_string();
        };
        args.iter()
            .fold(template.clone(), |rendered, (name, value)| {
                rendered.replace(&format!("{{{name}}}"), value)
            })
    }
}

pub(super) fn catalog_for(state: &super::state::TuiState) -> Catalog {
    let preferences =
        super::preferences::TuiPreferences::from_resolved(&state.runtime.snapshot.ui_preferences);
    Catalog::new(preferences.locale)
}

pub(super) fn text(state: &super::state::TuiState, key: &str) -> String {
    catalog_for(state).translate(key, &[])
}

pub(super) fn translate(
    state: &super::state::TuiState,
    key: &str,
    args: &[(&str, &str)],
) -> String {
    catalog_for(state).translate(key, args)
}

fn effective_locale(locale: LocaleId) -> LocaleId {
    match locale {
        LocaleId::ZhCn => LocaleId::ZhCn,
        LocaleId::System | LocaleId::En => LocaleId::En,
    }
}

fn catalog_values(locale: LocaleId) -> &'static BTreeMap<String, String> {
    match effective_locale(locale) {
        LocaleId::ZhCn => ZH_CN_VALUES.get_or_init(|| parse_catalog(ZH_CN_CATALOG)),
        LocaleId::System | LocaleId::En => EN_VALUES.get_or_init(|| parse_catalog(EN_CATALOG)),
    }
}

fn parse_catalog(source: &str) -> BTreeMap<String, String> {
    serde_json::from_str(source).expect("embedded TUI catalog must be valid JSON")
}

#[cfg(test)]
fn catalog_parameter_sets(locale: LocaleId) -> BTreeMap<String, BTreeSet<String>> {
    catalog_values(locale)
        .iter()
        .map(|(key, value)| (key.clone(), parameters(value)))
        .collect()
}

#[cfg(test)]
fn parameters(template: &str) -> BTreeSet<String> {
    let mut parameters = BTreeSet::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let after_open = &rest[open + 1..];
        let Some(close) = after_open.find('}') else {
            break;
        };
        parameters.insert(after_open[..close].to_string());
        rest = &after_open[close + 1..];
    }
    parameters
}

#[cfg(test)]
mod tests {
    use super::{Catalog, LocaleId, catalog_parameter_sets, catalog_values};

    #[test]
    fn catalogs_have_exact_key_and_parameter_parity() {
        let english = catalog_values(LocaleId::En);
        let chinese = catalog_values(LocaleId::ZhCn);

        assert_eq!(
            english.keys().collect::<Vec<_>>(),
            chinese.keys().collect::<Vec<_>>()
        );
        assert!(english.values().all(|value| !value.trim().is_empty()));
        assert!(chinese.values().all(|value| !value.trim().is_empty()));
        assert_eq!(
            catalog_parameter_sets(LocaleId::En),
            catalog_parameter_sets(LocaleId::ZhCn)
        );
    }

    #[test]
    fn locale_aliases_fallbacks_and_missing_keys_are_visible() {
        assert_eq!(LocaleId::from_system_locale("zh-Hans-CN"), LocaleId::ZhCn);
        assert_eq!(LocaleId::from_system_locale("xx-Unknown"), LocaleId::En);
        assert_eq!(
            Catalog::new(LocaleId::ZhCn).translate("missing.visible.key", &[]),
            "missing.visible.key"
        );
    }

    #[test]
    fn interpolation_preserves_identifiers_paths_commands_and_shortcuts() {
        let catalog = Catalog::new(LocaleId::ZhCn);
        assert_eq!(
            catalog.translate(
                "approval.target",
                &[("target", "src/config.rs"), ("command", "git diff --check")]
            ),
            "目标  src/config.rs · git diff --check"
        );
        assert_eq!(
            catalog.translate("welcome.shortcuts", &[]),
            "↑↓ 移动 · Enter 打开 · Esc 关闭"
        );
    }
}
