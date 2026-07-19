use viden_gui_spike_gpui::{
    app::{D1Slice, ProjectionState},
    approval::ApprovalChoice,
    theme::{Density, Skin},
};

fn exercise_slice(mut app: D1Slice) -> D1Slice {
    app.composer.begin_composition();
    app.composer.update_composition("你好");
    assert!(!app.composer.submit());
    app.composer.commit_composition();
    app.composer.paste("第一行\n第二行");
    app.start_stream();
    app.queue_current_draft();
    app.cancel_stream();
    app.approval.respond(ApprovalChoice::AllowOnce);
    app.transcript.open_history_at("row-120");
    app.transcript.append_new_output("row-50001");
    app.theme.select(Skin::IceLight, Density::Comfy);
    app.focus("composer");
    app
}

#[test]
fn d1_slice_supports_cjk_streaming_approval_history_and_accessible_focus() {
    let app = exercise_slice(D1Slice::new(ProjectionState::fixture()));

    assert_eq!(app.composer.draft(), "你好第一行\n第二行");
    assert_eq!(app.transcript.anchor(), Some("row-120"));
    assert_eq!(app.transcript.new_output_count(), 1);
    assert_eq!(app.focused_role(), Some("composer"));
    assert!(app.visible_focus());
    assert_eq!(app.exposed_roles(), D1Slice::REQUIRED_ROLES);
}

#[test]
fn d1_slice_action_log_and_projection_hash_match_the_shared_contract() {
    let app = exercise_slice(D1Slice::new(ProjectionState::fixture()));

    assert_eq!(
        app.action_log(),
        [
            "composition:start",
            "composition:update:你好",
            "composition:commit:你好",
            "paste:第一行\\n第二行",
            "stream:start",
            "queue:你好第一行\\n第二行",
            "stream:cancel",
            "approval:allow-once",
            "history:row-120",
            "output:row-50001",
            "theme:ice-light:comfy",
            "focus:composer",
        ]
    );
    assert_eq!(app.projection_hash(), "e849d08e7c57e3a4");
}

#[test]
fn theme_tokens_are_generated_from_the_design_source() {
    let aurora = Skin::AuroraDark.tokens();
    let ice = Skin::IceLight.tokens();

    assert_eq!(aurora.bg_base, 0x0a1019);
    assert_eq!(aurora.fg_primary, 0xd4e6f1);
    assert_eq!(aurora.accent, 0x34bdd9);
    assert_eq!(aurora.gold, 0xe3ab44);
    assert_eq!(ice.bg_base, 0xf3f7fc);
    assert_eq!(ice.fg_primary, 0x16222e);
    assert_eq!(ice.accent, 0x2d65d2);
    assert_eq!(ice.gold, 0x5d6a8f);
}

#[test]
fn d1_slice_supports_denial_and_keyboard_only_focus_traversal() {
    let mut app = D1Slice::new(ProjectionState::fixture());

    app.approval.respond(ApprovalChoice::Deny);
    let visited: Vec<_> = D1Slice::REQUIRED_ROLES
        .iter()
        .map(|_| app.focus_next())
        .collect();

    assert_eq!(app.approval.last_choice(), Some(ApprovalChoice::Deny));
    assert_eq!(visited, D1Slice::REQUIRED_ROLES);
    assert_eq!(app.focused_role(), Some("new-output-count"));
    assert!(app.visible_focus());
}

#[test]
fn d1_slice_exposes_both_skins_and_all_density_choices() {
    let mut app = D1Slice::new(ProjectionState::fixture());

    for skin in [Skin::AuroraDark, Skin::IceLight] {
        for density in [Density::Compact, Density::Regular, Density::Comfy] {
            app.theme.select(skin, density);
            assert_eq!(app.theme.skin(), skin);
            assert_eq!(app.theme.density(), density);
        }
    }
}
