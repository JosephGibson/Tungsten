//! The core UI model (`D-125`): the [`UiTree`] resource with generational
//! [`WidgetId`]s, roots on [`UiLayer`]s, a closed set of widget kinds, roles
//! and labels in node data, Tungsten-owned layout style types solved by Taffy
//! (`D-124`), a theme-token style model with text-only inheritance,
//! viewport-space [`Rect`]s, hit testing, keyboard focus, and a
//! [`FixedAdvanceMeasure`] text double for headless tests. M2 adds the
//! [`PaintList`] the tree paints into, in physical pixels under one rounding
//! rule (`D-127`), and the theme file behind the manifest's `ui_themes`.
//! Nothing here draws: the render crate consumes the paint list.

pub mod focus;
pub mod geometry;
pub mod hit;
pub mod id;
pub mod layout;
pub mod measure;
pub mod node;
pub mod paint;
pub mod style;
pub mod theme;
pub mod theme_file;
pub mod tree;

pub use geometry::Rect;
pub use id::WidgetId;
pub use measure::{FixedAdvanceMeasure, MeasureCounts};
pub use node::{Role, UiLayer, Visibility, WidgetKind};
pub use paint::{ClipRect, PaintCommand, PaintList, UiDebug, to_physical};
pub use style::{Align, Anchor, Direction, Edges, Justify, LayoutStyle, Placement, Sizing};
pub use theme::{
    ColorTokens, FontTokens, KindStyle, KindStyles, RadiusTokens, ResolvedStyle, SpacingTokens,
    StyleOverrides, TextOverrides, TextTokens, Theme, ThemeEpoch, Tokens, VisualStyle, WidgetState,
};
pub use theme_file::{ThemeFile, ThemeFileError, UiThemeRegistry};
pub use tree::{LayoutStats, UiTree};
