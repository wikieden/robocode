use viden_gui::{ComposerAction, ComposerDraft};

#[test]
fn cjk_composition_never_submits_before_composition_end() {
    let mut composer = ComposerDraft::default();
    composer.replace_text("你好");
    composer.begin_composition();

    assert_eq!(composer.handle_enter(false), ComposerAction::None);
    assert_eq!(composer.text, "你好");

    composer.end_composition();
    assert_eq!(
        composer.handle_enter(false),
        ComposerAction::Submit("你好".into())
    );
    assert!(composer.text.is_empty());
}

#[test]
fn shift_enter_and_multiline_paste_preserve_exact_text() {
    let mut composer = ComposerDraft::default();
    composer.replace_text("第一行\n第二行");
    assert_eq!(composer.handle_enter(true), ComposerAction::None);
    assert_eq!(composer.text, "第一行\n第二行\n");
}

#[test]
fn undo_restores_the_previous_multiline_draft() {
    let mut composer = ComposerDraft::default();
    composer.replace_text("alpha\nbeta");
    composer.replace_text("alpha\nbeta\ngamma");

    assert!(composer.undo());
    assert_eq!(composer.text, "alpha\nbeta");
    assert!(composer.undo());
    assert_eq!(composer.text, "");
    assert!(!composer.undo());
}

#[test]
fn empty_or_whitespace_only_enter_does_not_submit() {
    let mut composer = ComposerDraft::default();
    composer.replace_text("  \n");
    assert_eq!(composer.handle_enter(false), ComposerAction::None);
    assert_eq!(composer.text, "  \n");
}
