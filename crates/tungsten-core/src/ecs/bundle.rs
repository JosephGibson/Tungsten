//! Bundles: an entity's components, inserted in one archetype move
//! (`D-130`).
//!
//! `world.spawn_with((Player, transform, sprite))` spawns an entity straight
//! into the archetype of its components, where a run of `insert` calls moves
//! it once per component. A [`Bundle`] is a tuple of one to sixteen
//! components, a game type that implements the trait, or two bundles joined
//! with [`Bundle::with`]. [`CommandBuffer`] has the same forms for a system
//! that cannot take `&mut World`.
//!
//! A tuple's elements are components, never bundles: any `'static` type is
//! a component, so `(RigidBodyBundle::dynamic(..), Player)` inserts the
//! `RigidBodyBundle` value itself as one component. Join bundles with
//! `with`: `RigidBodyBundle::dynamic(..).with((Player, transform))`.

use super::command_buffer::{Command, CommandBuffer, CommandTarget, InsertQueues};

mod sealed {
    pub trait Sink {}
}

/// An entity's components, in insertion order.
///
/// Implemented for tuples of one to sixteen components, for [`Chain`], and
/// for game types, whose [`put`](Self::put) hands each component to the
/// sink. [`World::spawn_with`](super::World::spawn_with) and
/// [`World::insert_bundle`](super::World::insert_bundle) apply them as one
/// archetype move, with the result of inserting them one by one: a type the
/// bundle names twice keeps the later value, and a type the entity already
/// has is overwritten.
pub trait Bundle: 'static {
    /// Hands each component to `sink` in insertion order, as
    /// `sink.put(value)`.
    fn put<S: BundleSink>(self, sink: &mut S);

    /// This bundle's components, then `other`'s.
    #[must_use]
    fn with<B: Bundle>(self, other: B) -> Chain<Self, B>
    where
        Self: Sized,
    {
        Chain(self, other)
    }
}

/// Where [`Bundle::put`] sends a bundle's components: the `World`'s staging
/// for its direct forms, or a command buffer's queue. Sealed: those two are
/// the only sinks.
pub trait BundleSink: sealed::Sink {
    /// Takes the bundle's next component.
    fn put<T: 'static>(&mut self, value: T);
}

/// Two bundles as one: `A`'s components, then `B`'s ([`Bundle::with`]).
pub struct Chain<A, B>(A, B);

impl<A: Bundle, B: Bundle> Bundle for Chain<A, B> {
    #[inline]
    fn put<S: BundleSink>(self, sink: &mut S) {
        self.0.put(sink);
        self.1.put(sink);
    }
}

macro_rules! tuple_bundle {
    ($($T:ident $t:ident),+) => {
        impl<$($T: 'static),+> Bundle for ($($T,)+) {
            #[inline]
            fn put<S: BundleSink>(self, sink: &mut S) {
                let ($($t,)+) = self;
                $(sink.put($t);)+
            }
        }
    };
}

tuple_bundle!(C0 c0);
tuple_bundle!(C0 c0, C1 c1);
tuple_bundle!(C0 c0, C1 c1, C2 c2);
tuple_bundle!(C0 c0, C1 c1, C2 c2, C3 c3);
tuple_bundle!(C0 c0, C1 c1, C2 c2, C3 c3, C4 c4);
tuple_bundle!(C0 c0, C1 c1, C2 c2, C3 c3, C4 c4, C5 c5);
tuple_bundle!(C0 c0, C1 c1, C2 c2, C3 c3, C4 c4, C5 c5, C6 c6);
tuple_bundle!(C0 c0, C1 c1, C2 c2, C3 c3, C4 c4, C5 c5, C6 c6, C7 c7);
tuple_bundle!(C0 c0, C1 c1, C2 c2, C3 c3, C4 c4, C5 c5, C6 c6, C7 c7, C8 c8);
tuple_bundle!(C0 c0, C1 c1, C2 c2, C3 c3, C4 c4, C5 c5, C6 c6, C7 c7, C8 c8, C9 c9);
tuple_bundle!(C0 c0, C1 c1, C2 c2, C3 c3, C4 c4, C5 c5, C6 c6, C7 c7, C8 c8, C9 c9, C10 c10);
tuple_bundle!(
    C0 c0, C1 c1, C2 c2, C3 c3, C4 c4, C5 c5, C6 c6, C7 c7, C8 c8, C9 c9, C10 c10, C11 c11
);
tuple_bundle!(
    C0 c0, C1 c1, C2 c2, C3 c3, C4 c4, C5 c5, C6 c6, C7 c7, C8 c8, C9 c9, C10 c10, C11 c11,
    C12 c12
);
tuple_bundle!(
    C0 c0, C1 c1, C2 c2, C3 c3, C4 c4, C5 c5, C6 c6, C7 c7, C8 c8, C9 c9, C10 c10, C11 c11,
    C12 c12, C13 c13
);
tuple_bundle!(
    C0 c0, C1 c1, C2 c2, C3 c3, C4 c4, C5 c5, C6 c6, C7 c7, C8 c8, C9 c9, C10 c10, C11 c11,
    C12 c12, C13 c13, C14 c14
);
tuple_bundle!(
    C0 c0, C1 c1, C2 c2, C3 c3, C4 c4, C5 c5, C6 c6, C7 c7, C8 c8, C9 c9, C10 c10, C11 c11,
    C12 c12, C13 c13, C14 c14, C15 c15
);

/// The `World`'s staging for its direct bundle forms: each value in its
/// type's queue, and the queue of each in insertion order, for
/// `Archetypes::insert_run` to apply as one move (`D-084`). Kept across
/// calls, so a bundle of types staged before allocates nothing.
#[derive(Default)]
pub(super) struct BundleScratch {
    pub(super) values: InsertQueues,
    pub(super) run: Vec<u32>,
}

impl BundleScratch {
    /// Empties the scratch for the next bundle. A call that finished left it
    /// empty; one whose `put` panicked, or whose entity was dead, left
    /// values behind, which this drops.
    pub(super) fn reset(&mut self) {
        if !self.run.is_empty() {
            self.values.clear();
            self.run.clear();
        }
    }
}

impl sealed::Sink for BundleScratch {}

impl BundleSink for BundleScratch {
    #[inline]
    fn put<T: 'static>(&mut self, value: T) {
        let queue = self.values.push(value);
        self.run.push(queue);
    }
}

/// A command buffer as a bundle's sink: each component becomes an insert on
/// `target`, so the flush applies the bundle as one run (`D-084`).
pub(super) struct BufferSink<'b> {
    pub(super) buffer: &'b mut CommandBuffer,
    pub(super) target: CommandTarget,
}

impl sealed::Sink for BufferSink<'_> {}

impl BundleSink for BufferSink<'_> {
    #[inline]
    fn put<T: 'static>(&mut self, value: T) {
        let queue = self.buffer.values.push(value);
        self.buffer.commands.push(Command::Insert {
            target: self.target,
            queue,
        });
    }
}

#[cfg(test)]
#[path = "../tests/ecs/bundle.rs"]
mod tests;
