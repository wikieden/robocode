use std::{
    io::{self, Write},
    time::{Duration, Instant},
};

use crossterm::{
    SynchronizedUpdate, cursor,
    event::{DisableMouseCapture, EnableMouseCapture},
    execute, queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use viden_core::{ResolvedUiPreferences, TuiColorDepth, UiMotion};

use super::{
    command_palette::is_command_palette_visible,
    composer::{composer_cursor_position, should_render_welcome},
    modal::{approval_focus_cursor, has_pending_approval},
    render::{render_frame, render_ops_frame, render_side_frame},
    state::TuiState,
    statusbar::BOTTOM_BAR_HEIGHT,
    text::char_width,
    theme::TuiTheme,
};

pub(super) struct TerminalGuard {
    active: bool,
    theme: TuiTheme,
    motion: UiMotion,
    last_lines: Vec<String>,
    last_size: Option<(u16, u16)>,
    last_style_signature: Option<String>,
    last_full_redraw: Instant,
}

const MAIN_RIGHT_RAIL_WIDTH: usize = 38;
// Terminal emulators can lose alternate-screen contents after sleep, focus, or
// long idle periods; periodic full redraw keeps the dirty-row cache honest.
const FULL_REDRAW_INTERVAL: Duration = Duration::from_secs(5);

impl TerminalGuard {
    pub(super) fn enter_with_theme(theme_name: Option<&str>) -> Result<Self, String> {
        Self::enter(TuiTheme::from_name_or_env(theme_name), UiMotion::System)
    }

    pub(super) fn enter_with_preferences(
        preferences: &ResolvedUiPreferences,
        color_depth: TuiColorDepth,
    ) -> Result<Self, String> {
        Self::enter(
            TuiTheme::from_preferences(preferences).with_color_depth(color_depth),
            preferences.motion,
        )
    }

    fn enter(theme: TuiTheme, motion: UiMotion) -> Result<Self, String> {
        let mut stdout = io::stdout();
        terminal::enable_raw_mode().map_err(|err| err.to_string())?;
        if let Err(err) = execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            cursor_style(motion),
            cursor::Show
        ) {
            let _ = terminal::disable_raw_mode();
            return Err(err.to_string());
        }
        Ok(Self {
            active: true,
            theme,
            motion,
            last_lines: Vec::new(),
            last_size: None,
            last_style_signature: None,
            last_full_redraw: Instant::now(),
        })
    }

    #[cfg(test)]
    pub(super) fn test() -> Self {
        Self {
            active: false,
            theme: TuiTheme::named("aurora"),
            motion: UiMotion::System,
            last_lines: Vec::new(),
            last_size: None,
            last_style_signature: None,
            last_full_redraw: Instant::now(),
        }
    }

    pub(super) fn draw(&mut self, state: &TuiState) -> Result<(), String> {
        let (width, height) = terminal::size().unwrap_or((80, 24));
        let frame = render_frame(state, width, height);
        let cursor = approval_focus_cursor(state, width, height, MAIN_RIGHT_RAIL_WIDTH)
            .unwrap_or_else(|| composer_cursor_position(state, width, height, BOTTOM_BAR_HEIGHT));
        self.draw_frame(&frame, Some(cursor), style_signature(state, &self.theme))
    }

    pub(super) fn draw_side(&mut self, state: &TuiState) -> Result<(), String> {
        let (width, height) = terminal::size().unwrap_or((80, 24));
        let frame = render_side_frame(state, width, height);
        self.draw_frame(&frame, None, style_signature(state, &self.theme))
    }

    pub(super) fn draw_ops(&mut self, state: &TuiState) -> Result<(), String> {
        let (width, height) = terminal::size().unwrap_or((80, 24));
        let frame = render_ops_frame(state, width, height);
        self.draw_frame(&frame, None, style_signature(state, &self.theme))
    }

    fn draw_frame(
        &mut self,
        frame: &str,
        cursor_position: Option<(u16, u16)>,
        style_signature: String,
    ) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }
        let size = terminal::size().unwrap_or((80, 24));
        let lines = frame.lines().map(str::to_string).collect::<Vec<_>>();
        let now = Instant::now();
        let full_redraw = should_full_redraw(
            self.last_size,
            self.last_style_signature.as_deref(),
            size,
            &style_signature,
            now.duration_since(self.last_full_redraw),
        );
        let mut stdout = io::stdout();
        let update_result = stdout
            .sync_update(|stdout| -> io::Result<()> {
                if full_redraw {
                    queue!(stdout, cursor::MoveTo(0, 0), Clear(ClearType::All))?;
                }
                let dirty_rows = dirty_rows(&self.last_lines, &lines, full_redraw);
                for row in dirty_rows {
                    let line = lines.get(row).map(String::as_str).unwrap_or("");
                    queue!(
                        stdout,
                        cursor::MoveTo(0, row as u16),
                        Clear(ClearType::CurrentLine)
                    )?;
                    let mut drawn_width = 0usize;
                    for segment in line_segments(line, &self.theme) {
                        drawn_width += char_width(&segment.text);
                        queue!(
                            stdout,
                            SetForegroundColor(segment.foreground),
                            SetBackgroundColor(segment.background),
                            Print(segment.text)
                        )?;
                    }
                    let remaining = usize::from(size.0).saturating_sub(drawn_width);
                    if remaining > 0 {
                        queue!(
                            stdout,
                            SetForegroundColor(self.theme.text),
                            SetBackgroundColor(self.theme.background),
                            Print(" ".repeat(remaining))
                        )?;
                    }
                    queue!(stdout, ResetColor)?;
                }
                if let Some((column, row)) = cursor_position {
                    queue!(
                        stdout,
                        cursor::MoveTo(
                            column.min(size.0.saturating_sub(1)),
                            row.min(size.1.saturating_sub(1))
                        ),
                        cursor_style(self.motion),
                        cursor::Show
                    )?;
                } else {
                    queue!(stdout, cursor::Hide)?;
                }
                Ok(())
            })
            .map_err(|err| err.to_string())?;
        update_result.map_err(|err| err.to_string())?;
        self.last_lines = lines;
        self.last_size = Some(size);
        self.last_style_signature = Some(style_signature);
        if full_redraw {
            self.last_full_redraw = now;
        }
        stdout.flush().map_err(|err| err.to_string())
    }

    pub(super) fn cycle_theme(&mut self) -> &'static str {
        self.theme = self.theme.next();
        self.theme.name
    }

    pub(super) fn set_theme(&mut self, theme_name: &str) -> Result<&'static str, String> {
        if !TuiTheme::is_known(theme_name) {
            return Err(format!(
                "Unknown TUI theme `{theme_name}`. Available themes: {}",
                TuiTheme::names().join(", ")
            ));
        }
        self.theme = TuiTheme::named(theme_name);
        Ok(self.theme.name)
    }

    pub(super) fn theme_name(&self) -> &'static str {
        self.theme.name
    }

    pub(super) fn leave(&mut self) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        let mut stdout = io::stdout();
        execute!(
            stdout,
            cursor::SetCursorStyle::DefaultUserShape,
            cursor::Show,
            DisableMouseCapture,
            LeaveAlternateScreen
        )
        .map_err(|err| err.to_string())?;
        terminal::disable_raw_mode().map_err(|err| err.to_string())
    }
}

fn cursor_style(motion: UiMotion) -> cursor::SetCursorStyle {
    match motion {
        UiMotion::Full => cursor::SetCursorStyle::BlinkingBar,
        UiMotion::System | UiMotion::Reduced => cursor::SetCursorStyle::SteadyBar,
    }
}

fn dirty_rows(previous: &[String], next: &[String], full_redraw: bool) -> Vec<usize> {
    if full_redraw {
        return (0..next.len()).collect();
    }
    let height = previous.len().max(next.len());
    (0..height)
        .filter(|row| previous.get(*row) != next.get(*row))
        .collect()
}

fn should_full_redraw(
    last_size: Option<(u16, u16)>,
    last_style_signature: Option<&str>,
    next_size: (u16, u16),
    next_style_signature: &str,
    elapsed_since_full_redraw: Duration,
) -> bool {
    last_size != Some(next_size)
        || last_style_signature != Some(next_style_signature)
        || elapsed_since_full_redraw >= FULL_REDRAW_INTERVAL
}

fn style_signature(state: &TuiState, theme: &TuiTheme) -> String {
    format!(
        "{}|layout={}|approval={}|lane={}|palette={}",
        theme.name,
        if should_render_welcome(state) {
            "welcome"
        } else {
            "cockpit"
        },
        has_pending_approval(state),
        state.focused_lane.as_deref().unwrap_or(""),
        is_command_palette_visible(state)
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StyledSegment {
    foreground: Color,
    background: Color,
    text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SpanStyle {
    foreground: Color,
    background: Color,
}

fn line_segments(line: &str, theme: &TuiTheme) -> Vec<StyledSegment> {
    let base = color_for_line(line, theme);
    let background = background_for_line(line, theme);
    let spans = semantic_spans(line, theme);
    if spans.is_empty() {
        return protect_frame_glyph_segments(
            vec![StyledSegment {
                foreground: base,
                background,
                text: line.to_string(),
            }],
            theme,
        );
    }

    let mut segments = Vec::new();
    let mut cursor = 0;
    for (start, end, style) in spans {
        if start > cursor {
            segments.push(StyledSegment {
                foreground: base,
                background,
                text: line[cursor..start].to_string(),
            });
        }
        segments.push(StyledSegment {
            foreground: style.foreground,
            background: style.background,
            text: line[start..end].to_string(),
        });
        cursor = end;
    }
    if cursor < line.len() {
        segments.push(StyledSegment {
            foreground: base,
            background,
            text: line[cursor..].to_string(),
        });
    }
    protect_frame_glyph_segments(segments, theme)
}

fn protect_frame_glyph_segments(
    segments: Vec<StyledSegment>,
    theme: &TuiTheme,
) -> Vec<StyledSegment> {
    // Semantic highlighters may accidentally include the closing rail in a
    // value span; split frame glyphs back out so borders stay visually stable.
    let mut protected = Vec::with_capacity(segments.len());
    for segment in segments {
        if !segment.text.chars().any(is_frame_glyph) {
            protected.push(segment);
            continue;
        }

        let mut current: Option<StyledSegment> = None;
        for ch in segment.text.chars() {
            let foreground = if is_frame_glyph(ch) {
                theme.frame
            } else {
                segment.foreground
            };
            if let Some(current_segment) = &mut current {
                if current_segment.foreground == foreground
                    && current_segment.background == segment.background
                {
                    current_segment.text.push(ch);
                    continue;
                }
                protected.push(current.take().expect("segment exists"));
            }
            current = Some(StyledSegment {
                foreground,
                background: segment.background,
                text: ch.to_string(),
            });
        }
        if let Some(current_segment) = current {
            protected.push(current_segment);
        }
    }
    protected
}

fn background_for_line(line: &str, theme: &TuiTheme) -> Color {
    if line.contains(" INPUT  ")
        || line.contains("│ ›")
        || line.contains("MODE [")
        || line.contains("PERM [")
        || line.contains("ACTIONS:")
    {
        theme.surface
    } else {
        theme.background
    }
}

fn semantic_spans(line: &str, theme: &TuiTheme) -> Vec<(usize, usize, SpanStyle)> {
    let mut spans = Vec::new();
    collect_frame_glyph_spans(line, theme, &mut spans);

    // Provider Health rows are compact label/value metrics; using the global
    // token highlighter here makes words like REQUESTS inherit single-letter
    // diagnostic colors.
    if let Some(provider_spans) = provider_health_metric_spans(line, theme) {
        spans.extend(provider_spans);
        spans.sort_by_key(|(start, _, _)| *start);
        return non_overlapping_spans(spans);
    }

    collect_approval_field_spans(line, theme, &mut spans);
    collect_hud_field_spans(line, theme, &mut spans);
    collect_model_provider_suffix_spans(line, theme, &mut spans);
    collect_role_spans(line, theme, &mut spans);
    collect_path_spans(line, theme, &mut spans);
    collect_metric_spans(line, theme, &mut spans);
    collect_state_value_spans(line, theme, &mut spans);
    collect_bracket_spans(line, theme, &mut spans);
    collect_progress_spans(line, theme, &mut spans);
    collect_sparkline_spans(line, theme, &mut spans);
    spans.sort_by_key(|(start, _, _)| *start);
    non_overlapping_spans(spans)
}

fn collect_model_provider_suffix_spans(
    line: &str,
    theme: &TuiTheme,
    spans: &mut Vec<(usize, usize, SpanStyle)>,
) {
    const PROVIDERS: [&str; 9] = [
        "DeepSeek",
        "OpenRouter",
        "OpenAI",
        "Anthropic",
        "Fallback",
        "Volcano Engine",
        "Alibaba (China)",
        "Qwen",
        "Kimi",
    ];
    if !line.contains('│') || line.contains("PROVIDER CONFIG") {
        return;
    }
    for provider in PROVIDERS {
        let mut cursor = 0;
        while let Some(offset) = line[cursor..].find(provider) {
            let start = cursor + offset;
            let end = start + provider.len();
            let prefix = line[..start].trim_start_matches(['│', ' ', '●']);
            let is_suffix = !prefix.is_empty()
                && prefix.chars().last().is_some_and(|ch| {
                    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '/' | '.')
                });
            if is_suffix {
                spans.push((
                    start,
                    end,
                    SpanStyle {
                        foreground: theme.muted,
                        background: background_for_line(line, theme),
                    },
                ));
            }
            cursor = end;
        }
    }
}

fn collect_frame_glyph_spans(
    line: &str,
    theme: &TuiTheme,
    spans: &mut Vec<(usize, usize, SpanStyle)>,
) {
    let background = background_for_line(line, theme);
    let mut span_start = None;
    let mut span_end = 0;

    for (index, ch) in line.char_indices() {
        if is_frame_glyph(ch) {
            span_start.get_or_insert(index);
            span_end = index + ch.len_utf8();
        } else if let Some(start) = span_start.take() {
            spans.push((
                start,
                span_end,
                SpanStyle {
                    foreground: theme.frame,
                    background,
                },
            ));
        }
    }

    if let Some(start) = span_start {
        spans.push((
            start,
            span_end,
            SpanStyle {
                foreground: theme.frame,
                background,
            },
        ));
    }
}

fn is_frame_glyph(ch: char) -> bool {
    matches!(
        ch,
        '│' | '─' | '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼' | '┆'
    )
}

fn provider_health_metric_spans(
    line: &str,
    theme: &TuiTheme,
) -> Option<Vec<(usize, usize, SpanStyle)>> {
    const LABELS: [&str; 9] = [
        "STATUS",
        "REQUESTS",
        "LATENCY",
        "TELEMETRY",
        "TOKENS",
        "RATE",
        "COST",
        "EVENTS",
        "ERROR",
    ];

    let label = LABELS
        .iter()
        .find(|label| is_panel_metric_row(line, label))?;
    let label_start = line.find(label)?;
    let label_end = label_start + label.len();
    let value_start = line[label_end..]
        .find(|ch: char| !ch.is_whitespace())
        .map(|offset| label_end + offset);
    let row_end = line.rfind('│').unwrap_or(line.len());
    let value_end = line[..row_end].trim_end().len();
    let mut spans = vec![(
        label_start,
        label_end,
        SpanStyle {
            foreground: theme.title,
            background: background_for_line(line, theme),
        },
    )];

    if let Some(value_start) = value_start.filter(|start| *start < value_end) {
        let value_foreground = if *label == "ERROR" {
            theme.error
        } else {
            theme.text
        };
        spans.push((
            value_start,
            value_end,
            SpanStyle {
                foreground: value_foreground,
                background: background_for_line(line, theme),
            },
        ));
    }

    Some(spans)
}

fn is_panel_metric_row(line: &str, label: &str) -> bool {
    let Some(start) = line.find(label) else {
        return false;
    };
    let after_label = &line[start + label.len()..];
    let looks_like_row_label =
        after_label.starts_with("  ") || after_label.starts_with("\t") || after_label.is_empty();
    let prefix_is_panel_padding = line[..start]
        .chars()
        .all(|ch| ch == '│' || ch == ' ' || ch == '┆');
    let value = after_label.trim_start().trim_end_matches([' ', '│']);
    let looks_like_provider_metric = match label {
        "STATUS" => {
            value.starts_with("Configured")
                || value.starts_with("Healthy")
                || value.starts_with("Error")
                || value.starts_with("Offline")
        }
        "REQUESTS" => value.contains(" ok / ") && value.ends_with("err"),
        "LATENCY" => value.starts_with("last "),
        "TELEMETRY" => value.starts_with("awaiting "),
        "TOKENS" => value.starts_with("last "),
        "RATE" => value.ends_with("/s"),
        "COST" => value.starts_with('$'),
        "EVENTS" => value.contains(" ctx "),
        "ERROR" => !value.is_empty(),
        _ => false,
    };

    looks_like_row_label && prefix_is_panel_padding && looks_like_provider_metric
}

fn collect_approval_field_spans(
    line: &str,
    theme: &TuiTheme,
    spans: &mut Vec<(usize, usize, SpanStyle)>,
) {
    const LABELS: [&str; 10] = [
        "PATH",
        "SIZE",
        "ACTION",
        "SCOPE",
        "POLICY",
        "EFFECT",
        "DIFF PREVIEW",
        "PREVIEW",
        "DECISION",
        "APPROVAL REQUIRED",
    ];
    for label in LABELS {
        let mut cursor = 0;
        while let Some(offset) = line[cursor..].find(label) {
            let start = cursor + offset;
            let end = start + label.len();
            let foreground = match label {
                "EFFECT" | "APPROVAL REQUIRED" => theme.warning,
                "DECISION" => theme.title,
                "DIFF PREVIEW" | "PREVIEW" => theme.accent,
                _ => theme.title,
            };
            spans.push((
                start,
                end,
                SpanStyle {
                    foreground,
                    background: theme.surface,
                },
            ));
            cursor = end;
        }
    }
}

fn collect_hud_field_spans(
    line: &str,
    theme: &TuiTheme,
    spans: &mut Vec<(usize, usize, SpanStyle)>,
) {
    let is_chip_line = line.contains('[');
    const LABELS: [&str; 16] = [
        "STATUS",
        "INPUT",
        "COMMAND",
        "MODE ",
        "MODE:",
        "ACTIONS:",
        "PERM",
        "TARGET",
        "CACHE",
        "DIAG",
        "AUTO",
        "LINKED",
        "SYNC",
        "SES",
        "ENTER",
        "multiline",
    ];
    for label in LABELS {
        if is_chip_line && matches!(label, "PERM" | "AUTO") {
            continue;
        }
        let mut cursor = 0;
        while let Some(offset) = line[cursor..].find(label) {
            let start = cursor + offset;
            let end = start + label.len();
            let foreground = match label {
                "CACHE" | "DIAG" | "LINKED" => theme.success,
                "AUTO" => theme.warning,
                _ => theme.accent,
            };
            spans.push((
                start,
                end,
                SpanStyle {
                    foreground,
                    background: background_for_line(line, theme),
                },
            ));
            cursor = end;
        }
    }
}

fn collect_role_spans(line: &str, theme: &TuiTheme, spans: &mut Vec<(usize, usize, SpanStyle)>) {
    const ROLES: [(&str, RoleTone); 10] = [
        ("USER", RoleTone::Accent),
        ("ASSISTANT", RoleTone::Success),
        ("TOOL CALL", RoleTone::Warning),
        ("TOOL RESULT", RoleTone::Success),
        ("CODEX TERM", RoleTone::Accent),
        ("CLAUDE TERM", RoleTone::Accent),
        ("SHELL TERM", RoleTone::Muted),
        ("codex", RoleTone::Accent),
        ("claude", RoleTone::Accent),
        ("shell", RoleTone::Muted),
    ];
    for (term, tone) in ROLES {
        collect_literal_spans(
            line,
            term,
            role_color(tone, theme),
            background_for_line(line, theme),
            spans,
        );
    }
}

fn collect_path_spans(line: &str, theme: &TuiTheme, spans: &mut Vec<(usize, usize, SpanStyle)>) {
    for token in line.split_whitespace() {
        let trimmed = token
            .trim_matches(|ch: char| matches!(ch, ',' | ';' | ':' | ')' | '(' | '[' | ']' | '`'));
        if !(trimmed.contains('/')
            || trimmed.ends_with(".rs")
            || trimmed.ends_with(".toml")
            || trimmed.ends_with(".md"))
        {
            continue;
        }
        if let Some(start) = line.find(trimmed) {
            spans.push((
                start,
                start + trimmed.len(),
                SpanStyle {
                    foreground: theme.accent,
                    background: background_for_line(line, theme),
                },
            ));
        }
    }
}

fn collect_metric_spans(line: &str, theme: &TuiTheme, spans: &mut Vec<(usize, usize, SpanStyle)>) {
    const METRICS: [&str; 37] = [
        "LATENCY", "RATE", "TPS", "CONTEXT", "TOKENS", "COST", "TIME", "LANES", "SCREEN", "FILES",
        "LINES", "LANG", "BUILD", "RLS", "GATE", "RISK", "ROUTE", "PTY", "PID", "TASK", "TAIL",
        "CMD", "JOB", "WATCH", "MUX", "CONTROL", "TERMINAL", "RUNTIME", "VERIFY", "STATE", "SYNC",
        "LINK", "SES", "TOK", "LANE", "PRESSURE", "THEME",
    ];
    for label in METRICS {
        collect_literal_spans(
            line,
            label,
            theme.title,
            background_for_line(line, theme),
            spans,
        );
    }
}

fn collect_state_value_spans(
    line: &str,
    theme: &TuiTheme,
    spans: &mut Vec<(usize, usize, SpanStyle)>,
) {
    const VALUES: [(&str, RoleTone); 15] = [
        ("approval-ready", RoleTone::Warning),
        ("gate-wait", RoleTone::Warning),
        ("lanes-live", RoleTone::Accent),
        ("armed", RoleTone::Accent),
        ("main→gate", RoleTone::Warning),
        ("main→s1", RoleTone::Accent),
        ("gate", RoleTone::Warning),
        ("s1", RoleTone::Accent),
        ("CONNECTED", RoleTone::Success),
        ("Configured", RoleTone::Success),
        ("hit", RoleTone::Success),
        ("ok", RoleTone::Success),
        ("medium", RoleTone::Warning),
        ("main", RoleTone::Success),
        ("Code", RoleTone::Accent),
    ];
    for (term, tone) in VALUES {
        collect_literal_spans(
            line,
            term,
            role_color(tone, theme),
            background_for_line(line, theme),
            spans,
        );
    }
}

#[derive(Debug, Clone, Copy)]
enum RoleTone {
    Accent,
    Success,
    Warning,
    Muted,
}

fn role_color(tone: RoleTone, theme: &TuiTheme) -> Color {
    match tone {
        RoleTone::Accent => theme.accent,
        RoleTone::Success => theme.success,
        RoleTone::Warning => theme.warning,
        RoleTone::Muted => theme.muted,
    }
}

fn collect_literal_spans(
    line: &str,
    term: &str,
    foreground: Color,
    background: Color,
    spans: &mut Vec<(usize, usize, SpanStyle)>,
) {
    let mut cursor = 0;
    while let Some(offset) = line[cursor..].find(term) {
        let start = cursor + offset;
        let end = start + term.len();
        spans.push((
            start,
            end,
            SpanStyle {
                foreground,
                background,
            },
        ));
        cursor = end;
    }
}

fn non_overlapping_spans(spans: Vec<(usize, usize, SpanStyle)>) -> Vec<(usize, usize, SpanStyle)> {
    let mut filtered = Vec::with_capacity(spans.len());
    let mut cursor = 0;
    for (start, end, style) in spans {
        if start < cursor {
            continue;
        }
        cursor = end;
        filtered.push((start, end, style));
    }
    filtered
}

fn collect_bracket_spans(line: &str, theme: &TuiTheme, spans: &mut Vec<(usize, usize, SpanStyle)>) {
    let mut cursor = 0;
    while let Some(offset) = line[cursor..].find('[') {
        let start = cursor + offset;
        let Some(end_offset) = line[start..].find(']') else {
            break;
        };
        let end = start + end_offset + 1;
        let text = &line[start..end];
        let foreground = if text.contains("Deny") {
            theme.error
        } else if text.contains("Approve") {
            theme.success
        } else if text.contains("Diff") {
            theme.accent
        } else if text.contains("all writes") {
            theme.warning
        } else if is_shortcut_chip(text) {
            shortcut_chip_color(text, theme)
        } else if text.contains("done") {
            theme.success
        } else if text.contains("pending") || text.contains("waiting") {
            theme.warning
        } else if text.contains("stream") || text.contains("input") || text.contains("event") {
            theme.accent
        } else if text.contains("PERMISSIONS")
            || text.contains("PERM")
            || text.contains("Suggest")
            || matches!(
                text,
                "[Ask]" | "[AutoEdit]" | "[ReadOnly]" | "[Full]" | "[Auto Edit]" | "[Full Access]"
            )
        {
            theme.warning
        } else if text.contains("in_prog") {
            theme.success
        } else if text.contains("idle") {
            theme.muted
        } else if text.contains("write_file")
            || text.contains("Rs")
            || text.contains("Tm")
            || text.contains("Md")
            || text.contains("Fs")
            || text.contains("MODEL")
            || text.contains("PROVIDER")
            || text.contains("SESSION")
            || text.contains("LANES")
            || text.contains("CTX")
            || text.contains("GIT")
            || text.contains("PERMISSIONS")
            || text.contains("LSP")
        {
            theme.accent
        } else {
            theme.title
        };
        let background = if text.contains("Deny")
            || text.contains("Approve")
            || text.contains("Diff")
            || text.contains("all writes")
        {
            theme.overlay
        } else {
            theme.chip
        };
        spans.push((
            start,
            end,
            SpanStyle {
                foreground,
                background,
            },
        ));
        cursor = end;
    }
}

fn is_shortcut_chip(text: &str) -> bool {
    text.starts_with("[^") || text == "[? Help]"
}

fn shortcut_chip_color(text: &str, theme: &TuiTheme) -> Color {
    if text.contains("Theme") {
        theme.warning
    } else if text.contains("Lane")
        || text.contains("Route")
        || text.contains("Send")
        || text.contains("Regenerate")
        || text.contains("New Task")
    {
        theme.accent
    } else if text.contains("Help") {
        theme.muted
    } else {
        theme.title
    }
}

fn collect_progress_spans(
    line: &str,
    theme: &TuiTheme,
    spans: &mut Vec<(usize, usize, SpanStyle)>,
) {
    let mut cursor = 0;
    while let Some(offset) = line[cursor..].find('▓') {
        let start = cursor + offset;
        let end = line[start..]
            .char_indices()
            .find_map(|(index, ch)| (!matches!(ch, '▓' | '░')).then_some(start + index))
            .unwrap_or(line.len());
        spans.push((
            start,
            end,
            SpanStyle {
                foreground: theme.success,
                background: theme.chip,
            },
        ));
        cursor = end;
    }
}

fn collect_sparkline_spans(
    line: &str,
    theme: &TuiTheme,
    spans: &mut Vec<(usize, usize, SpanStyle)>,
) {
    let mut start = None;
    let mut last_end = 0;
    for (index, ch) in line.char_indices() {
        if matches!(ch, '▁' | '▂' | '▃' | '▄' | '▅' | '▆' | '▇' | '█') {
            start.get_or_insert(index);
            last_end = index + ch.len_utf8();
        } else if let Some(span_start) = start.take() {
            spans.push((
                span_start,
                last_end,
                SpanStyle {
                    foreground: theme.accent,
                    background: theme.chip,
                },
            ));
        }
    }
    if let Some(span_start) = start {
        spans.push((
            span_start,
            last_end,
            SpanStyle {
                foreground: theme.accent,
                background: theme.chip,
            },
        ));
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.leave();
    }
}

pub(super) fn render_ansi_preview_with_theme(frame: &str, theme_name: Option<&str>) -> String {
    let theme = TuiTheme::from_name_or_env(theme_name);
    let mut output = String::new();
    for line in frame.lines() {
        for segment in line_segments(line, &theme) {
            output.push_str(&ansi_color(segment.foreground, true));
            output.push_str(&ansi_color(segment.background, false));
            output.push_str(&segment.text);
            output.push_str("\x1b[0m");
        }
        output.push('\n');
    }
    output
}

fn ansi_color(color: Color, foreground: bool) -> String {
    let prefix = if foreground { 38 } else { 48 };
    match color {
        Color::Rgb { r, g, b } => format!("\x1b[{prefix};2;{r};{g};{b}m"),
        Color::Black => format!("\x1b[{}m", if foreground { 30 } else { 40 }),
        Color::DarkGrey => format!("\x1b[{}m", if foreground { 90 } else { 100 }),
        Color::Red | Color::DarkRed => format!("\x1b[{}m", if foreground { 31 } else { 41 }),
        Color::Green | Color::DarkGreen => format!("\x1b[{}m", if foreground { 32 } else { 42 }),
        Color::Yellow | Color::DarkYellow => format!("\x1b[{}m", if foreground { 33 } else { 43 }),
        Color::Blue | Color::DarkBlue => format!("\x1b[{}m", if foreground { 34 } else { 44 }),
        Color::Magenta | Color::DarkMagenta => {
            format!("\x1b[{}m", if foreground { 35 } else { 45 })
        }
        Color::Cyan | Color::DarkCyan => format!("\x1b[{}m", if foreground { 36 } else { 46 }),
        Color::White | Color::Grey => format!("\x1b[{}m", if foreground { 37 } else { 47 }),
        Color::AnsiValue(value) => format!("\x1b[{prefix};5;{value}m"),
        Color::Reset => "\x1b[0m".to_string(),
    }
}

fn color_for_line(line: &str, theme: &TuiTheme) -> Color {
    let trimmed = line.trim_start();
    if line.contains("DECISION") {
        theme.text
    } else if trimmed.starts_with("E ")
        || ((line.contains("[Deny") || line.contains("✕ Deny")) && !line.contains("Approve"))
    {
        theme.error
    } else if line.contains(" INPUT  ") || line.contains("│ ›") {
        theme.title
    } else if line.contains("mutation review")
        || line.contains("! approval required")
        || line.contains("PERM")
        || line.contains("[PERM ")
        || line.contains("[waiting]")
    {
        theme.warning
    } else if line.contains("CONNECTED")
        || line.contains("Healthy")
        || line.contains("Approve")
        || line.contains("✓ Approve")
        || line.contains("ASSISTANT")
        || trimmed.starts_with("✓")
    {
        theme.success
    } else if is_panel_title(line) {
        theme.title
    } else if is_frame_line(line) {
        theme.frame
    } else if line.contains("Viden")
        || line.contains("PROVIDER")
        || line.contains("MODEL")
        || line.contains("SIDE-1")
        || line.contains("SIDE-2")
    {
        theme.title
    } else if line.contains("TOOL CALL")
        || line.contains("TOOL RESULT")
        || line.contains("tool write_file")
        || line.contains("review gate")
        || line.contains("[write_file]")
        || line.contains("codex")
        || line.contains("claude")
        || line.contains("[in_prog]")
        || line.contains("TPS")
        || line.contains("CONTEXT")
        || line.contains("BUILD")
        || line.contains("RLS")
    {
        theme.accent
    } else if line.contains("USER") || trimmed.starts_with('◇') {
        theme.text
    } else if trimmed.starts_with('·')
        || trimmed.starts_with('░')
        || line.contains("pending")
        || line.contains("[idle]")
        || line.contains("next")
        || line.contains("RECENT FILES")
        || line.contains("Press ?")
    {
        theme.muted
    } else if trimmed.starts_with('$') || trimmed.starts_with('>') {
        theme.accent
    } else {
        theme.text
    }
}

fn is_frame_line(line: &str) -> bool {
    line.starts_with('+') || line.starts_with('┌') || line.starts_with('├') || line.starts_with('└')
}

fn is_panel_title(line: &str) -> bool {
    line.contains(" TRANSCRIPT ")
        || line.contains(" WORKSPACE ")
        || line.contains(" ACTIVE TASKS ")
        || line.contains(" TERMINAL LANES")
        || line.contains(" SCREENS ")
        || line.contains(" LSP DIAGNOSTICS ")
        || line.contains(" PROVIDER HEALTH ")
        || line.contains(" RECENT FILES ")
        || line.contains(" APPROVAL ")
        || line.contains(" AGENT LANES ")
        || line.contains(" LIVE OUTPUT ")
        || line.contains(" SIDE STATUS ")
        || line.contains(" LSP / BUILD ")
        || line.contains(" RECENT EVENTS ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_approval_and_actions_by_semantics() {
        let theme = TuiTheme::aurora_cyan();

        assert_eq!(
            color_for_line("│ APPROVAL REQUIRED: write_file", &theme),
            theme.title
        );
        assert_eq!(color_for_line("│ [✕ Deny n]", &theme), theme.error);
        assert_eq!(color_for_line("│ [✓ Approve y]", &theme), theme.success);
        assert_eq!(
            color_for_line("│ DECISION [✕ Deny n] [✓ Approve y]", &theme),
            theme.text
        );
        let segments = line_segments("│ APPROVAL REQUIRED: write_file │", &theme);
        assert!(
            segments
                .iter()
                .any(|segment| segment.text == "APPROVAL REQUIRED"
                    && segment.foreground == theme.warning)
        );
    }

    #[test]
    fn colors_tool_and_panel_lines_distinctly() {
        let theme = TuiTheme::aurora_cyan();

        assert_eq!(color_for_line("│   ⚙  TOOL CALL", &theme), theme.accent);
        assert_eq!(
            color_for_line("┌ TRANSCRIPT ───────────────── live session ┐", &theme),
            theme.title
        );
    }

    #[test]
    fn splits_inline_chips_into_semantic_segments() {
        let theme = TuiTheme::aurora_cyan();
        let segments = line_segments("│ [MODEL test-local] [WORK Build] [PERM Ask] │", &theme);

        assert!(
            segments
                .iter()
                .any(|segment| segment.text == "[MODEL test-local]"
                    && segment.foreground == theme.accent
                    && segment.background == theme.chip)
        );
        assert!(segments.iter().any(|segment| segment.text == "[PERM Ask]"
            && segment.foreground == theme.warning
            && segment.background == theme.chip));
    }

    #[test]
    fn splits_progress_bars_into_success_segments() {
        let theme = TuiTheme::aurora_cyan();
        let segments = line_segments("│ codex ▓▓▓░░ patched failing tests │", &theme);

        assert!(segments.iter().any(|segment| segment.text == "▓▓▓░░"
            && segment.foreground == theme.success
            && segment.background == theme.chip));
    }

    #[test]
    fn colors_side_screen_titles_and_lane_badges() {
        let theme = TuiTheme::aurora_cyan();

        assert_eq!(
            color_for_line("│ Viden  SIDE-1  [SESSION c4f2b7e] │", &theme),
            theme.title
        );
        assert_eq!(
            color_for_line("┌ LSP / BUILD ───────────────────────────── 0 ┐", &theme),
            theme.title
        );

        let segments = line_segments("│ ● L1 codex [in_prog] target main │", &theme);
        assert!(segments.iter().any(|segment| segment.text == "[in_prog]"
            && segment.foreground == theme.success
            && segment.background == theme.chip));

        let segments = line_segments("│ ◐ L2 claude [pending] target side-1 │", &theme);
        assert!(segments.iter().any(|segment| segment.text == "[pending]"
            && segment.foreground == theme.warning
            && segment.background == theme.chip));
    }

    #[test]
    fn colors_transcript_status_badges_by_state() {
        let theme = TuiTheme::aurora_cyan();
        let segments = line_segments(
            "│   ✣  ASSISTANT   [stream]  ⚙ TOOL CALL [pending] ✓ TOOL RESULT [done] │",
            &theme,
        );

        assert!(segments.iter().any(|segment| segment.text == "[stream]"
            && segment.foreground == theme.accent
            && segment.background == theme.chip));
        assert!(segments.iter().any(|segment| segment.text == "[pending]"
            && segment.foreground == theme.warning
            && segment.background == theme.chip));
        assert!(segments.iter().any(|segment| segment.text == "[done]"
            && segment.foreground == theme.success
            && segment.background == theme.chip));
    }

    #[test]
    fn uses_base_background_for_approval_modal_rows_to_avoid_full_width_bands() {
        let theme = TuiTheme::aurora_cyan();
        let segments = line_segments("│ [Deny] [Diff] [Approve] │", &theme);

        assert!(
            segments
                .iter()
                .filter(|segment| !segment.text.starts_with('['))
                .all(|segment| segment.background == theme.background)
        );
        assert!(
            segments
                .iter()
                .any(|segment| segment.text == "[Approve]" && segment.foreground == theme.success)
        );
    }

    #[test]
    fn uses_base_background_for_regular_panel_content() {
        let theme = TuiTheme::aurora_cyan();
        let segments = line_segments("│      ordinary transcript row      │", &theme);

        assert!(
            segments
                .iter()
                .all(|segment| segment.background == theme.background)
        );
    }

    #[test]
    fn uses_base_background_for_panel_title_rows_to_avoid_full_width_bands() {
        let theme = TuiTheme::aurora_cyan();
        let segments = line_segments(
            "│                                                                 │ ┌ RECENT FILES ─ tail ┐",
            &theme,
        );

        assert!(
            segments
                .iter()
                .all(|segment| segment.background == theme.background),
            "{segments:?}"
        );
    }

    #[test]
    fn keeps_panel_frame_glyphs_on_stable_frame_color() {
        let theme = TuiTheme::aurora_cyan();
        for line in [
            "│ STATUS     Configured              │",
            "│ next watch side-1 tail failure      │",
            "┌ SIDE STATUS ───────────────── tail ┐",
            "│ ACTIONS: [^J Send] [? Help]        │",
            "│ ● CONNECTED ┆ SESSION ses~5000 ┆ HELP ? │",
        ] {
            let segments = line_segments(line, &theme);
            assert!(
                frame_segments_use_frame_color(&segments, &theme),
                "{line}: {segments:?}"
            );
        }
    }

    #[test]
    fn forces_periodic_full_redraw_even_when_frame_cache_matches() {
        assert!(!should_full_redraw(
            Some((160, 48)),
            Some("aurora-cyan|layout=cockpit"),
            (160, 48),
            "aurora-cyan|layout=cockpit",
            FULL_REDRAW_INTERVAL - Duration::from_millis(1)
        ));
        assert!(should_full_redraw(
            Some((160, 48)),
            Some("aurora-cyan|layout=cockpit"),
            (160, 48),
            "aurora-cyan|layout=cockpit",
            FULL_REDRAW_INTERVAL
        ));
    }

    #[test]
    fn full_redraw_still_triggers_on_resize_or_style_change() {
        assert!(should_full_redraw(
            Some((160, 48)),
            Some("aurora-cyan|layout=cockpit"),
            (120, 48),
            "aurora-cyan|layout=cockpit",
            Duration::ZERO
        ));
        assert!(should_full_redraw(
            Some((160, 48)),
            Some("aurora-cyan|layout=cockpit"),
            (160, 48),
            "aurora-cyan|layout=welcome",
            Duration::ZERO
        ));
    }

    #[test]
    fn keeps_preview_frame_glyphs_on_stable_frame_color() {
        let theme = TuiTheme::aurora_cyan();
        let previews = [
            crate::tui::render_preview("deepseek", "deepseek-v4-flash"),
            crate::tui::render_idle_preview("deepseek", "deepseek-v4-flash"),
            crate::tui::render_lane_preview("deepseek", "deepseek-v4-flash"),
            crate::tui::render_side_preview("deepseek", "deepseek-v4-flash"),
            crate::tui::render_ops_preview("deepseek", "deepseek-v4-flash"),
        ];

        for preview in previews {
            for line in preview.lines() {
                let segments = line_segments(line, &theme);
                assert!(
                    frame_segments_use_frame_color(&segments, &theme),
                    "{line}: {segments:?}"
                );
            }
        }
    }

    fn frame_segments_use_frame_color(segments: &[StyledSegment], theme: &TuiTheme) -> bool {
        segments
            .iter()
            .filter(|segment| segment.text.chars().any(is_frame_glyph))
            .all(|segment| segment.foreground == theme.frame)
    }

    #[test]
    fn uses_base_background_for_approval_modal_rows_mixed_with_rail() {
        let theme = TuiTheme::aurora_cyan();
        let segments = line_segments(
            "│     │ APPROVAL REQUIRED: shell │        │ │ ● Implement load_config [review] │",
            &theme,
        );

        assert!(
            segments
                .iter()
                .filter(|segment| !segment.text.contains("APPROVAL REQUIRED"))
                .all(|segment| segment.background == theme.background
                    || segment.background == theme.chip),
            "{segments:?}"
        );
    }

    #[test]
    fn highlights_approval_field_labels() {
        let theme = TuiTheme::aurora_cyan();
        let segments = line_segments(
            "│ PATH    src/config.rs              SIZE   +48 lines │",
            &theme,
        );

        assert!(segments.iter().any(|segment| segment.text == "PATH"
            && segment.foreground == theme.title
            && segment.background == theme.surface));
        assert!(segments.iter().any(|segment| segment.text == "SIZE"
            && segment.foreground == theme.title
            && segment.background == theme.surface));

        let segments = line_segments(
            "│ POLICY  Ask level              EFFECT Mutating command │",
            &theme,
        );
        assert!(segments.iter().any(|segment| segment.text == "EFFECT"
            && segment.foreground == theme.warning
            && segment.background == theme.surface));
    }

    #[test]
    fn highlights_provider_sparklines() {
        let theme = TuiTheme::aurora_cyan();
        let segments = line_segments("│ LATENCY      412ms  ▁▃▆▇▅▃ │", &theme);

        assert!(segments.iter().any(|segment| segment.text == "▁▃▆▇▅▃"
            && segment.foreground == theme.accent
            && segment.background == theme.chip));
    }

    #[test]
    fn provider_health_rows_use_stable_label_value_colors() {
        let theme = TuiTheme::aurora_cyan();
        let segments = line_segments("│ STATUS     Configured              │", &theme);

        assert!(
            segments
                .iter()
                .any(|segment| segment.text == "STATUS" && segment.foreground == theme.title),
            "{segments:?}"
        );
        assert!(
            segments
                .iter()
                .any(|segment| segment.text == "Configured" && segment.foreground == theme.text),
            "{segments:?}"
        );

        let segments = line_segments("│ REQUESTS   0 ok / 0 err            │", &theme);
        assert!(
            segments
                .iter()
                .any(|segment| segment.text == "0 ok / 0 err" && segment.foreground == theme.text),
            "{segments:?}"
        );
        assert!(
            !segments
                .iter()
                .any(|segment| segment.text == "ok" && segment.foreground == theme.success),
            "{segments:?}"
        );
    }

    #[test]
    fn semantic_highlighting_does_not_color_single_letters_inside_words() {
        let theme = TuiTheme::aurora_cyan();
        let segments = line_segments("│ next watch side-1 tail failure      │", &theme);

        assert!(
            !segments
                .iter()
                .any(|segment| matches!(segment.text.as_str(), "E" | "W")
                    && segment.foreground == theme.title),
            "{segments:?}"
        );
    }

    #[test]
    fn highlights_command_deck_and_status_labels() {
        let theme = TuiTheme::aurora_cyan();
        let segments = line_segments(
            "│ MODE [Build] [Plan]  PERM [Ask] [AutoEdit] [ReadOnly] [Full] │",
            &theme,
        );

        assert!(segments.iter().any(|segment| segment.text == "MODE "
            && segment.foreground == theme.accent
            && segment.background == theme.surface));
        assert!(segments.iter().any(|segment| segment.text == "[Ask]"
            && segment.foreground == theme.warning
            && segment.background == theme.chip));

        let segments = line_segments("│ CACHE hit 73% ┆ DIAG ok │", &theme);
        assert!(
            segments
                .iter()
                .any(|segment| segment.text == "CACHE" && segment.foreground == theme.success)
        );
        assert!(
            segments
                .iter()
                .any(|segment| segment.text == "DIAG" && segment.foreground == theme.success)
        );
        assert!(
            segments
                .iter()
                .any(|segment| segment.text == "hit" && segment.foreground == theme.success)
        );
        assert!(
            segments
                .iter()
                .any(|segment| segment.text == "ok" && segment.foreground == theme.success)
        );
    }

    #[test]
    fn highlights_composer_shortcuts_by_action_semantics() {
        let theme = TuiTheme::aurora_cyan();
        let segments = line_segments(
            "│ ACTIONS: [^J Send] [^K Clr] [^R Regenerate] [^N New Task] [? Help] │",
            &theme,
        );

        assert!(segments.iter().any(|segment| segment.text == "[^J Send]"
            && segment.foreground == theme.accent
            && segment.background == theme.chip));
        assert!(segments.iter().any(|segment| segment.text == "[^K Clr]"
            && segment.foreground == theme.title
            && segment.background == theme.chip));
        assert!(
            segments
                .iter()
                .any(|segment| segment.text == "[^R Regenerate]"
                    && segment.foreground == theme.accent
                    && segment.background == theme.chip)
        );
        assert!(
            segments
                .iter()
                .any(|segment| segment.text == "[^N New Task]"
                    && segment.foreground == theme.accent
                    && segment.background == theme.chip)
        );
        assert!(segments.iter().any(|segment| segment.text == "[? Help]"
            && segment.foreground == theme.muted
            && segment.background == theme.chip));
    }

    #[test]
    fn highlights_roles_paths_and_operational_metrics() {
        let theme = TuiTheme::aurora_cyan();
        let segments = line_segments(
            "│ ASSISTANT wrote src/config.rs  LATENCY 412ms  TPS 29.3 │",
            &theme,
        );

        assert!(
            segments
                .iter()
                .any(|segment| segment.text == "ASSISTANT" && segment.foreground == theme.success)
        );
        assert!(
            segments.iter().any(
                |segment| segment.text == "src/config.rs" && segment.foreground == theme.accent
            )
        );
        assert!(
            segments
                .iter()
                .any(|segment| segment.text == "LATENCY" && segment.foreground == theme.title)
        );
        assert!(
            segments
                .iter()
                .any(|segment| segment.text == "TPS" && segment.foreground == theme.title)
        );
    }

    #[test]
    fn ansi_preview_emits_truecolor_sequences() {
        let preview = render_ansi_preview_with_theme("│ [MODEL test-local] │", None);

        assert!(preview.contains("\x1b[38;2;"));
        assert!(preview.contains("\x1b[48;2;"));
        assert!(preview.contains("[MODEL test-local]"));
        assert!(preview.ends_with('\n'));
    }

    #[test]
    fn ansi_preview_uses_explicit_theme_when_provided() {
        let frame = "│ [MODEL test-local] [WORK Build] [PERM Ask] │";

        let aurora = render_ansi_preview_with_theme(frame, Some("aurora-cyan"));
        let ember = render_ansi_preview_with_theme(frame, Some("ember-gold"));

        assert_ne!(aurora, ember);
        assert!(ember.contains("\x1b[38;2;255;176;64m"));
    }

    #[test]
    fn dirty_rows_only_reports_changed_lines_after_first_draw() {
        let previous = vec![
            "top".to_string(),
            "input old".to_string(),
            "status".to_string(),
        ];
        let next = vec![
            "top".to_string(),
            "input new".to_string(),
            "status".to_string(),
        ];

        assert_eq!(dirty_rows(&previous, &next, false), vec![1]);
        assert_eq!(dirty_rows(&previous, &next, true), vec![0, 1, 2]);
    }

    #[test]
    fn dirty_rows_clears_removed_trailing_lines() {
        let previous = vec!["top".to_string(), "stale".to_string()];
        let next = vec!["top".to_string()];

        assert_eq!(dirty_rows(&previous, &next, false), vec![1]);
    }
}
