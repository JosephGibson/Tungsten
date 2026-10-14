//! The transform hierarchy: children that follow their parents' positions.
//!
//! Positions only: a child's `Transform.position` is its parent's plus its
//! [`LocalOffset`], down the chain, while rotation and scale stay each
//! entity's own. The default extract turns a sprite about its centre, not
//! about `Transform.position`, so a turned offset would leave the parent's
//! drawn shape, and tweens and squash keep writing those channels as on any
//! entity.

use glam::Vec2;
use tungsten_core::physics::PHYSICS_SYNC;
use tungsten_core::schedule::names::{
    CAMERA_UPDATE, PARTICLE_COUNT_REFRESH, SQUASH_STRETCH_TRIGGER, TWEEN_TICK,
};
use tungsten_core::{
    Bundle, CommandBuffer, Entity, Plugin, RigidBody, Schedule, Stage, Transform, With, Without,
    World, system,
};

/// [`HierarchyPlugin`]'s first pass, in `PostUpdate` after the physics sync
/// and before the particles, tweens, game feel and camera.
pub const TRANSFORM_PROPAGATE: &str = "transform_propagate";
/// [`HierarchyPlugin`]'s second pass, in `PostUpdate` after the tween tick
/// and before the camera. A game's `post_update` system that moves a parent
/// runs before it, and one that reads a child's position after it.
pub const TRANSFORM_PROPAGATE_LATE: &str = "transform_propagate_late";

/// The entity this one hangs from. Only [`spawn_child`] sets it; a game
/// reads it. Removing it detaches the child where it stands, and the next
/// propagation drops the child from the old parent's [`Children`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Parent(Entity);

impl Parent {
    /// The parent entity.
    #[must_use]
    pub fn entity(self) -> Entity {
        self.0
    }
}

/// An entity's children, in the order [`spawn_child`] added them. The kit
/// keeps it: [`spawn_child`] appends to it, the propagation drops the
/// entries that no longer hang from this entity, and [`despawn_recursive`]
/// drops the entity it despawns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Children(Vec<Entity>);

impl Children {
    /// The children, in order.
    pub fn iter(&self) -> impl Iterator<Item = Entity> {
        self.0.iter().copied()
    }

    /// How many children the list holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A child's position relative to its parent's `Transform.position`. A game
/// moves a child by writing its offset; the propagation writes the child's
/// `Transform.position` from it, and a child without one sits on its parent.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LocalOffset(pub Vec2);

/// Spawns `bundle` as a child of `parent`, `offset` from it, and returns the
/// child: one archetype move with its [`Parent`] and [`LocalOffset`], and
/// one of the parent the first time it takes [`Children`]. The child's
/// `Transform.position` starts at the parent's plus `offset`, keeping the
/// bundle's rotation and scale, or a `Transform` at that point when the
/// bundle has none. The spawn is immediate, so the frame's propagation and
/// extract see the child.
///
/// # Panics
///
/// When `parent` is not alive or has no `Transform`, or when the bundle
/// holds a [`RigidBody`]: bodies live on root entities (`D-132`). A refused
/// child is despawned first.
#[track_caller]
pub fn spawn_child<B: Bundle>(
    world: &mut World,
    parent: Entity,
    offset: Vec2,
    bundle: B,
) -> Entity {
    assert!(
        world.is_alive(parent),
        "spawn_child: the parent {parent:?} is not alive"
    );
    let Some(parent_position) = world.get::<Transform>(parent).map(|t| t.position) else {
        panic!("spawn_child: the parent {parent:?} has no Transform");
    };
    let position = parent_position + offset;
    let child = world.spawn_with(
        (Transform::from_position(position),)
            .with(bundle)
            .with((Parent(parent), LocalOffset(offset))),
    );
    if world.has::<RigidBody>(child) {
        world.despawn(child);
        panic!(
            "spawn_child: the bundle for a child of {parent:?} holds a RigidBody; bodies live on root entities (D-132)"
        );
    }
    // The bundle's own `Transform` keeps its rotation and scale.
    if let Some(transform) = world.get_mut::<Transform>(child) {
        transform.position = position;
    }
    if let Some(children) = world.get_mut::<Children>(parent) {
        children.0.push(child);
    } else {
        world.insert(parent, Children(vec![child]));
    }
    child
}

/// Queues the despawn of `entity` and its descendants for the command flush.
/// At the flush it despawns `entity` and every child that still hangs from
/// it, down the tree, and drops `entity` from its parent's [`Children`]. A
/// child whose [`Parent`] an earlier command in the same flush removed has
/// left the tree and stays; a dead `entity` is a no-op, as a plain
/// `despawn`'s is.
pub fn despawn_recursive(commands: &mut CommandBuffer, entity: Entity) {
    commands.call(entity, despawn_tree_at_flush);
}

/// The transform hierarchy's two propagation passes, in `PostUpdate`. Each
/// walks from every root (an entity with [`Children`] and no [`Parent`]),
/// parents before children, and writes each child's `Transform.position` as
/// its parent's plus its [`LocalOffset`]; rotation and scale are never
/// touched. Bodies may be parents: their children follow the interpolated
/// point the physics sync writes.
///
/// - [`TRANSFORM_PROPAGATE`] runs after the physics sync and before the
///   particles, tweens, game feel and camera, so an emitter on a child
///   leaves from where its body is. It also keeps the tree: it despawns,
///   with its subtree, a child whose parent has died, drops the entries that
///   no longer hang from their parent, and panics on an entity with both a
///   `Parent` and a `RigidBody`, naming it (`D-132`).
/// - [`TRANSFORM_PROPAGATE_LATE`] runs after the tween tick and before the
///   camera, and only writes positions, so a parent a tween or a game
///   system moved after the first pass draws with its children.
///
/// A parent despawned directly takes its children at the next first pass;
/// one despawned at a command flush leaves them for the next frame's, so
/// they draw one frame after it. [`despawn_recursive`] avoids that frame.
/// The constraints on the engine's names apply only when those are
/// registered.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::{CommandBuffer, Plugin, Schedule, Stage, Transform, World};
/// use tungsten_kit::{HierarchyPlugin, despawn_recursive, spawn_child};
///
/// struct Aura;
///
/// let mut world = World::new();
/// world.insert_resource(CommandBuffer::new());
/// let mut schedule = Schedule::new();
/// HierarchyPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let player = world.spawn_with((Transform::from_position(Vec2::new(100.0, 50.0)),));
/// let aura = spawn_child(&mut world, player, Vec2::new(0.0, -20.0), (Aura,));
/// assert_eq!(world.get::<Transform>(aura).unwrap().position, Vec2::new(100.0, 30.0));
///
/// // The player moves; the propagation brings the aura along.
/// world.get_mut::<Transform>(player).unwrap().position.x += 10.0;
/// schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
/// assert_eq!(world.get::<Transform>(aura).unwrap().position, Vec2::new(110.0, 30.0));
///
/// // Despawning the player takes the aura at the command flush.
/// despawn_recursive(world.resource_mut::<CommandBuffer>(), player);
/// let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
/// world.flush_reusing(&mut commands);
/// assert!(!world.is_alive(player) && !world.is_alive(aura));
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct HierarchyPlugin;

impl Plugin for HierarchyPlugin {
    fn build(&self, schedule: &mut Schedule, _world: &mut World) {
        let mut first = Walk::default();
        schedule.add(
            Stage::PostUpdate,
            system(TRANSFORM_PROPAGATE, move |world: &mut World| {
                first.first_pass(world);
            })
            .after_if_present(PHYSICS_SYNC)
            .before_if_present(PARTICLE_COUNT_REFRESH)
            .before_if_present(TWEEN_TICK)
            .before_if_present(SQUASH_STRETCH_TRIGGER)
            .before_if_present(CAMERA_UPDATE),
        );
        let mut late = Walk::default();
        schedule.add(
            Stage::PostUpdate,
            system(TRANSFORM_PROPAGATE_LATE, move |world: &mut World| {
                late.propagate(world);
            })
            .after(TRANSFORM_PROPAGATE)
            .after_if_present(TWEEN_TICK)
            .before_if_present(CAMERA_UPDATE),
        );
    }
}

/// Whether `child` is alive and its [`Parent`] still names `parent`: the
/// entries of `parent`'s [`Children`] that the walk and the recursive
/// despawn follow.
fn hangs_from(world: &World, child: Entity, parent: Entity) -> bool {
    world.is_alive(child) && world.get::<Parent>(child).is_some_and(|p| p.0 == parent)
}

/// [`despawn_recursive`]'s command.
fn despawn_tree_at_flush(world: &mut World, entity: Entity) {
    if !world.is_alive(entity) {
        return;
    }
    if let Some(parent) = world.get::<Parent>(entity).map(|p| p.0)
        && let Some(children) = world.get_mut::<Children>(parent)
    {
        children.0.retain(|&child| child != entity);
    }
    despawn_tree(world, entity, &mut Vec::new());
}

/// Despawns `root` and every live descendant, through `stack`.
fn despawn_tree(world: &mut World, root: Entity, stack: &mut Vec<Entity>) {
    stack.clear();
    stack.push(root);
    while let Some(entity) = stack.pop() {
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(
                children
                    .iter()
                    .filter(|&child| hangs_from(world, child, entity)),
            );
        }
        world.despawn(entity);
    }
}

/// A pass's scratch, kept between frames so a frame allocates nothing once
/// the passes have warmed up.
#[derive(Default)]
struct Walk {
    roots: Vec<Entity>,
    /// Entities still to write, each with its parent's position.
    stack: Vec<(Entity, Vec2)>,
    /// Parents whose `Children` hold entries the walk skipped.
    stale: Vec<Entity>,
    orphans: Vec<Entity>,
    /// The recursive despawn's stack and the pruned lists.
    scratch: Vec<Entity>,
}

impl Walk {
    /// The first pass: bodies refused, orphans despawned, positions written
    /// and the skipped entries pruned.
    fn first_pass(&mut self, world: &mut World) {
        if let Some(entity) = world
            .query_filtered::<Entity, (With<Parent>, With<RigidBody>)>()
            .next()
        {
            panic!(
                "{TRANSFORM_PROPAGATE}: {entity:?} has a Parent and a RigidBody; bodies live on root entities (D-132)"
            );
        }
        self.orphans.clear();
        self.orphans.extend(
            world
                .query::<(Entity, &Parent)>()
                .filter(|(_, parent)| !world.is_alive(parent.0))
                .map(|(entity, _)| entity),
        );
        for &orphan in &self.orphans {
            despawn_tree(world, orphan, &mut self.scratch);
        }
        self.propagate(world);
        for &parent in &self.stale {
            prune(world, parent, &mut self.scratch);
        }
    }

    /// Writes every live child's position from its parent's, roots first,
    /// and notes the parents whose lists hold entries the walk skipped.
    fn propagate(&mut self, world: &mut World) {
        let Self {
            roots,
            stack,
            stale,
            ..
        } = self;
        roots.clear();
        stale.clear();
        roots.extend(world.query_filtered::<Entity, (With<Children>, Without<Parent>)>());
        for &root in roots.iter() {
            let Some(position) = world.get::<Transform>(root).map(|t| t.position) else {
                continue;
            };
            push_children(world, root, position, stack, stale);
            while let Some((entity, parent_position)) = stack.pop() {
                let offset = world.get::<LocalOffset>(entity).map_or(Vec2::ZERO, |o| o.0);
                let position = parent_position + offset;
                if let Some(transform) = world.get_mut::<Transform>(entity) {
                    transform.position = position;
                }
                push_children(world, entity, position, stack, stale);
            }
        }
    }
}

/// Queues `parent`'s live children with `position`, the parent's own, and
/// notes `parent` in `stale` when its list holds an entry it skipped.
fn push_children(
    world: &World,
    parent: Entity,
    position: Vec2,
    stack: &mut Vec<(Entity, Vec2)>,
    stale: &mut Vec<Entity>,
) {
    let Some(children) = world.get::<Children>(parent) else {
        return;
    };
    let before = stack.len();
    stack.extend(
        children
            .iter()
            .filter(|&child| hangs_from(world, child, parent))
            .map(|child| (child, position)),
    );
    if stack.len() - before != children.len() {
        stale.push(parent);
    }
}

/// Drops the entries of `parent`'s list that no longer hang from it.
fn prune(world: &mut World, parent: Entity, scratch: &mut Vec<Entity>) {
    let Some(children) = world.get::<Children>(parent) else {
        return;
    };
    scratch.clear();
    scratch.extend(
        children
            .iter()
            .filter(|&child| hangs_from(world, child, parent)),
    );
    if let Some(children) = world.get_mut::<Children>(parent) {
        children.0.clear();
        children.0.extend_from_slice(scratch);
    }
}

#[cfg(test)]
#[path = "tests/hierarchy.rs"]
mod tests;
