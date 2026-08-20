//! Personal UI preference mutation, as a client of the Core contract.
//!
//! Core owns the whole loop: it previews the write, runs the permission gate,
//! writes the user `[ui]` table atomically, re-resolves precedence, and
//! publishes `UiPreferencesUpdated`. This module is the transport-safe
//! vocabulary the webview and the shell share; the typed Core command is built
//! in `adapter.rs`, which is where Core contract types live. The GUI never
//! writes a preference file, never re-resolves precedence, and never treats
//! its own optimistic preview as a persistence confirmation.

use serde::{Deserialize, Serialize};

use crate::d1::D1OutcomeProjection;
use crate::projection::{PreferenceDiagnosticProjection, ResolvedPreferencesProjection};

/// The frontend-contract-v1 capability that carries preference persistence.
///
/// This is the id Core actually publishes in its handshake
/// (`FRONTEND_V1_EXTENSION_CAPABILITIES`); the client must not invent a
/// finer-grained one, because an unpublished id can never become available.
pub const UI_PREFERENCE_PERSISTENCE_CAPABILITY: &str = "ui.preference_persistence";

/// The preference axes as the webview sends them.
///
/// Every field is optional: only axes the operator explicitly selected enter
/// the typed patch, so an unspecified axis keeps whatever Core resolves.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferencePatchInput {
    pub locale: Option<String>,
    pub skin: Option<String>,
    pub mode: Option<String>,
    pub density: Option<String>,
    pub motion: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PreferenceIntent {
    Save { patch: PreferencePatchInput },
    Restore,
}

/// What the client may render after one preference command.
///
/// `preferences` and `diagnostics` are populated only from the confirming
/// `UiPreferencesUpdated` fact, so a pending or rejected command never leaves
/// the panel showing a value Core did not publish.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferenceIntentResult {
    pub outcome: D1OutcomeProjection,
    pub preferences: Option<ResolvedPreferencesProjection>,
    /// Whether the confirming event carried a persisted user `[ui]` table.
    /// A reset confirms with `persisted: None` while the fallback still
    /// resolves, so this is `false` on a confirmed restore.
    pub persisted: bool,
    pub diagnostics: Vec<PreferenceDiagnosticProjection>,
    pub pending_command_id: Option<String>,
    pub capability_available: bool,
}
