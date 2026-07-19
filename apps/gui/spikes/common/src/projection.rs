use viden_core::{CoreTransport, EventCursor, RuntimeViewState, StatefulCoreClient};

/// A full state/cursor pair already validated and reduced by Core.
///
/// The private fields and Core-only constructor keep the GUI from projecting
/// raw runtime events through a second business reducer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedState {
    cursor: EventCursor,
    view: RuntimeViewState,
}

impl ConfirmedState {
    pub fn from_core<T>(client: &StatefulCoreClient<T>) -> Option<Self>
    where
        T: CoreTransport,
    {
        Some(Self {
            cursor: client.confirmed_cursor()?.clone(),
            view: client.confirmed_view()?.clone(),
        })
    }
}

/// GUI-owned presentation projection of Core's last confirmed full view.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuiProjection {
    cursor: Option<EventCursor>,
    view: Option<RuntimeViewState>,
    generation: u64,
}

impl GuiProjection {
    /// Publishes only full Core-confirmed states; this method never applies
    /// `RuntimeEvent` values or interprets cursor/replay semantics.
    pub fn apply_batch(&mut self, states: impl IntoIterator<Item = ConfirmedState>) {
        for state in states {
            if self.cursor.as_ref() == Some(&state.cursor)
                && self.view.as_ref() == Some(&state.view)
            {
                continue;
            }
            self.cursor = Some(state.cursor);
            self.view = Some(state.view);
            self.generation = self.generation.saturating_add(1);
        }
    }

    pub fn cursor(&self) -> Option<&EventCursor> {
        self.cursor.as_ref()
    }

    pub fn view(&self) -> Option<&RuntimeViewState> {
        self.view.as_ref()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
}
