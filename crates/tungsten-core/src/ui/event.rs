//! UI events (`D-149`): what the router publishes into
//! `EventQueue<UiEvent>` once a frame, before the systems that read them,
//! under `D-040`'s rotation. M55 adds a control's [`UiValue`] (`D-162`).

use crate::ecs::EventQueue;
use crate::input::MouseButton;

use super::id::WidgetId;

/// Something that happened to a widget at a routing (`D-149`). A
/// widget's events come only while it is the target of the pointer or of
/// focus: the nearest enabled interactive ancestor-or-self of the hit, or
/// the focused widget. Read a click with [`EventQueue::activated`], which
/// sees each activation in one frame only.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UiEvent {
    /// The pointer moved onto the widget, which now hovers.
    #[non_exhaustive]
    PointerEntered {
        /// The widget.
        widget: WidgetId,
    },
    /// The pointer moved off the widget, or it stopped hovering.
    #[non_exhaustive]
    PointerLeft {
        /// The widget.
        widget: WidgetId,
    },
    /// A button went down over the widget. The primary button also captures
    /// the pointer until its release.
    #[non_exhaustive]
    Pressed {
        /// The widget.
        widget: WidgetId,
        /// The button.
        button: MouseButton,
    },
    /// The button of an earlier `Pressed` went up, over the widget or not.
    #[non_exhaustive]
    Released {
        /// The widget.
        widget: WidgetId,
        /// The button.
        button: MouseButton,
    },
    /// A press ended without a release: focus loss, the pointer leaving the
    /// window, or the widget removed, hidden or disabled.
    #[non_exhaustive]
    PressCancelled {
        /// The widget.
        widget: WidgetId,
    },
    /// The widget was activated: a primary click released over it, or
    /// `ui_accept` pressed while it holds focus.
    #[non_exhaustive]
    Activated {
        /// The widget.
        widget: WidgetId,
    },
    /// A second click activated the widget soon after and near the first.
    /// Follows that click's `Activated`.
    #[non_exhaustive]
    DoubleClicked {
        /// The widget.
        widget: WidgetId,
    },
    /// The widget took keyboard focus, through the UI or the game.
    #[non_exhaustive]
    FocusGained {
        /// The widget.
        widget: WidgetId,
    },
    /// The widget lost keyboard focus.
    #[non_exhaustive]
    FocusLost {
        /// The widget.
        widget: WidgetId,
    },
    /// `ui_cancel` was pressed while the UI had the keyboard: back out of
    /// `scope`, the focused widget's scope, or the shown modal root.
    #[non_exhaustive]
    Cancel {
        /// The scope to back out of.
        scope: WidgetId,
    },
    /// A toggle or a slider took a new value from the pointer or a `ui_*`
    /// action (`D-162`). One per change, after the press's own events; a
    /// toggle's follows its `Activated`. The `UiTree` setters send none.
    /// Read it with [`EventQueue::value_changed`].
    #[non_exhaustive]
    ValueChanged {
        /// The toggle or slider.
        widget: WidgetId,
        /// Its new value.
        value: UiValue,
    },
}

/// A control's value, as [`UiEvent::ValueChanged`] carries it.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UiValue {
    /// A toggle's state.
    Bool(bool),
    /// A slider's value, inside its range and on its step.
    Number(f32),
}

impl UiValue {
    /// A toggle's state; `None` for a slider's value.
    #[must_use]
    pub const fn as_bool(self) -> Option<bool> {
        match self {
            Self::Bool(on) => Some(on),
            Self::Number(_) => None,
        }
    }

    /// A slider's value; `None` for a toggle's state.
    #[must_use]
    pub const fn as_number(self) -> Option<f32> {
        match self {
            Self::Number(value) => Some(value),
            Self::Bool(_) => None,
        }
    }
}

impl UiEvent {
    /// The widget the event is about; a `Cancel`'s scope.
    #[must_use]
    pub const fn widget(&self) -> WidgetId {
        match *self {
            Self::PointerEntered { widget }
            | Self::PointerLeft { widget }
            | Self::Pressed { widget, .. }
            | Self::Released { widget, .. }
            | Self::PressCancelled { widget }
            | Self::Activated { widget }
            | Self::DoubleClicked { widget }
            | Self::FocusGained { widget }
            | Self::FocusLost { widget }
            | Self::ValueChanged { widget, .. }
            | Self::Cancel { scope: widget } => widget,
        }
    }
}

impl EventQueue<UiEvent> {
    /// Whether `widget` was activated at this frame's routing. Reads
    /// [`iter_current`](Self::iter_current), so a reader that runs every
    /// frame sees one activation once, where [`iter`](Self::iter) would show
    /// it in two frames (`D-149`).
    #[must_use]
    pub fn activated(&self, widget: WidgetId) -> bool {
        self.iter_current()
            .any(|event| matches!(event, UiEvent::Activated { widget: w } if *w == widget))
    }

    /// The value `widget` took last at this frame's routing; `None` when it
    /// took none. Reads [`iter_current`](Self::iter_current), as
    /// [`activated`](Self::activated) does, so a reader that runs every
    /// frame sees each change once.
    #[must_use]
    pub fn value_changed(&self, widget: WidgetId) -> Option<UiValue> {
        self.iter_current()
            .filter_map(|event| match *event {
                UiEvent::ValueChanged { widget: w, value } if w == widget => Some(value),
                _ => None,
            })
            .last()
    }
}
