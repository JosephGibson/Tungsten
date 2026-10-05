//! Widget handles (`D-125`).

/// A widget in a [`UiTree`](super::UiTree). Generational, as
/// [`TextNodeId`](crate::text::TextNodeId) is: a removed widget's ID never
/// names the widget that later takes its slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WidgetId {
    index: u32,
    generation: u32,
}

impl WidgetId {
    /// The ID of slot `index` at `generation`. The tree mints them.
    #[must_use]
    pub(crate) const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    /// The slot index.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }

    /// The slot's generation when the ID was minted.
    #[must_use]
    pub const fn generation(self) -> u32 {
        self.generation
    }
}
