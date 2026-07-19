use std::{cell::RefCell, rc::Rc};

use crate::{
    approval::ApprovalDockModel, composer::ComposerModel, theme::ThemeModel,
    transcript::TranscriptModel,
};

#[derive(Clone, Default)]
pub(crate) struct ActionRecorder(Rc<RefCell<Vec<String>>>);

impl ActionRecorder {
    pub(crate) fn record(&self, action: impl Into<String>) {
        self.0.borrow_mut().push(action.into());
    }

    fn snapshot(&self) -> Vec<String> {
        self.0.borrow().clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionState {
    pub project_id: String,
    pub lane_id: String,
    pub session_id: String,
    pub task_id: String,
}

impl ProjectionState {
    pub fn fixture() -> Self {
        Self {
            project_id: "project-1".into(),
            lane_id: "lane-1".into(),
            session_id: "session-1".into(),
            task_id: "task-1".into(),
        }
    }
}

pub struct D1Slice {
    pub composer: ComposerModel,
    pub approval: ApprovalDockModel,
    pub transcript: TranscriptModel,
    pub theme: ThemeModel,
    projection: ProjectionState,
    recorder: ActionRecorder,
    focused_role: Option<String>,
    visible_focus: bool,
    streaming: bool,
}

impl D1Slice {
    pub const REQUIRED_ROLES: [&'static str; 7] = [
        "composer",
        "tool-row",
        "approval-dock",
        "queue-action",
        "cancel-action",
        "history-viewport",
        "new-output-count",
    ];

    pub fn new(projection: ProjectionState) -> Self {
        let recorder = ActionRecorder::default();
        Self {
            composer: ComposerModel::new(recorder.clone()),
            approval: ApprovalDockModel::new(recorder.clone()),
            transcript: TranscriptModel::new(recorder.clone()),
            theme: ThemeModel::new(recorder.clone()),
            projection,
            recorder,
            focused_role: None,
            visible_focus: false,
            streaming: false,
        }
    }

    pub fn start_stream(&mut self) {
        self.streaming = true;
        self.recorder.record("stream:start");
    }

    pub fn queue_current_draft(&mut self) {
        assert!(self.streaming, "queue is available only while streaming");
        self.recorder.record(format!(
            "queue:{}",
            self.composer.draft().replace('\n', "\\n")
        ));
    }

    pub fn cancel_stream(&mut self) {
        if self.streaming {
            self.streaming = false;
            self.recorder.record("stream:cancel");
        }
    }

    pub fn focus(&mut self, role: &str) {
        assert!(Self::REQUIRED_ROLES.contains(&role), "unknown role: {role}");
        self.focused_role = Some(role.to_owned());
        self.visible_focus = true;
        self.recorder.record(format!("focus:{role}"));
    }

    pub fn focus_next(&mut self) -> &'static str {
        let current_index = self.focused_role.as_deref().and_then(|focused| {
            Self::REQUIRED_ROLES
                .iter()
                .position(|role| *role == focused)
        });
        let next_index = current_index.map_or(0, |index| (index + 1) % Self::REQUIRED_ROLES.len());
        let role = Self::REQUIRED_ROLES[next_index];
        self.focus(role);
        role
    }

    pub fn focused_role(&self) -> Option<&str> {
        self.focused_role.as_deref()
    }

    pub fn visible_focus(&self) -> bool {
        self.visible_focus
    }

    pub fn exposed_roles(&self) -> [&'static str; 7] {
        Self::REQUIRED_ROLES
    }

    pub fn action_log(&self) -> Vec<String> {
        self.recorder.snapshot()
    }

    pub fn projection_hash(&self) -> String {
        let canonical = format!(
            "{}|{}|{}|{}\n{}",
            self.projection.project_id,
            self.projection.lane_id,
            self.projection.session_id,
            self.projection.task_id,
            self.action_log().join("\n")
        );
        let hash = canonical
            .bytes()
            .fold(0xcbf29ce484222325_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
            });
        format!("{hash:016x}")
    }
}

#[cfg(feature = "desktop")]
pub mod desktop {
    use gpui::{
        App, Application, Bounds, Context, Render, Window, WindowBounds, WindowOptions, div,
        prelude::*, px, rgb, size,
    };

    use super::{D1Slice, ProjectionState};

    pub struct D1Desktop {
        slice: D1Slice,
    }

    impl Render for D1Desktop {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let tokens = self.slice.theme.skin().tokens();
            div()
                .id("d1-shell")
                .size_full()
                .flex()
                .flex_col()
                .gap_2()
                .p_4()
                .bg(rgb(tokens.bg_base))
                .text_color(rgb(tokens.fg_primary))
                .child(div().child("Viden · project-1 / lane-1"))
                .child(
                    div()
                        .id("history-viewport")
                        .flex_1()
                        .overflow_scroll()
                        .child(div().id("tool-row").child("Core fixture ready"))
                        .child(div().id("new-output-count").child("0 new outputs")),
                )
                .child(
                    div()
                        .id("approval-dock")
                        .border_1()
                        .border_color(rgb(tokens.gold))
                        .p_2()
                        .child("Permission request · Allow once · Deny"),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(div().id("composer").flex_1().child("Message composer"))
                        .child(div().id("queue-action").child("Queue"))
                        .child(div().id("cancel-action").child("Cancel")),
                )
        }
    }

    pub fn run() {
        Application::new().run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|_| D1Desktop {
                        slice: D1Slice::new(ProjectionState::fixture()),
                    })
                },
            )
            .expect("open Viden GPUI D1 spike window");
            cx.activate(true);
        });
    }
}
