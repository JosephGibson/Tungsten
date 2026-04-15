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
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct Position {
        x: f32,
        y: f32,
    }

    #[test]
    fn new_buffer_is_empty() {
        let buffer = CommandBuffer::new();
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn spawn_increments_len() {
        let mut buffer = CommandBuffer::new();
        assert_eq!(buffer.len(), 0);

        buffer.spawn();
        assert_eq!(buffer.len(), 1);

        buffer.spawn();
        assert_eq!(buffer.len(), 2);
    }

    #[test]
    fn spawn_returns_distinct_pending_ids() {
        let mut buffer = CommandBuffer::new();
        let a = buffer.spawn();
        let b = buffer.spawn();
        assert_ne!(a, b);
    }

    #[test]
    fn despawn_queued() {
        let mut buffer = CommandBuffer::new();
        let entity = Entity {
            index: 0,
            generation: 0,
        };

        buffer.despawn(entity);

        assert_eq!(buffer.len(), 1);
    }

    #[test]
    fn insert_live_queued() {
        let mut buffer = CommandBuffer::new();
        let entity = Entity {
            index: 0,
            generation: 0,
        };

        buffer.insert(entity, Position { x: 1.0, y: 2.0 });

        assert_eq!(buffer.len(), 1);
    }

    #[test]
    fn insert_pending_queued() {
        let mut buffer = CommandBuffer::new();
        let pending = buffer.spawn();

        buffer.insert_pending(pending, Position { x: 1.0, y: 2.0 });

        assert_eq!(buffer.len(), 2);
    }

    #[test]
    fn remove_component_queued() {
        let mut buffer = CommandBuffer::new();
        let entity = Entity {
            index: 0,
            generation: 0,
        };

        buffer.remove_component::<Position>(entity);

        assert_eq!(buffer.len(), 1);
    }
}
