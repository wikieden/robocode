//! Personal UI preferences flow through the Core persistence contract.
//!
//! The GUI never writes a preference file and never re-resolves precedence: it
//! sends `SetUiPreferences`/`ResetUiPreferences` and treats only the ordered
//! `UiPreferencesUpdated` fact as confirmation, mirroring the TUI settings
//! overlay (`apps/tui/src/tui/preferences.rs`). A local preview is not a
//! persistence confirmation.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use viden_core::{
    LocaleId, ResolvedUiPreferences, RuntimeCommand, RuntimeEventEnvelope, RuntimeEventKind,
    RuntimeSnapshot, RuntimeViewState, RuntimeWireEvent, UiColorMode, UiDensity, UiMotion,
    UiPreferenceDiagnostic, UiPreferencePatch, UiPreferences, UiSkin,
};
use viden_gui::{
    GuiCoreAdapter, PreferenceIntent, PreferencePatchInput, UI_PREFERENCE_PERSISTENCE_CAPABILITY,
};

mod support;
use support::TestCoreClient;

const D1_FIXTURE: &str = include_str!(
    "../../../crates/types/tests/fixtures/frontend-contract-v1/d1-vertical-slice.json"
);

#[derive(serde::Deserialize)]
struct Fixture {
    initial_snapshot: RuntimeSnapshot,
    events: Vec<RuntimeEventEnvelope>,
}

fn d1_view() -> RuntimeViewState {
    let fixture: Fixture = serde_json::from_str(D1_FIXTURE).expect("parse D1 fixture");
    let mut view = RuntimeViewState::new(fixture.initial_snapshot);
    for envelope in fixture.events {
        if let RuntimeWireEvent::Known(event) = envelope.event {
            view.apply_event(&event);
        }
    }
    view
}

fn connected(client: TestCoreClient) -> GuiCoreAdapter {
    let mut adapter = GuiCoreAdapter::new(Box::new(client));
    adapter.connect().expect("connect preference client");
    adapter
}

fn sent_commands(
    sent: &Arc<Mutex<Vec<viden_core::RuntimeCommandEnvelope>>>,
) -> Vec<viden_core::RuntimeCommandEnvelope> {
    sent.lock().expect("sent commands").clone()
}

fn save_intent(patch: PreferencePatchInput) -> PreferenceIntent {
    PreferenceIntent::Save { patch }
}

fn resolved(skin: UiSkin, mode: UiColorMode) -> ResolvedUiPreferences {
    ResolvedUiPreferences {
        locale: LocaleId::ZhCn,
        skin,
        mode,
        density: UiDensity::Comfy,
        motion: UiMotion::Reduced,
        diagnostics: Vec::new(),
    }
}

#[test]
fn save_sends_the_exact_set_ui_preferences_command() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut adapter = connected(TestCoreClient::new(d1_view(), Arc::clone(&sent)));

    let result = adapter
        .send_preference_intent_and_wait(
            "gui-pref-save",
            save_intent(PreferencePatchInput {
                locale: Some("zh-CN".into()),
                skin: Some("phosphor".into()),
                mode: Some("dark".into()),
                density: Some("comfy".into()),
                motion: Some("reduced".into()),
            }),
            Duration::ZERO,
        )
        .expect("preference save dispatches");

    let commands = sent_commands(&sent);
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].command_id, "gui-pref-save");
    assert_eq!(commands[0].owner, viden_core::RuntimeOwner::default());
    assert_eq!(
        commands[0].command,
        RuntimeCommand::SetUiPreferences {
            patch: UiPreferencePatch {
                locale: Some(LocaleId::ZhCn),
                skin: Some(UiSkin::Phosphor),
                mode: Some(UiColorMode::Dark),
                density: Some(UiDensity::Comfy),
                motion: Some(UiMotion::Reduced),
            }
        }
    );
    // Nothing confirmed the write yet, so the client reports no persistence.
    assert_eq!(result.outcome.state, "pending");
    assert_eq!(result.pending_command_id.as_deref(), Some("gui-pref-save"));
    assert!(!result.persisted);
    assert!(result.preferences.is_none());
}

#[test]
fn save_confirms_only_on_ui_preferences_updated_carrying_the_persisted_table() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let persisted = UiPreferences {
        locale: LocaleId::ZhCn,
        skin: UiSkin::Phosphor,
        mode: UiColorMode::Dark,
        density: UiDensity::Comfy,
        motion: UiMotion::Reduced,
    };
    let client = TestCoreClient::new(d1_view(), Arc::clone(&sent))
        .with_event(RuntimeEventKind::CommandAccepted {
            command_id: "gui-pref-save".into(),
            command: RuntimeCommand::SetUiPreferences {
                patch: UiPreferencePatch {
                    locale: Some(LocaleId::ZhCn),
                    skin: Some(UiSkin::Phosphor),
                    ..UiPreferencePatch::default()
                },
            },
        })
        // A snapshot republish is not a persistence receipt.
        .with_event(RuntimeEventKind::SnapshotUpdated {
            snapshot: d1_view().snapshot.clone(),
        })
        .with_event(RuntimeEventKind::UiPreferencesUpdated {
            resolved: ResolvedUiPreferences {
                diagnostics: vec![UiPreferenceDiagnostic::new(
                    "ui.preference_downgraded",
                    "ui.preference",
                    "mode",
                    Some("light".into()),
                )],
                ..resolved(UiSkin::Phosphor, UiColorMode::Dark)
            },
            persisted: Some(persisted),
            diagnostics: vec![UiPreferenceDiagnostic::new(
                "ui.preference_downgraded",
                "ui.preference",
                "mode",
                Some("light".into()),
            )],
        });
    let mut adapter = connected(client);

    let result = adapter
        .send_preference_intent_and_wait(
            "gui-pref-save",
            save_intent(PreferencePatchInput {
                locale: Some("zh-CN".into()),
                skin: Some("phosphor".into()),
                ..PreferencePatchInput::default()
            }),
            Duration::from_millis(10),
        )
        .expect("preference save confirms");

    assert_eq!(result.outcome.state, "confirmed");
    assert_eq!(result.pending_command_id, None);
    assert!(result.persisted);
    let preferences = result.preferences.expect("Core republished the resolution");
    assert_eq!(preferences.locale, "zh-CN");
    assert_eq!(preferences.skin, "phosphor");
    assert_eq!(preferences.mode, "dark");
    assert_eq!(preferences.density, "comfy");
    assert_eq!(preferences.motion, "reduced");
    // Diagnostics come from the confirming event, not a client re-resolution.
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].code, "ui.preference_downgraded");
    assert_eq!(
        result.diagnostics[0].rejected_value.as_deref(),
        Some("light")
    );
}

#[test]
fn a_persisted_table_that_contradicts_the_patch_does_not_confirm_the_save() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    // Another frontend's write lands first. It carries a persisted table, but
    // not the values this command asked Core to store.
    let client = TestCoreClient::new(d1_view(), Arc::clone(&sent))
        .with_event(RuntimeEventKind::CommandAccepted {
            command_id: "gui-pref-save".into(),
            command: RuntimeCommand::SetUiPreferences {
                patch: UiPreferencePatch {
                    skin: Some(UiSkin::Phosphor),
                    ..UiPreferencePatch::default()
                },
            },
        })
        .with_event(RuntimeEventKind::UiPreferencesUpdated {
            resolved: resolved(UiSkin::Ice, UiColorMode::Dark),
            persisted: Some(UiPreferences {
                locale: LocaleId::ZhCn,
                skin: UiSkin::Ice,
                mode: UiColorMode::Dark,
                density: UiDensity::Comfy,
                motion: UiMotion::Reduced,
            }),
            diagnostics: Vec::new(),
        });
    let mut adapter = connected(client);

    let result = adapter
        .send_preference_intent_and_wait(
            "gui-pref-save",
            save_intent(PreferencePatchInput {
                skin: Some("phosphor".into()),
                ..PreferencePatchInput::default()
            }),
            Duration::from_millis(10),
        )
        .expect("preference save dispatches");

    assert_eq!(result.outcome.state, "pending");
    assert!(!result.persisted);
    assert!(result.preferences.is_none());
}

#[test]
fn restore_confirms_on_a_ui_preferences_updated_with_no_persisted_table() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let client = TestCoreClient::new(d1_view(), Arc::clone(&sent))
        .with_event(RuntimeEventKind::CommandAccepted {
            command_id: "gui-pref-restore".into(),
            command: RuntimeCommand::ResetUiPreferences,
        })
        .with_event(RuntimeEventKind::UiPreferencesUpdated {
            resolved: ResolvedUiPreferences {
                locale: LocaleId::En,
                skin: UiSkin::Aurora,
                mode: UiColorMode::Dark,
                density: UiDensity::Regular,
                motion: UiMotion::System,
                diagnostics: Vec::new(),
            },
            // After a reset the user table is gone while the fallback still
            // resolves; `persisted` is None and stays that way.
            persisted: None,
            diagnostics: Vec::new(),
        });
    let mut adapter = connected(client);

    let result = adapter
        .send_preference_intent_and_wait(
            "gui-pref-restore",
            PreferenceIntent::Restore,
            Duration::from_millis(10),
        )
        .expect("preference restore confirms");

    let commands = sent_commands(&sent);
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].command, RuntimeCommand::ResetUiPreferences);
    assert_eq!(result.outcome.state, "confirmed");
    assert!(!result.persisted);
    let preferences = result.preferences.expect("the fallback still resolves");
    assert_eq!(preferences.skin, "aurora");
    assert_eq!(preferences.mode, "dark");
}

#[test]
fn a_core_rejection_carries_the_reason_through_to_the_client() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    // Plan/Review/Explore deny the mutation; Core answers with CommandRejected
    // and the config bytes stay unchanged.
    let client = TestCoreClient::new(d1_view(), Arc::clone(&sent)).with_event(
        RuntimeEventKind::CommandRejected {
            command_id: "gui-pref-save".into(),
            reason: "plan mode denies ui_preferences_set".into(),
        },
    );
    let mut adapter = connected(client);

    let result = adapter
        .send_preference_intent_and_wait(
            "gui-pref-save",
            save_intent(PreferencePatchInput {
                density: Some("compact".into()),
                ..PreferencePatchInput::default()
            }),
            Duration::from_millis(10),
        )
        .expect("a rejection is a projected outcome, not a transport error");

    assert_eq!(result.outcome.state, "rejected");
    assert_eq!(
        result.outcome.reason.as_deref(),
        Some("plan mode denies ui_preferences_set")
    );
    assert_eq!(result.pending_command_id, None);
    assert!(!result.persisted);
    assert!(result.preferences.is_none());
}

#[test]
fn an_unknown_wire_value_fails_closed_before_any_command_is_sent() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut adapter = connected(TestCoreClient::new(d1_view(), Arc::clone(&sent)));

    for (field, patch) in [
        (
            "locale",
            PreferencePatchInput {
                locale: Some("klingon".into()),
                ..PreferencePatchInput::default()
            },
        ),
        (
            "skin",
            PreferencePatchInput {
                skin: Some("neon".into()),
                ..PreferencePatchInput::default()
            },
        ),
        (
            "mode",
            PreferencePatchInput {
                mode: Some("sepia".into()),
                ..PreferencePatchInput::default()
            },
        ),
        (
            "density",
            PreferencePatchInput {
                density: Some("tight".into()),
                ..PreferencePatchInput::default()
            },
        ),
        (
            "motion",
            PreferencePatchInput {
                motion: Some("bouncy".into()),
                ..PreferencePatchInput::default()
            },
        ),
    ] {
        let error = adapter
            .send_preference_intent_and_wait("gui-pref-save", save_intent(patch), Duration::ZERO)
            .expect_err("an unknown wire value fails closed");
        assert!(error.contains(field), "{error}");
    }
    assert!(sent_commands(&sent).is_empty());
}

#[test]
fn an_empty_patch_is_not_a_command() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut adapter = connected(TestCoreClient::new(d1_view(), Arc::clone(&sent)));

    let error = adapter
        .send_preference_intent_and_wait(
            "gui-pref-save",
            save_intent(PreferencePatchInput::default()),
            Duration::ZERO,
        )
        .expect_err("an empty patch is not a preference change");
    assert!(error.contains("no preference change"), "{error}");
    assert!(sent_commands(&sent).is_empty());
}

#[test]
fn an_absent_capability_blocks_the_command_and_is_readable_by_the_client() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut client = TestCoreClient::new(d1_view(), Arc::clone(&sent));
    client
        .capabilities
        .remove(UI_PREFERENCE_PERSISTENCE_CAPABILITY);
    let mut adapter = connected(client);

    assert!(!adapter.supports_ui_preference_persistence());
    let error = adapter
        .send_preference_intent_and_wait(
            "gui-pref-save",
            save_intent(PreferencePatchInput {
                skin: Some("ice".into()),
                ..PreferencePatchInput::default()
            }),
            Duration::ZERO,
        )
        .expect_err("a missing capability fails closed");
    assert!(
        error.contains(UI_PREFERENCE_PERSISTENCE_CAPABILITY),
        "{error}"
    );
    assert!(sent_commands(&sent).is_empty());
}

#[test]
fn the_capability_is_read_from_the_handshake_when_core_publishes_it() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let adapter = connected(TestCoreClient::new(d1_view(), Arc::clone(&sent)));
    assert!(adapter.supports_ui_preference_persistence());
}
