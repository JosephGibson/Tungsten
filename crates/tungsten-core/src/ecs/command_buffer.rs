use std::any::{Any, TypeId};
use std::collections::VecDeque;

use super::archetype::{AnyColumn, TypeIdMap, TypedVec};
use super::entity::Entity;
use super::world::World;

/// Pending spawn handle; valid only for its source buffer before [`World::flush`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PendingEntity(pub(crate) u32);

/// Inserted values of one component type in recording order, type-erased for
/// the flush.
trait InsertQueue: Any {
    /// Empty column for the component, for an archetype created at flush.
    fn new_column(&self) -> Box<dyn AnyColumn>;
    /// Take the oldest value and write it at `row` of `column`, which stores
    /// this component: a push when `row` is the next row, an overwrite below
    /// that.
    fn write_next(&mut self, column: &mut dyn AnyColumn, row: usize);
    /// Take the oldest value and drop it.
    fn drop_next(&mut self);
}

struct TypedQueue<T: 'static>(VecDeque<T>);

impl<T: 'static> InsertQueue for TypedQueue<T> {
    fn new_column(&self) -> Box<dyn AnyColumn> {
        Box::new(TypedVec::<T>(Vec::new()))
    }

    fn write_next(&mut self, column: &mut dyn AnyColumn, row: usize) {
        let value = self.0.pop_front().expect("flush: insert queue ran dry");
        column
            .typed_mut::<T>()
            .expect("flush: column type mismatch")
            .write(row, value);
    }

    fn drop_next(&mut self) {
        self.0.pop_front();
    }
}

/// The values of a buffer's insert commands: one typed queue per component
/// type, so recording an insert boxes nothing (D-084).
///
/// An insert command names its queue. The flush handles every insert command
/// exactly once and in recording order, so the value a command recorded is
/// the oldest one left in its queue when the flush reaches it.
#[derive(Default)]
pub(super) struct InsertQueues {
    /// Component type per queue, in first-use order.
    types: Vec<TypeId>,
    queues: Vec<Box<dyn InsertQueue>>,
    /// Component type -> queue index.
    index: TypeIdMap<u32>,
}

impl InsertQueues {
    /// Queue `value` and return its queue's index.
    fn push<T: 'static>(&mut self, value: T) -> u32 {
        let t_id = TypeId::of::<T>();
        let queue = if let Some(&queue) = self.index.get(&t_id) {
            queue
        } else {
            let queue = self.queues.len() as u32;
            self.types.push(t_id);
            self.queues.push(Box::new(TypedQueue::<T>(VecDeque::new())));
            self.index.insert(t_id, queue);
            queue
        };
        let typed: &mut dyn Any = &mut *self.queues[queue as usize];
        typed
            .downcast_mut::<TypedQueue<T>>()
            .expect("insert queue type matches its key")
            .0
            .push_back(value);
        queue
    }

    /// Component type stored by `queue`.
    pub(super) fn component_type(&self, queue: u32) -> TypeId {
        self.types[queue as usize]
    }

    /// Empty column for `queue`'s component type.
    pub(super) fn new_column(&self, queue: u32) -> Box<dyn AnyColumn> {
        self.queues[queue as usize].new_column()
    }

    /// Take `queue`'s oldest value and write it at `row` of `column`: a push
    /// when `row` is the next row, an overwrite below that.
    pub(super) fn write_next(&mut self, queue: u32, column: &mut dyn AnyColumn, row: usize) {
        self.queues[queue as usize].write_next(column, row);
    }

    /// Take `queue`'s oldest value and drop it: its target died before the
    /// flush reached the command.
    pub(super) fn drop_next(&mut self, queue: u32) {
        self.queues[queue as usize].drop_next();
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum CommandTarget {
    Live(Entity),
    Pending(u32),
}

#[derive(Clone, Copy)]
pub(super) enum Command {
    Spawn {
        pending_id: u32,
    },
    /// The value is the oldest one left in queue `queue`.
    Insert {
        target: CommandTarget,
        queue: u32,
    },
    Remove {
        entity: Entity,
        remove: fn(&mut World, Entity),
    },
    Despawn(Entity),
}

impl Command {
    /// Whether this is an insert on `target`.
    pub(super) fn inserts_on(&self, target: CommandTarget) -> bool {
        matches!(self, Command::Insert { target: own, .. } if *own == target)
    }
}

/// Deferred structural mutations; flushed after systems, before extract/render.
///
/// A flushed buffer is empty and keeps its storage, so a buffer that lives
/// across frames stops allocating once it has seen its largest frame
/// ([`World::flush_reusing`]).
pub struct CommandBuffer {
    pub(super) commands: Vec<Command>,
    pub(super) pending_count: u32,
    pub(super) values: InsertQueues,
    /// Flush scratch: the entity of each pending spawn.
    pub(super) pending_entities: Vec<Entity>,
}

impl CommandBuffer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            pending_count: 0,
            values: InsertQueues::default(),
            pending_entities: Vec::new(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Queue spawn and return pending handle for this buffer.
    pub fn spawn(&mut self) -> PendingEntity {
        let pending_id = self.pending_count;
        self.pending_count += 1;
        self.commands.push(Command::Spawn { pending_id });
        PendingEntity(pending_id)
    }

    /// Queue component insert on live entity.
    pub fn insert<T: 'static>(&mut self, entity: Entity, component: T) {
        let queue = self.values.push(component);
        self.commands.push(Command::Insert {
            target: CommandTarget::Live(entity),
            queue,
        });
    }

    /// Queue component insert on pending entity.
    pub fn insert_pending<T: 'static>(&mut self, pending: PendingEntity, component: T) {
        let queue = self.values.push(component);
        self.commands.push(Command::Insert {
            target: CommandTarget::Pending(pending.0),
            queue,
        });
    }

    /// Queue component removal; dead/missing component is no-op at flush.
    pub fn remove_component<T: 'static>(&mut self, entity: Entity) {
        self.commands.push(Command::Remove {
            entity,
            remove: remove_component::<T>,
        });
    }

    /// Queue despawn; dead entity is no-op at flush.
    pub fn despawn(&mut self, entity: Entity) {
        self.commands.push(Command::Despawn(entity));
    }
}

impl Default for CommandBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// What a removal command runs at flush; a plain function, so the command
/// boxes nothing.
fn remove_component<T: 'static>(world: &mut World, entity: Entity) {
    world.remove_component::<T>(entity);
}

#[cfg(test)]
#[path = "../tests/ecs/command_buffer.rs"]
mod tests;
