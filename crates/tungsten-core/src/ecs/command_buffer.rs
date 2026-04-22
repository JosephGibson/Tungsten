use super::entity::Entity;
use super::world::World;

/// Opaque handle to an entity queued for spawn in a [`CommandBuffer`]
/// but not yet flushed into the [`World`].
///
/// **Lifetime rule:** a `PendingEntity` is only valid for the buffer that
/// produced it, and only until [`World::flush`] is called for that buffer.
/// Do not store a `PendingEntity` across a flush boundary or use it with
/// a different buffer. Doing so will panic or corrupt entity state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PendingEntity(pub(crate) u32);

pub(super) trait ComponentSetter: 'static {
    fn apply(self: Box<Self>, world: &mut World, entity: Entity);
}

pub(super) struct InsertSetter<T: 'static> {
    pub(super) component: T,
}

impl<T: 'static> ComponentSetter for InsertSetter<T> {
    fn apply(self: Box<Self>, world: &mut World, entity: Entity) {
        world.insert(entity, self.component);
    }
}

pub(super) enum CommandTarget {
    Live(Entity),
    Pending(u32),
}

pub(super) enum Command {
    Spawn {
        pending_id: u32,
    },
    Insert {
        target: CommandTarget,
        setter: Box<dyn ComponentSetter>,
    },
    Remove(Box<dyn FnOnce(&mut World)>),
    Despawn(Entity),
}

/// Collects deferred structural mutations to apply to the [`World`] at a
/// fixed frame boundary (after all systems run, before extract/render).
pub struct CommandBuffer {
    pub(super) commands: Vec<Command>,
    pub(super) pending_count: u32,
}

impl CommandBuffer {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            pending_count: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Queue a spawn. Returns a [`PendingEntity`] handle that can be passed to
    /// [`insert_pending`](Self::insert_pending) within this buffer, before flush.
    pub fn spawn(&mut self) -> PendingEntity {
        let pending_id = self.pending_count;
        self.pending_count += 1;
        self.commands.push(Command::Spawn { pending_id });
        PendingEntity(pending_id)
    }

    /// Queue a component insert on a live (already-existing) entity.
    pub fn insert<T: 'static>(&mut self, entity: Entity, component: T) {
        self.commands.push(Command::Insert {
            target: CommandTarget::Live(entity),
            setter: Box::new(InsertSetter { component }),
        });
    }

    /// Queue a component insert on a not-yet-flushed pending entity.
    pub fn insert_pending<T: 'static>(&mut self, pending: PendingEntity, component: T) {
        self.commands.push(Command::Insert {
            target: CommandTarget::Pending(pending.0),
            setter: Box::new(InsertSetter { component }),
        });
    }

    /// Queue a component removal from a live entity.
    /// No-op at flush time if the entity is dead or lacks the component.
    pub fn remove_component<T: 'static>(&mut self, entity: Entity) {
        self.commands
            .push(Command::Remove(Box::new(move |world: &mut World| {
                world.remove_component::<T>(entity);
            })));
    }

    /// Queue a despawn. No-op at flush time if the entity is already dead.
    pub fn despawn(&mut self, entity: Entity) {
        self.commands.push(Command::Despawn(entity));
    }
}

impl Default for CommandBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "../tests/ecs/command_buffer.rs"]
mod tests;
