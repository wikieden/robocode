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
    pub view_hash: String,
}

impl ProjectionState {
    pub fn fixture() -> Self {
        let projection = viden_gui_spike_common::D1FixtureProjection::from_committed_fixture()
            .expect("canonical D1 fixture projection");
        Self {
            project_id: projection.project_id,
            lane_id: projection.lane_id,
            session_id: projection.session_id,
            task_id: projection.task_id,
            view_hash: projection.view_hash,
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

    pub fn sync_composer_from_framework(&mut self, value: &str) {
        self.composer.sync_from_framework(value);
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
        self.projection.view_hash.clone()
    }
}

#[cfg(feature = "desktop")]
pub mod desktop {
    use gpui::{
        App, Application, Bounds, Context, Entity, Render, Subscription, Window, WindowBounds,
        WindowOptions, div, prelude::*, px, rgb, size,
    };
    use gpui_component::{
        Root,
        button::Button,
        input::{Input, InputEvent, InputState},
    };

    use crate::{
        approval::ApprovalChoice,
        theme::{Density, Skin},
    };

    use super::{D1Slice, ProjectionState};

    pub struct D1Desktop {
        slice: D1Slice,
        composer: Entity<InputState>,
        _subscriptions: Vec<Subscription>,
    }

    impl D1Desktop {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let composer = cx.new(|cx| {
                InputState::new(window, cx)
                    .multi_line(true)
                    .placeholder("Message composer")
            });
            let _subscriptions = vec![cx.subscribe_in(
                &composer,
                window,
                |this, input, event: &InputEvent, _window, cx| match event {
                    InputEvent::Change => {
                        let value = input.read(cx).value();
                        this.slice.sync_composer_from_framework(value.as_ref());
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => {
                        let value = input.read(cx).value();
                        this.slice.sync_composer_from_framework(value.as_ref());
                        this.slice.composer.submit();
                        cx.notify();
                    }
                    InputEvent::Focus => {
                        this.slice.focus("composer");
                        cx.notify();
                    }
                    InputEvent::Blur => {}
                },
            )];
            let mut slice = D1Slice::new(ProjectionState::fixture());
            slice.start_stream();
            Self {
                slice,
                composer,
                _subscriptions,
            }
        }

        fn action_button(
            &self,
            id: &'static str,
            label: &'static str,
            cx: &Context<Self>,
            action: impl Fn(&mut D1Slice) + 'static,
        ) -> Button {
            let view = cx.entity();
            Button::new(id)
                .label(label)
                .tab_stop(true)
                .on_click(move |_, _window, cx| {
                    view.update(cx, |this, cx| {
                        if D1Slice::REQUIRED_ROLES.contains(&id) {
                            this.slice.focus(id);
                        }
                        action(&mut this.slice);
                        cx.notify();
                    });
                })
        }
    }

    impl Render for D1Desktop {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let tokens = self.slice.theme.skin().tokens();
            let history_view = cx.entity();
            div()
                .id("d1-shell")
                .size_full()
                .flex()
                .flex_col()
                .gap_2()
                .p_4()
                .bg(rgb(tokens.bg_base))
                .text_color(rgb(tokens.fg_primary))
                .child(div().child(format!(
                    "Viden · {} / {}",
                    self.slice.projection.project_id, self.slice.projection.lane_id
                )))
                .child(
                    div()
                        .id("history-viewport")
                        .flex_1()
                        .overflow_scroll()
                        .on_scroll_wheel(move |_, _window, cx| {
                            history_view.update(cx, |this, cx| {
                                this.slice.transcript.open_history_at("row-120");
                                this.slice.focus("history-viewport");
                                cx.notify();
                            });
                        })
                        .child(div().id("tool-row").child("Core fixture ready"))
                        .child(div().id("new-output-count").child(format!(
                            "{} new outputs",
                            self.slice.transcript.new_output_count()
                        ))),
                )
                .child(
                    div()
                        .id("approval-dock")
                        .border_1()
                        .border_color(rgb(tokens.gold))
                        .p_2()
                        .flex()
                        .gap_2()
                        .child(
                            self.action_button("approval-dock", "Allow once", cx, |slice| {
                                slice.approval.respond(ApprovalChoice::AllowOnce)
                            }),
                        )
                        .child(self.action_button("approval-deny", "Deny", cx, |slice| {
                            slice.approval.respond(ApprovalChoice::Deny)
                        })),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            div()
                                .id("composer")
                                .flex_1()
                                .child(Input::new(&self.composer).h(px(84.0))),
                        )
                        .child(self.action_button("queue-action", "Queue", cx, |slice| {
                            slice.queue_current_draft()
                        }))
                        .child(self.action_button("cancel-action", "Cancel", cx, |slice| {
                            slice.cancel_stream()
                        })),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(self.action_button("theme-ice", "Ice light", cx, |slice| {
                            slice.theme.select(Skin::IceLight, slice.theme.density())
                        }))
                        .child(self.action_button("density-comfy", "Comfy", cx, |slice| {
                            slice.theme.select(slice.theme.skin(), Density::Comfy)
                        })),
                )
        }
    }

    pub fn run() {
        Application::new().run(|cx: &mut App| {
            gpui_component::init(cx);
            let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| D1Desktop::new(window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .expect("open Viden GPUI D1 spike window");
            cx.activate(true);
        });
    }
}
