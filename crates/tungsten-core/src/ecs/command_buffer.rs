use super::entity::Entity;
use super::world::World;

/// Pending spawn handle; valid only for its source buffer before [`World::flush`].
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

/// Deferred structural mutations; flushed after systems, before extract/render.
pub struct CommandBuffer {
    pub(super) commands: Vec<Command>,
    pub(super) pending_count: u32,
}

impl CommandBuffer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            pending_count: 0,
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
        self.commands.push(Command::Insert {
            target: CommandTarget::Live(entity),
            setter: Box::new(InsertSetter { component }),
        });
    }

    /// Queue component insert on pending entity.
    pub fn insert_pending<T: 'static>(&mut self, pending: PendingEntity, component: T) {
        self.commands.push(Command::Insert {
            target: CommandTarget::Pending(pending.0),
            setter: Box::new(InsertSetter { component }),
        });
    }

    /// Queue component removal; dead/missing component is no-op at flush.
    pub fn remove_component<T: 'static>(&mut self, entity: Entity) {
        self.commands
            .push(Command::Remove(Box::new(move |world: &mut World| {
                world.remove_component::<T>(entity);
            })));
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

#[cfg(test)]
#[path = "../tests/ecs/command_buffer.rs"]
mod tests;
