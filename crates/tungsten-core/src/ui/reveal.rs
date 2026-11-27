//! Text revealed by count without relayout (redo 8.1): a typewriter's
//! line, which shows its first characters and then more, laid out once at
//! its whole length.
//!
//! The counts live in [`UiReveals`], a resource of their own, so neither
//! [`UiTree`] nor its nodes grow: the app takes the tree out and inserts it
//! again at every paint, and a field on either would cost every painted
//! frame an allocation of the larger size. Paint writes them into the
//! [`PaintList`]'s `reveals` under each label's text node, and the renderer
//! clips the label's run to its first characters there.

use crate::World;
use crate::text::TextNodeId;

use super::id::WidgetId;
use super::paint::PaintList;
use super::tree::UiTree;

/// The labels that show only their first characters, a World resource
/// [`UiReveals::of`] inserts on first use: per label, or per button's
/// label, how many characters of its laid-out text draw. A change marks no
/// layout and no paint; it raises [`revision`](Self::revision), which the
/// app's paint step reads to write the counts into the frame's paint list.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::World;
/// use tungsten_core::ui::{
///     FixedAdvanceMeasure, LayoutStyle, PaintList, UiLayer, UiReveals, UiTree,
/// };
///
/// let mut world = World::new();
/// let mut tree = UiTree::new();
/// let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
/// let line = tree.label(root, "Evening, warden.");
/// tree.layout(Vec2::new(320.0, 180.0), &mut FixedAdvanceMeasure::default());
/// let reveals = UiReveals::of(&mut world);
/// reveals.set(line, Some(3));
/// assert_eq!(reveals.get(line), Some(3));
/// assert!(!tree.is_layout_dirty(), "a count lays nothing out");
///
/// let mut list = PaintList::default();
/// reveals.write(&tree, &mut list);
/// assert_eq!(list.reveals.len(), 1);
/// assert_eq!(list.reveals[0].1, 3);
/// reveals.set(line, None);
/// reveals.write(&tree, &mut list);
/// assert!(list.reveals.is_empty(), "every character again");
/// ```
#[derive(Debug, Default)]
pub struct UiReveals {
    counts: Vec<(WidgetId, u32)>,
    revision: u64,
}

impl UiReveals {
    /// The World's reveals, inserted empty on first use.
    pub fn of(world: &mut World) -> &mut Self {
        if !world.has_resource::<Self>() {
            world.insert_resource(Self::default());
        }
        world.resource_mut::<Self>()
    }

    /// Shows the first `count` characters of `label`'s text, a line break
    /// counting as one; `None` shows every one. A count equal to the one held
    /// changes nothing.
    pub fn set(&mut self, label: WidgetId, count: Option<u32>) {
        let at = self.counts.iter().position(|&(id, _)| id == label);
        match (at, count) {
            (Some(at), Some(count)) if self.counts[at].1 == count => return,
            (Some(at), Some(count)) => self.counts[at].1 = count,
            (Some(at), None) => {
                self.counts.swap_remove(at);
            }
            (None, Some(count)) => self.counts.push((label, count)),
            (None, None) => return,
        }
        self.revision += 1;
    }

    /// `label`'s count; `None` while it shows every character.
    #[must_use]
    pub fn get(&self, label: WidgetId) -> Option<u32> {
        self.counts
            .iter()
            .find(|&&(id, _)| id == label)
            .map(|&(_, count)| count)
    }

    /// Raised by each change.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Writes the counts into `list.reveals`, each under its label's text
    /// node's base ID, so the outline and shadow copies of a label with
    /// coloured spans, which draw its plain view, reveal with it; a label
    /// the tree no longer holds, or that holds no text, is left out.
    pub fn write(&self, tree: &UiTree, list: &mut PaintList) {
        list.reveals.clear();
        list.reveals.extend(
            self.counts
                .iter()
                .filter_map(|&(label, count)| Some((text_node(tree, label)?, count))),
        );
    }
}

/// The base text node of `label`, or of a button's label.
fn text_node(tree: &UiTree, label: WidgetId) -> Option<TextNodeId> {
    let holder = tree.text_holder(label)?;
    tree.node(holder)?.text_id.map(TextNodeId::base)
}

#[cfg(test)]
#[path = "../tests/ui/reveal.rs"]
mod tests;
