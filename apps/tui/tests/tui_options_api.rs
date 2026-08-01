use viden_core::TuiColorDepth;
use viden_tui::TuiOptions;

#[test]
fn downstream_callers_can_still_use_the_public_struct_literal() {
    let options = TuiOptions {
        startup_summary: "compatibility".to_string(),
        startup_check: true,
        color_depth: TuiColorDepth::Ansi16,
    };

    assert_eq!(options.color_depth, TuiColorDepth::Ansi16);
}
