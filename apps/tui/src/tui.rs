mod app;
mod canvas;
mod client;
// Deterministic design previews still exercise the richer selector surface;
// the CoreClient loop adopts those interactions incrementally.
#[allow(dead_code)]
mod command_palette;
mod composer;
mod indicators;
#[allow(dead_code)]
mod input;
#[cfg(test)]
#[allow(dead_code)]
mod lane;
mod lane_presenter;
#[allow(dead_code)]
mod modal;
mod ops_screen;
mod panel;
mod preview;
mod render;
mod right_rail;
#[cfg(test)]
#[allow(dead_code)]
mod screen;
mod side_screen;
// Task 3 removes the remaining legacy persistence/read helpers. They are not
// called from the production CoreClient loop in Tasks 1-2.
#[allow(dead_code)]
mod state;
mod statusbar;
#[allow(dead_code)]
mod terminal;
mod text;
#[allow(dead_code)]
mod theme;
mod topbar;
mod transcript;

pub use app::{TuiError, TuiOptions, run_tui};
pub use preview::{
    render_ansi_cjk_input_preview_with_theme, render_ansi_command_palette_preview_with_theme,
    render_ansi_idle_preview_with_theme, render_ansi_lane_preview_with_theme,
    render_ansi_lane_selector_preview_with_theme, render_ansi_live_turn_preview_with_theme,
    render_ansi_model_selector_preview_with_theme, render_ansi_ops_preview_with_theme,
    render_ansi_preview_with_theme, render_ansi_provider_detail_preview_with_theme,
    render_ansi_provider_selector_preview_with_theme, render_ansi_resize_preview_with_theme,
    render_ansi_setup_wizard_preview_with_theme, render_ansi_side_preview_with_theme,
    render_cjk_input_preview, render_command_palette_preview, render_idle_preview,
    render_lane_preview, render_lane_selector_preview, render_live_turn_preview,
    render_model_selector_preview, render_ops_preview, render_preview,
    render_provider_detail_preview, render_provider_selector_preview, render_resize_preview,
    render_setup_wizard_preview, render_side_preview,
};

pub fn is_known_theme(name: &str) -> bool {
    theme::TuiTheme::is_known(name)
}

pub fn theme_names() -> &'static [&'static str] {
    theme::TuiTheme::names()
}
