mod app;
mod canvas;
mod command_palette;
mod composer;
mod indicators;
mod input;
mod lane;
mod modal;
mod ops_screen;
mod panel;
mod preview;
mod render;
mod right_rail;
mod screen;
mod side_screen;
mod state;
mod statusbar;
mod terminal;
mod text;
mod theme;
mod topbar;
mod transcript;

pub(crate) use app::run_tui_with_theme;
pub(crate) use preview::{
    render_ansi_command_palette_preview_with_theme, render_ansi_idle_preview_with_theme,
    render_ansi_lane_preview_with_theme, render_ansi_live_turn_preview_with_theme,
    render_ansi_ops_preview_with_theme, render_ansi_preview_with_theme,
    render_ansi_side_preview_with_theme, render_command_palette_preview, render_idle_preview,
    render_lane_preview, render_live_turn_preview, render_ops_preview, render_preview,
    render_side_preview,
};
pub(crate) use screen::{SideScreen, run_side_tui_with_theme};

pub(crate) fn is_known_theme(name: &str) -> bool {
    theme::TuiTheme::is_known(name)
}

pub(crate) fn theme_names() -> &'static [&'static str] {
    theme::TuiTheme::names()
}
