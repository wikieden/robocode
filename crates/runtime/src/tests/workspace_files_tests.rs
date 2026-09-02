//! Runtime behavior of the permission-gated workspace file inventory
//! (GUI-CORE-022).
//!
//! Three things must hold for the inventory to be usable by a frontend that is
//! forbidden from walking the filesystem itself:
//!
//! 1. the permission engine decides *before* any directory is read, and a
//!    refusal is published as an error rather than as an empty page;
//! 2. runtime and agent state directories never appear, whatever the
//!    workspace's `.gitignore` says;
//! 3. ordering, prefix filtering, and the cursor are applied to the whole
//!    inventory, so `complete` and `next_after` describe the filtered ordered
//!    tree rather than whatever the walker produced first.

use std::fs;

use viden_types::{
    PermissionBehavior, PermissionRule, PermissionRuleSource, PermissionRuleValue, RuntimeCommand,
    RuntimeEventKind, WorkMode, WorkspaceFileKind, WorkspaceFilePage, WorkspaceFilesQuery,
};

use super::{SequenceProvider, temp_dir};
use crate::SessionEngine;

/// Builds a workspace whose shape exercises ordering, gitignore, and the
/// unconditional runtime-state exclusions in one walk.
fn workspace_engine(name: &str) -> (std::path::PathBuf, SessionEngine) {
    let cwd = temp_dir(&format!("{name}_cwd"));
    let home = temp_dir(&format!("{name}_home"));
    fs::write(cwd.join(".gitignore"), "target/\nsecret.env\n").unwrap();
    fs::write(cwd.join("README.md"), "readme bytes").unwrap();
    fs::write(cwd.join("secret.env"), "TOKEN=redacted").unwrap();
    fs::create_dir_all(cwd.join("src")).unwrap();
    fs::write(cwd.join("src/lib.rs"), "pub fn main() {}").unwrap();
    fs::write(cwd.join("src/main.rs"), "fn main() {}").unwrap();
    fs::create_dir_all(cwd.join("target/debug")).unwrap();
    fs::write(cwd.join("target/debug/binary"), "build artifact").unwrap();
    // Runtime and agent state, deliberately *not* gitignored here so the test
    // proves the exclusion is unconditional rather than inherited from the
    // workspace's own ignore file.
    for state_dir in [".git", ".viden", ".omx", ".worktrees", ".ref"] {
        fs::create_dir_all(cwd.join(state_dir)).unwrap();
        fs::write(cwd.join(state_dir).join("state.json"), "{}").unwrap();
    }
    let engine = SessionEngine::new_with_home(
        &cwd,
        Box::new(SequenceProvider::new(Vec::new())),
        Some(home),
    )
    .unwrap();
    (cwd, engine)
}

fn inventory_rule(rule_behavior: PermissionBehavior) -> PermissionRule {
    PermissionRule {
        source: PermissionRuleSource::Session,
        rule_behavior,
        rule_value: PermissionRuleValue {
            tool_name: "workspace_file_inventory".to_string(),
            rule_content: None,
        },
    }
}

fn query_files(
    engine: &mut SessionEngine,
    command_id: &str,
    query: WorkspaceFilesQuery,
) -> Vec<viden_types::RuntimeEvent> {
    let mut denier = |_prompt| panic!("a read-only inventory query must not request approval");
    engine
        .handle_runtime_command(
            command_id,
            RuntimeCommand::QueryWorkspaceFiles { query },
            &mut denier,
        )
        .unwrap()
}

fn page(events: &[viden_types::RuntimeEvent], expected_command_id: &str) -> WorkspaceFilePage {
    let loaded = events
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::WorkspaceFilesLoaded { command_id, page } => {
                Some((command_id.clone(), page.clone()))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected a WorkspaceFilesLoaded, got {events:?}"));
    assert_eq!(
        loaded.0, expected_command_id,
        "the page must name the exact read it answers"
    );
    loaded.1
}

#[test]
fn workspace_file_inventory_lists_the_tree_in_lexicographic_order_with_its_command_id() {
    let (_cwd, mut engine) = workspace_engine("workspace_files_order");
    let events = query_files(&mut engine, "files-read-1", WorkspaceFilesQuery::default());
    assert!(matches!(
        &events[0].kind,
        RuntimeEventKind::CommandAccepted { command_id, .. } if command_id == "files-read-1"
    ));
    let page = page(&events, "files-read-1");
    let paths = page
        .entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths, sorted, "entries must be lexicographic by path");
    assert!(paths.contains(&"README.md".to_string()));
    assert!(paths.contains(&"src/lib.rs".to_string()));
    assert!(paths.contains(&"src".to_string()));
    assert!(page.complete);
    assert_eq!(page.next_after, None);

    // A directory carries no size; a file carries the bytes on disk.
    let dir = page
        .entries
        .iter()
        .find(|entry| entry.path == "src")
        .expect("the walk must publish the directory itself");
    assert_eq!(dir.kind, WorkspaceFileKind::Dir);
    assert_eq!(dir.size_bytes, None);
    let file = page
        .entries
        .iter()
        .find(|entry| entry.path == "README.md")
        .expect("the walk must publish the readme");
    assert_eq!(file.kind, WorkspaceFileKind::File);
    assert_eq!(file.size_bytes, Some("readme bytes".len() as u64));
}

#[test]
fn workspace_file_inventory_honors_gitignore_and_always_excludes_runtime_state() {
    let (_cwd, mut engine) = workspace_engine("workspace_files_excluded");
    let page = page(
        &query_files(&mut engine, "files-read-1", WorkspaceFilesQuery::default()),
        "files-read-1",
    );
    let paths = page
        .entries
        .iter()
        .map(|entry| entry.path.as_str())
        .collect::<Vec<_>>();
    // Gitignored content is workspace content the operator chose to hide.
    for ignored in ["target", "target/debug/binary", "secret.env"] {
        assert!(
            !paths.contains(&ignored),
            "gitignored `{ignored}` must not appear, got {paths:?}"
        );
    }
    // Runtime and agent state is never workspace content, gitignore or not.
    for state in [".git", ".viden", ".omx", ".worktrees", ".ref"] {
        assert!(
            !paths
                .iter()
                .any(|path| path == &state || path.starts_with(&format!("{state}/"))),
            "runtime state `{state}` must never appear, got {paths:?}"
        );
    }
}

#[test]
fn workspace_file_inventory_pages_the_filtered_ordered_inventory() {
    let (_cwd, mut engine) = workspace_engine("workspace_files_paging");
    let first = page(
        &query_files(
            &mut engine,
            "files-read-1",
            WorkspaceFilesQuery {
                prefix: Some("src".to_string()),
                limit: Some(1),
                after: None,
            },
        ),
        "files-read-1",
    );
    // The prefix is applied before the page is cut, so `complete` describes the
    // filtered inventory and not the whole tree.
    assert_eq!(first.entries.len(), 1);
    assert!(!first.complete);
    assert_eq!(
        first.next_after.as_deref(),
        Some(first.entries[0].path.as_str())
    );

    let second = page(
        &query_files(
            &mut engine,
            "files-read-2",
            WorkspaceFilesQuery {
                prefix: Some("src".to_string()),
                limit: Some(50),
                after: first.next_after.clone(),
            },
        ),
        "files-read-2",
    );
    // The cursor is exclusive, so the two pages tile the subtree without
    // repeating the boundary entry.
    assert!(
        !second
            .entries
            .iter()
            .any(|entry| Some(&entry.path) == first.next_after.as_ref()),
        "the resume cursor must be exclusive"
    );
    assert!(second.complete);
    assert_eq!(second.next_after, None);
    let mut all = first
        .entries
        .iter()
        .chain(second.entries.iter())
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    let sorted = {
        let mut sorted = all.clone();
        sorted.sort();
        sorted
    };
    assert_eq!(all, sorted);
    all.dedup();
    assert!(all.iter().all(|path| path.starts_with("src")));
    assert!(all.contains(&"src/lib.rs".to_string()));
    assert!(all.contains(&"src/main.rs".to_string()));
}

#[test]
fn workspace_file_inventory_answers_in_plan_mode_without_prompting() {
    let (_cwd, mut engine) = workspace_engine("workspace_files_plan");
    engine.set_work_mode(WorkMode::Plan).unwrap();
    // The engine's SafeRead branch allows a non-mutating tool in plan mode, so
    // a read stays answerable while every mutation is blocked. `query_files`
    // panics if the approver is ever reached.
    let page = page(
        &query_files(&mut engine, "files-plan", WorkspaceFilesQuery::default()),
        "files-plan",
    );
    assert!(!page.entries.is_empty());
}

/// Reads the rejection reason for the read named by `command_id`.
///
/// A refusal is published as `CommandRejected` carrying the caller's own
/// command id, never as a bare `Error`. An uncorrelated `Error` would be
/// indistinguishable, to a client with a read outstanding, from an unrelated
/// failure that happened to land in the same window, so the client would
/// attribute a lane or provider error to its inventory read and render a
/// refusal that never happened.
fn rejection_reason(events: &[viden_types::RuntimeEvent], command_id: &str) -> String {
    assert!(
        !events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::WorkspaceFilesLoaded { .. })),
        "a refused read must never publish a page, empty or otherwise"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::Error { .. })),
        "a refusal must be attributable, so it is never a bare Error event"
    );
    events
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::CommandRejected {
                command_id: rejected,
                reason,
            } if rejected == command_id => Some(reason.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected CommandRejected for {command_id}, got {events:?}"))
}

#[test]
fn a_denied_workspace_file_inventory_is_rejected_with_the_exact_command_id() {
    let (_cwd, mut engine) = workspace_engine("workspace_files_denied");
    engine.add_permission_rule_for_test(inventory_rule(PermissionBehavior::Deny));
    let mut denier = |_prompt| panic!("a denied inventory must not reach an approval prompt");
    let events = engine
        .handle_runtime_command(
            "files-denied",
            RuntimeCommand::QueryWorkspaceFiles {
                query: WorkspaceFilesQuery::default(),
            },
            &mut denier,
        )
        .unwrap();
    let reason = rejection_reason(&events, "files-denied");
    assert!(
        reason.contains("workspace_file_inventory"),
        "the reason must name the permission that refused, got {reason}"
    );
    assert!(
        reason.contains("grant the workspace file inventory permission"),
        "the reason must keep the actionable grant hint, got {reason}"
    );
}

#[test]
fn an_ask_rule_on_the_inventory_is_rejected_without_blocking_on_approval() {
    let (_cwd, mut engine) = workspace_engine("workspace_files_ask");
    engine.add_permission_rule_for_test(inventory_rule(PermissionBehavior::Ask));
    // A read query is not an interactive turn: blocking the client's read on an
    // approval prompt would stall a palette keystroke behind a modal. The
    // honest answer is the same refusal an operator would see, attributed to
    // the exact read, so the approver must never be reached.
    let mut denier = |_prompt| panic!("a read query must never block on an approval prompt");
    let events = engine
        .handle_runtime_command(
            "files-ask",
            RuntimeCommand::QueryWorkspaceFiles {
                query: WorkspaceFilesQuery::default(),
            },
            &mut denier,
        )
        .unwrap();
    let reason = rejection_reason(&events, "files-ask");
    assert!(
        reason.contains("workspace_file_inventory"),
        "an unresolved ask must surface as a named refusal, got {reason}"
    );
}

#[test]
fn a_workspace_files_query_with_an_escaping_prefix_is_rejected_before_the_walk() {
    let (_cwd, mut engine) = workspace_engine("workspace_files_escape");
    let mut denier = |_prompt| panic!("a rejected query must not request approval");
    let events = engine
        .handle_runtime_command(
            "files-escape",
            RuntimeCommand::QueryWorkspaceFiles {
                query: WorkspaceFilesQuery {
                    prefix: Some("../..".to_string()),
                    ..WorkspaceFilesQuery::default()
                },
            },
            &mut denier,
        )
        .unwrap();
    assert!(
        events.iter().any(|event| matches!(
            &event.kind,
            RuntimeEventKind::CommandRejected { command_id, .. } if command_id == "files-escape"
        )),
        "an escaping prefix must be rejected, got {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::WorkspaceFilesLoaded { .. })),
        "a rejected query must publish no page"
    );
}
