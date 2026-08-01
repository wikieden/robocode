//! Viden's production desktop client boundary.

mod adapter;
mod d1;
mod d10;
mod d2;
mod d4;
mod d6;
mod permission;
mod presentation;
mod projection;

use std::sync::Mutex;
use std::time::Duration;
use std::{env, ffi::OsString, path::Path};

pub use adapter::{D11Intent, D11IntentResult, GuiCoreAdapter, open_local_workspace};
pub use d1::{
    D1_OWNER_CAPABILITY, D1AgentSessionInputProjection, D1AgentSessionProjection,
    D1ChecklistItemProjection, D1CockpitProjection, D1ContextDockProjection,
    D1ContextUsageProjection, D1CostUsageProjection, D1CursorProjection, D1Intent, D1IntentResult,
    D1LaneAgentProjection, D1OutcomeProjection, D1ProviderHealthProjection,
    D1RuntimeServiceProjection, D1StarterLaneReceiptProjection, D1WorkspaceSourceProjection,
};
pub use d2::{
    D2_KIND_CONTRACT, D2_KIND_GATE, D2_KIND_REVIEW, D2ActionProjection, D2ContextProjection,
    D2DecisionsProjection, D2DetailProjection, D2EvidenceProjection, D2GroupProjection, D2Intent,
    D2IntentResult, D2QueueItemProjection, D2UnavailableProjection,
};
pub use d4::{
    D4_STARTER_LANE_CAPABILITY, D4ApprovalIntent, D4Intent, D4IntentResult, D4LaneCreateProjection,
    D4LaneRequest, D4Preset,
};
pub use d6::{D6ActionProjection, D6ConnectionState, D6RecoveryProjection, D6State};
pub use d10::{
    D10AgentProjection, D10EvidenceProjection, D10LaneMonitorProjection, D10LaneProjection,
};
pub use permission::{
    PermissionActionProjection, PermissionChoice, PermissionDockProjection, PermissionIntent,
    PermissionIntentResult, PermissionOutcomeProjection, PermissionRequestProjection,
    PermissionTargetProjection,
};

pub use presentation::{
    ComposerAction, ComposerDraft, GuiPreferences, TranscriptRow, TranscriptViewport,
    WorkspaceSelection,
};
pub use projection::{D11IntakeProjection, ResolvedPreferencesProjection, RuntimeProjection};

struct DesktopState {
    adapter: Mutex<Option<GuiCoreAdapter>>,
}

#[tauri::command]
fn open_workspace(root: String, state: tauri::State<'_, DesktopState>) -> Result<(), String> {
    // Build and connect the replacement before taking the shared slot so a
    // failed folder validation never discards the currently open workspace.
    let replacement = open_local_workspace(&root)?;
    let mut adapter = state
        .adapter
        .lock()
        .map_err(|_| "GUI Core adapter lock is unavailable".to_string())?;
    *adapter = Some(replacement);
    Ok(())
}

#[tauri::command]
fn resolved_preferences(
    state: tauri::State<'_, DesktopState>,
) -> Option<ResolvedPreferencesProjection> {
    state
        .adapter
        .lock()
        .ok()?
        .as_ref()?
        .projection()
        .preferences()
}

#[tauri::command]
fn d11_intake(state: tauri::State<'_, DesktopState>) -> Option<D11IntakeProjection> {
    state
        .adapter
        .lock()
        .ok()?
        .as_ref()?
        .projection()
        .d11_intake()
}

#[tauri::command]
fn d11_send_intent(
    command_id: String,
    intent: D11Intent,
    state: tauri::State<'_, DesktopState>,
) -> Result<D11IntentResult, String> {
    state
        .adapter
        .lock()
        .map_err(|_| "GUI Core adapter lock is unavailable".to_string())?
        .as_mut()
        .ok_or_else(|| "Core adapter is not connected".to_string())?
        .send_d11_intent_and_wait(&command_id, intent, Duration::from_millis(250))
}

#[tauri::command]
fn d11_poll(state: tauri::State<'_, DesktopState>) -> Result<D11IntentResult, String> {
    state
        .adapter
        .lock()
        .map_err(|_| "GUI Core adapter lock is unavailable".to_string())?
        .as_mut()
        .ok_or_else(|| "Core adapter is not connected".to_string())?
        .poll_d11(Duration::ZERO)
}

#[tauri::command]
fn d4_lane_create(state: tauri::State<'_, DesktopState>) -> Option<D4LaneCreateProjection> {
    state.adapter.lock().ok()?.as_ref()?.d4_lane_create()
}

#[tauri::command]
fn d4_send_intent(
    command_id: String,
    intent: D4Intent,
    state: tauri::State<'_, DesktopState>,
) -> Result<D4IntentResult, String> {
    state
        .adapter
        .lock()
        .map_err(|_| "GUI Core adapter lock is unavailable".to_string())?
        .as_mut()
        .ok_or_else(|| "Core adapter is not connected".to_string())?
        .send_d4_intent_and_wait(&command_id, intent, Duration::from_millis(250))
}

#[tauri::command]
fn d4_poll(state: tauri::State<'_, DesktopState>) -> Result<D4IntentResult, String> {
    state
        .adapter
        .lock()
        .map_err(|_| "GUI Core adapter lock is unavailable".to_string())?
        .as_mut()
        .ok_or_else(|| "Core adapter is not connected".to_string())?
        .poll_d4(Duration::ZERO)
}

#[tauri::command]
fn d1_cockpit(
    selected_lane_id: Option<String>,
    state: tauri::State<'_, DesktopState>,
) -> Option<D1CockpitProjection> {
    state
        .adapter
        .lock()
        .ok()?
        .as_ref()?
        .d1_cockpit(selected_lane_id.as_deref())
}

#[tauri::command]
fn d1_send_intent(
    command_id: String,
    intent: D1Intent,
    state: tauri::State<'_, DesktopState>,
) -> Result<D1IntentResult, String> {
    state
        .adapter
        .lock()
        .map_err(|_| "GUI Core adapter lock is unavailable".to_string())?
        .as_mut()
        .ok_or_else(|| "Core adapter is not connected".to_string())?
        .send_d1_intent_and_wait(&command_id, intent, Duration::from_millis(250))
}

#[tauri::command]
fn d1_poll(
    selected_lane_id: Option<String>,
    wait_for_event: bool,
    state: tauri::State<'_, DesktopState>,
) -> Result<D1IntentResult, String> {
    let mut guard = state
        .adapter
        .lock()
        .map_err(|_| "GUI Core adapter lock is unavailable".to_string())?;
    let adapter = guard
        .as_mut()
        .ok_or_else(|| "Core adapter is not connected".to_string())?;
    let timeout = if wait_for_event {
        Duration::from_millis(250)
    } else {
        Duration::ZERO
    };
    adapter.poll_d1(selected_lane_id.as_deref(), timeout)
}

#[tauri::command]
fn permission_send_intent(
    command_id: String,
    intent: PermissionIntent,
    state: tauri::State<'_, DesktopState>,
) -> Result<PermissionIntentResult, String> {
    state
        .adapter
        .lock()
        .map_err(|_| "GUI Core adapter lock is unavailable".to_string())?
        .as_mut()
        .ok_or_else(|| "Core adapter is not connected".to_string())?
        .send_permission_intent_and_wait(&command_id, intent, Duration::from_millis(250))
}

#[tauri::command]
fn permission_poll(
    state: tauri::State<'_, DesktopState>,
) -> Result<PermissionIntentResult, String> {
    state
        .adapter
        .lock()
        .map_err(|_| "GUI Core adapter lock is unavailable".to_string())?
        .as_mut()
        .ok_or_else(|| "Core adapter is not connected".to_string())?
        .poll_permission(Duration::ZERO)
}

#[tauri::command]
fn d10_lane_monitor(
    state: tauri::State<'_, DesktopState>,
) -> Result<Option<D10LaneMonitorProjection>, String> {
    Ok(state
        .adapter
        .lock()
        .map_err(|_| "GUI Core adapter lock is unavailable".to_string())?
        .as_ref()
        .ok_or_else(|| "Core adapter is not connected".to_string())?
        .d10_lane_monitor())
}

#[tauri::command]
fn d2_decisions(
    selected_id: Option<String>,
    state: tauri::State<'_, DesktopState>,
) -> Result<Option<D2DecisionsProjection>, String> {
    let guard = state
        .adapter
        .lock()
        .map_err(|_| "GUI Core adapter lock is unavailable".to_string())?;
    let adapter = guard
        .as_ref()
        .ok_or_else(|| "Core adapter is not connected".to_string())?;
    Ok(match selected_id {
        Some(id) => adapter.d2_decisions_for(&id),
        None => adapter.d2_decisions(),
    })
}

#[tauri::command]
fn d2_send_intent(
    command_id: String,
    intent: D2Intent,
    state: tauri::State<'_, DesktopState>,
) -> Result<D2IntentResult, String> {
    state
        .adapter
        .lock()
        .map_err(|_| "GUI Core adapter lock is unavailable".to_string())?
        .as_mut()
        .ok_or_else(|| "Core adapter is not connected".to_string())?
        .d2_send_intent(&command_id, intent)
}

#[tauri::command]
fn d6_recover(state: tauri::State<'_, DesktopState>) -> Result<D6RecoveryProjection, String> {
    let mut guard = state
        .adapter
        .lock()
        .map_err(|_| "GUI Core adapter lock is unavailable".to_string())?;
    let adapter = guard
        .as_mut()
        .ok_or_else(|| "Core adapter is not connected".to_string())?;
    adapter.recover().map_err(|error| error.to_string())?;
    Ok(adapter.d6_recovery())
}

pub fn run() {
    install_desktop_command_path();
    let adapter = match adapter::default_local_adapter() {
        Ok(adapter) => adapter,
        Err(error) => {
            eprintln!("Viden Core workspace bootstrap failed: {error}");
            None
        }
    };
    run_with_adapter(adapter);
}

pub fn run_with_adapter(adapter: Option<GuiCoreAdapter>) {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(DesktopState {
            adapter: Mutex::new(adapter),
        })
        .invoke_handler(tauri::generate_handler![
            open_workspace,
            resolved_preferences,
            d11_intake,
            d11_send_intent,
            d11_poll,
            d4_lane_create,
            d4_send_intent,
            d4_poll,
            d1_cockpit,
            d1_send_intent,
            d1_poll,
            permission_send_intent,
            permission_poll,
            d2_decisions,
            d2_send_intent,
            d10_lane_monitor,
            d6_recover
        ])
        .run(tauri::generate_context!())
        .expect("failed to run the Viden desktop client");
}

fn install_desktop_command_path() {
    let current = env::var_os("PATH");
    let home = env::var_os("HOME").map(std::path::PathBuf::from);
    let path = desktop_command_path(current.clone(), home.as_deref());
    if Some(&path) != current.as_ref() {
        // Startup is still single-threaded here. Core discovery and every ACP
        // child spawned afterward must observe the same resolved command path.
        unsafe { env::set_var("PATH", path) };
    }
}

fn desktop_command_path(current: Option<OsString>, home: Option<&Path>) -> OsString {
    let mut entries = Vec::new();
    if let Some(home) = home {
        for relative in [
            ".local/bin",
            ".bun/bin",
            ".volta/bin",
            ".asdf/shims",
            ".local/share/mise/shims",
            ".local/share/fnm/aliases/default/bin",
        ] {
            let candidate = home.join(relative);
            if candidate.is_dir() && !entries.contains(&candidate) {
                entries.push(candidate);
            }
        }
    }
    #[cfg(target_os = "macos")]
    for candidate in [Path::new("/opt/homebrew/bin"), Path::new("/usr/local/bin")] {
        if candidate.is_dir() && !entries.iter().any(|entry| entry == candidate) {
            entries.push(candidate.to_path_buf());
        }
    }
    if let Some(current) = current.as_deref() {
        for entry in env::split_paths(current) {
            if !entries.contains(&entry) {
                entries.push(entry);
            }
        }
    }
    env::join_paths(entries).unwrap_or_else(|_| current.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use super::desktop_command_path;

    #[test]
    fn desktop_command_path_recovers_user_local_bin_from_restricted_path() {
        let home = env::temp_dir().join(format!("viden-gui-desktop-path-{}", std::process::id()));
        let user_bin = home.join(".local/bin");
        fs::create_dir_all(&user_bin).expect("create user-local bin fixture");

        let path = desktop_command_path(Some("/usr/bin:/bin".into()), Some(&home));
        let entries = env::split_paths(&path).collect::<Vec<_>>();

        assert_eq!(entries.first(), Some(&user_bin));
        assert!(entries.contains(&"/usr/bin".into()));
        assert!(entries.contains(&"/bin".into()));
        fs::remove_dir_all(home).expect("remove desktop path fixture");
    }
}
