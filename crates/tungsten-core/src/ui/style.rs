//! Tungsten-owned layout style (`D-124`): the only layout names a game
//! writes. The solver sits behind them in `layout/taffy.rs`.

use glam::Vec2;

/// How one axis of a widget is sized (w01 §8).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Sizing {
    /// Content-sized on the main axis, never shrinking; follows the parent's
    /// [`Align`] on the cross axis (stretched under the default).
    #[default]
    Auto,
    /// A fixed size in UI units.
    Px(f32),
    /// A percentage of the parent's inner size, 0–100.
    Percent(f32),
    /// The remaining space on the main axis, shared with other `Fill`
    /// siblings; stretched on the cross axis.
    Fill,
}

/// Per-side lengths in logical order: `start` is left in left-to-right text.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Edges {
    /// The top side.
    pub top: f32,
    /// The end side: right in left-to-right text.
    pub end: f32,
    /// The bottom side.
    pub bottom: f32,
    /// The start side: left in left-to-right text.
    pub start: f32,
}

impl Edges {
    /// The same length on every side.
    #[must_use]
    pub const fn all(value: f32) -> Self {
        Self {
            top: value,
            end: value,
            bottom: value,
            start: value,
        }
    }

    /// `horizontal` at start and end, `vertical` at top and bottom.
    #[must_use]
    pub const fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Self {
            top: vertical,
            end: horizontal,
            bottom: vertical,
            start: horizontal,
        }
    }
}

/// The axis a widget's children flow along.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Left to right.
    Row,
    /// Top to bottom.
    #[default]
    Column,
}

/// Cross-axis alignment of a widget's children, or of one child through
/// [`LayoutStyle::align_self`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Align {
    /// The cross-axis start: the top of a row, the start of a column.
    Start,
    /// Centred on the cross axis.
    Center,
    /// The cross-axis end.
    End,
    /// Children take the container's cross size.
    #[default]
    Stretch,
}

/// Main-axis distribution of a widget's children.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Justify {
    /// Packed at the start.
    #[default]
    Start,
    /// Packed in the middle.
    Center,
    /// Packed at the end.
    End,
    /// The free space between children.
    SpaceBetween,
    /// The free space around every child, half at each end.
    SpaceAround,
    /// The free space in equal gaps, the ends included.
    SpaceEvenly,
}

/// Where an out-of-flow widget sits against its parent's box.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Anchor {
    /// The top start corner.
    #[default]
    TopStart,
    /// The middle of the top edge.
    TopCenter,
    /// The top end corner.
    TopEnd,
    /// The middle of the start edge.
    CenterStart,
    /// The centre.
    Center,
    /// The middle of the end edge.
    CenterEnd,
    /// The bottom start corner.
    BottomStart,
    /// The middle of the bottom edge.
    BottomCenter,
    /// The bottom end corner.
    BottomEnd,
}

/// In the parent's flow, or anchored out of it.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Placement {
    /// Laid out with its siblings along the parent's [`Direction`].
    #[default]
    Flow,
    /// Out of flow against the parent's box, taking no space in it.
    Anchored {
        /// The point of the parent's box the widget sits against.
        anchor: Anchor,
        /// On an axis anchored at `Start` or `End`, the distance from that
        /// edge into the parent; on a centred axis, a signed shift (positive
        /// is right or down).
        offset: Vec2,
    },
}

/// A widget's layout style. The solver sits behind it (`D-124`), so these
/// are the only layout names a game writes. The default is a column in flow,
/// `Auto` both ways, no limits, zero edges and gap, children stretched on
/// the cross axis and packed at the start.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutStyle {
    /// In the parent's flow, or anchored.
    pub placement: Placement,
    /// The axis this widget's children flow along.
    pub direction: Direction,
    /// Width.
    pub width: Sizing,
    /// Height.
    pub height: Sizing,
    /// The narrowest the widget may get, in UI units.
    pub min_width: Option<f32>,
    /// The widest the widget may get.
    pub max_width: Option<f32>,
    /// The shortest the widget may get.
    pub min_height: Option<f32>,
    /// The tallest the widget may get.
    pub max_height: Option<f32>,
    /// Space inside the border, around the children.
    pub padding: Edges,
    /// Space outside the box.
    pub margin: Edges,
    /// Space between children.
    pub gap: f32,
    /// Cross-axis alignment of this widget's children.
    pub align_items: Align,
    /// Main-axis distribution of this widget's children.
    pub justify_content: Justify,
    /// This widget's own cross-axis alignment, over the parent's
    /// `align_items`.
    pub align_self: Option<Align>,
}

impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            placement: Placement::Flow,
            direction: Direction::Column,
            width: Sizing::Auto,
            height: Sizing::Auto,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: Edges::default(),
            margin: Edges::default(),
            gap: 0.0,
            align_items: Align::Stretch,
            justify_content: Justify::Start,
            align_self: None,
        }
    }
}

impl LayoutStyle {
    /// A row with the default everything else.
    #[must_use]
    pub fn row() -> Self {
        Self {
            direction: Direction::Row,
            ..Self::default()
        }
    }

    /// A column with the default everything else.
    #[must_use]
    pub fn column() -> Self {
        Self::default()
    }

    /// This style with `width` and `height`.
    #[must_use]
    pub fn with_size(mut self, width: Sizing, height: Sizing) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// This style with `padding` on every side.
    #[must_use]
    pub fn with_padding(mut self, padding: f32) -> Self {
        self.padding = Edges::all(padding);
        self
    }

    /// This style with `gap` between children.
    #[must_use]
    pub fn with_gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    /// This style anchored at `anchor` with `offset`, out of the parent's
    /// flow.
    #[must_use]
    pub fn anchored(mut self, anchor: Anchor, offset: Vec2) -> Self {
        self.placement = Placement::Anchored { anchor, offset };
        self
    }
}

/// One column or row of a [`Grid`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Track {
    /// A column as wide as its widest cell's content, measured at
    /// max-content; a row as tall as its tallest cell.
    Auto,
    /// A fixed size in UI units.
    Px(f32),
    /// A share of the width the fixed and `Auto` columns leave, by weight:
    /// columns only, above 0.
    Fr(f32),
}

/// The most columns a grid lays out.
const MAX_COLUMNS: usize = 32;

/// A grid's tracks (`D-124` as `D-317` amends it): its flowed children
/// fill rows of `columns.len()` cells in order, each cell as wide as its
/// column, so the columns line up across rows; a row is as tall as its
/// tallest cell, or its `rows` track's size. Laid out on the flex solver
/// by rows core adds to that root's solve alone, so a root with no grid
/// lays out as before.
///
/// # Examples
///
/// ```
/// use tungsten_core::ui::{Grid, Track};
///
/// let keys = Grid::new([Track::Auto, Track::Fr(1.0), Track::Fr(1.0)]).with_rows([Track::Px(24.0)]);
/// assert_eq!(keys.columns().len(), 3);
/// assert_eq!(keys.rows(), [Track::Px(24.0)]);
/// assert!(Grid::try_new(vec![], vec![]).is_err(), "at least one column");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    columns: Vec<Track>,
    rows: Vec<Track>,
}

impl Grid {
    /// A grid of `columns`, its rows sized by their cells.
    ///
    /// # Panics
    ///
    /// When there are no columns or more than 32, or a track is not
    /// finite and at least 0, or an `Fr` not above 0.
    #[must_use]
    #[track_caller]
    pub fn new(columns: impl IntoIterator<Item = Track>) -> Self {
        Self::try_new(columns.into_iter().collect(), Vec::new())
            .unwrap_or_else(|error| panic!("Grid::new: {error}"))
    }

    /// This grid with `rows`, one track a row from the first, the rest
    /// `Auto`.
    ///
    /// # Panics
    ///
    /// When a track is not finite and at least 0, or is an `Fr`.
    #[must_use]
    #[track_caller]
    pub fn with_rows(self, rows: impl IntoIterator<Item = Track>) -> Self {
        Self::try_new(self.columns, rows.into_iter().collect())
            .unwrap_or_else(|error| panic!("Grid::with_rows: {error}"))
    }

    /// A grid of `columns` and `rows`, or why it cannot be one: 1 to 32
    /// columns, every track finite and at least 0, an `Fr` above 0 and in a
    /// column alone.
    pub fn try_new(columns: Vec<Track>, rows: Vec<Track>) -> Result<Self, String> {
        if columns.is_empty() || columns.len() > MAX_COLUMNS {
            return Err(format!(
                "a grid has 1 to {MAX_COLUMNS} columns, not {}",
                columns.len()
            ));
        }
        for (at, track) in columns.iter().enumerate() {
            match *track {
                Track::Px(v) if !(v.is_finite() && v >= 0.0) => {
                    return Err(format!("column {at}: {v} is not a finite size at least 0"));
                }
                Track::Fr(v) if !(v.is_finite() && v > 0.0) => {
                    return Err(format!("column {at}: {v}fr is not a share above 0"));
                }
                _ => {}
            }
        }
        for (at, track) in rows.iter().enumerate() {
            match *track {
                Track::Px(v) if !(v.is_finite() && v >= 0.0) => {
                    return Err(format!("row {at}: {v} is not a finite size at least 0"));
                }
                Track::Fr(_) => {
                    return Err(format!(
                        "row {at}: a row takes `auto` or a length, not a share"
                    ));
                }
                _ => {}
            }
        }
        Ok(Self { columns, rows })
    }

    /// Its columns.
    #[must_use]
    pub fn columns(&self) -> &[Track] {
        &self.columns
    }

    /// Its rows' tracks, from the first; a row past them is `Auto`.
    #[must_use]
    pub fn rows(&self) -> &[Track] {
        &self.rows
    }
}

#[cfg(test)]
#[path = "../tests/ui/style.rs"]
mod tests;
