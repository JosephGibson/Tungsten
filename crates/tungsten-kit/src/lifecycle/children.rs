//! Child spawns: prefabs spawned once as an entity's children, at offsets
//! from it, through the command buffer.

use std::collections::HashSet;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::{
    CommandBuffer, Entity, Inspectable, MissingAsset, Plugin, PrefabRegistry, Schedule,
    SpawnContext, Transform, World, system,
};

use crate::foundation::describe::{
    ComponentDescription, FieldDescription, FieldType, PLATFORMER, SHMUP, describe, describe_shape,
    round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::trace::{TraceKind, trace};
use crate::foundation::vec2;
use crate::transform::hierarchy::{adopt_at_flush, queued_child};

/// [`SpawnChildrenPlugin`]'s system, in the react phase, after the spawner
/// by registration.
pub const SPAWN_CHILDREN: &str = "spawn_children";

/// The shape of one child.
const CHILD: &str = "child";

/// The engine's scene name of a physics body, which a child may not have.
const RIGID_BODY: &str = "rigid_body";

/// Prefabs spawned once as the entity's children: a torch's glow, a
/// gunship's wings. [`SpawnChildrenPlugin`]'s system spawns each at its
/// first fixed step, at the entity's `Transform.position` plus its offset,
/// through the `CommandBuffer`, so the children land at that frame's
/// command flush, then removes the component there. Each child is spawned
/// from its parent ([`CommandBuffer::spawn_prefab_from`]), so it takes what
/// the World's spawn inheritance gives, a state's `SceneEntity` and the
/// kit's [`Source`](crate::Source), and leaves with the parent's state; it
/// hangs from the parent ([`Parent`](crate::Parent),
/// [`LocalOffset`](crate::LocalOffset)) and is listed in its
/// [`Children`](crate::Children) at the flush, in this order, so it
/// follows the parent from the next propagation on and goes with
/// [`despawn_recursive`](crate::despawn_recursive). A parent despawned
/// earlier in the frame's commands spawns none.
///
/// A child's prefab may not hold a `rigid_body`: bodies live on root
/// entities (`D-132`). The load refuses one, as an unknown prefab ID; a
/// prefab inserted in code that holds one, or that does not prepare, logs
/// one error per prefab ID and spawns nothing.
///
/// Scene name `spawn_children`: `[{ "prefab": "glow", "offset": [0, -12] }]`,
/// `offset` `[0, 0]` by default. It is data: it serializes as that list.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use serde_json::json;
/// use tungsten_core::{
///     CommandBuffer, Plugin, PrefabData, PrefabRegistry, Schedule, Stage, Tag, Time, Transform,
///     World,
/// };
/// use tungsten_kit::{Children, LocalOffset, SpawnChildren, SpawnChildrenPlugin};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// world.insert_resource(CommandBuffer::new());
/// let glow: PrefabData = serde_json::from_value(json!({ "tag": "glow" })).unwrap();
/// let mut prefabs = PrefabRegistry::new();
/// prefabs.insert("glow".to_string(), glow);
/// world.insert_resource(prefabs);
/// let mut schedule = Schedule::new();
/// SpawnChildrenPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let torch = world.spawn_with((
///     Transform::from_position(Vec2::new(40.0, 80.0)),
///     SpawnChildren::new().with_child("glow", Vec2::new(0.0, -12.0)),
/// ));
/// world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
/// schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
/// world.flush_reusing(&mut commands);
///
/// let glow = world.get::<Children>(torch).unwrap().iter().next().unwrap();
/// assert_eq!(world.get::<Tag>(glow).unwrap().name, "glow");
/// assert_eq!(world.get::<Transform>(glow).unwrap().position, Vec2::new(40.0, 68.0));
/// assert_eq!(world.get::<LocalOffset>(glow), Some(&LocalOffset(Vec2::new(0.0, -12.0))));
/// assert!(!world.has::<SpawnChildren>(torch), "spawned once");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec<ChildDef>", into = "Vec<ChildDef>")]
pub struct SpawnChildren {
    children: Vec<(String, Vec2)>,
    /// The system has queued them.
    done: bool,
}

/// One child as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChildDef {
    prefab: String,
    #[serde(default, with = "vec2")]
    offset: Vec2,
}

impl TryFrom<Vec<ChildDef>> for SpawnChildren {
    type Error = String;

    fn try_from(children: Vec<ChildDef>) -> Result<Self, Self::Error> {
        if let Some((i, _)) = children
            .iter()
            .enumerate()
            .find(|(_, child)| !child.offset.is_finite())
        {
            return Err(format!("{i}.offset: must be finite"));
        }
        Ok(Self {
            children: children
                .into_iter()
                .map(|child| (child.prefab, child.offset))
                .collect(),
            done: false,
        })
    }
}

impl From<SpawnChildren> for Vec<ChildDef> {
    fn from(spawn: SpawnChildren) -> Self {
        spawn
            .children
            .into_iter()
            .map(|(prefab, offset)| ChildDef { prefab, offset })
            .collect()
    }
}

impl SpawnChildren {
    /// No children.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The list with prefab `prefab`, a manifest `prefabs` ID, after the
    /// children so far, `offset` from the parent.
    ///
    /// # Panics
    ///
    /// When `offset` is not finite.
    #[must_use]
    #[track_caller]
    pub fn with_child(mut self, prefab: impl Into<String>, offset: Vec2) -> Self {
        assert!(
            offset.is_finite(),
            "SpawnChildren::with_child: offset must be finite (got {offset})"
        );
        self.children.push((prefab.into(), offset));
        self
    }

    /// Each child's prefab and offset, in order.
    pub fn children(&self) -> impl Iterator<Item = (&str, Vec2)> {
        self.children
            .iter()
            .map(|(prefab, offset)| (prefab.as_str(), *offset))
    }
}

/// Whether the World's prefab `id` holds a physics body.
fn has_body(world: &World, id: &str) -> bool {
    world
        .get_resource::<PrefabRegistry>()
        .and_then(|prefabs| prefabs.get(id))
        .is_some_and(|data| data.components.contains_key(RIGID_BODY))
}

/// Registers the `spawn_children` scene component and [`SPAWN_CHILDREN`]
/// in the react phase. Its system spawns each [`SpawnChildren`] with a
/// `Transform` once; without a `CommandBuffer` nothing spawns and the lists
/// wait.
#[derive(Debug, Default, Clone, Copy)]
pub struct SpawnChildrenPlugin;

impl Plugin for SpawnChildrenPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<SpawnChildren>(world, "SpawnChildren");
        describe(
            world,
            ComponentDescription::new(
                SPAWN_CHILDREN,
                "Prefabs spawned once as the entity's children, at offsets from it.",
            )
            .field(FieldDescription::new(
                "children",
                FieldType::List(Box::new(FieldType::Shape(CHILD))),
                "the value itself: each child's prefab and offset, in order",
            ))
            .genre(PLATFORMER)
            .genre(SHMUP)
            .sample(r#"[{ "prefab": "glow" }]"#)
            .roundtrip(round_trip::<Vec<ChildDef>>),
        );
        describe_shape(
            world,
            ComponentDescription::new(
                CHILD,
                "One child: its prefab and its offset from the parent.",
            )
            .field(FieldDescription::new(
                "prefab",
                FieldType::Prefab,
                "what it spawns; the load checks the ID and refuses a prefab with a `rigid_body`",
            ))
            .field(
                FieldDescription::new(
                    "offset",
                    FieldType::Vec2,
                    "world pixels from the parent's position",
                )
                .default("[0.0, 0.0]"),
            )
            .sample(r#"{ "prefab": "glow" }"#)
            .roundtrip(round_trip::<ChildDef>),
        );
        world.register_component_with_assets(
            SPAWN_CHILDREN,
            |spawn: &SpawnChildren, world: &World| -> Result<(), MissingAsset> {
                for (i, (prefab, _)) in spawn.children.iter().enumerate() {
                    let field = format!("{i}.prefab");
                    world.check_prefab_id(&field, prefab)?;
                    if has_body(world, prefab) {
                        return Err(MissingAsset::new(
                            field,
                            "prefab without a rigid_body",
                            prefab.as_str(),
                        ));
                    }
                }
                Ok(())
            },
            |spawn: SpawnChildren, _: &SpawnContext<'_>| (spawn,),
        );
        let mut spawner = Spawn::default();
        schedule.add(
            Phase::React.stage(),
            Phase::React.join(system(SPAWN_CHILDREN, move |world: &mut World| {
                spawner.run(world);
            })),
        );
    }
}

/// The system's state between steps.
#[derive(Default)]
struct Spawn {
    /// The prefab IDs whose failure was logged.
    warned: HashSet<String>,
    /// This step's parents.
    due: Vec<Due>,
}

/// A parent whose children this step queues.
struct Due {
    parent: Entity,
    /// The parent's point.
    at: Vec2,
    children: Vec<(String, Vec2)>,
}

impl Spawn {
    fn run(&mut self, world: &mut World) {
        if !world.has_resource::<CommandBuffer>() {
            return;
        }
        for (entity, spawn, transform) in
            world.query_mut::<(Entity, &mut SpawnChildren, &Transform)>()
        {
            if std::mem::replace(&mut spawn.done, true) {
                continue;
            }
            self.due.push(Due {
                parent: entity,
                at: transform.position,
                children: std::mem::take(&mut spawn.children),
            });
        }
        let mut due = std::mem::take(&mut self.due);
        for Due {
            parent,
            at,
            children,
        } in due.drain(..)
        {
            for (index, (prefab, offset)) in (0_u32..).zip(children) {
                let prepared = if has_body(world, &prefab) {
                    Err("it holds a rigid_body; bodies live on root entities (D-132)".to_string())
                } else {
                    world
                        .prepare_prefab(&prefab, at + offset)
                        .map_err(|err| err.to_string())
                };
                match prepared {
                    Ok(prepared) => {
                        world.resource_mut::<CommandBuffer>().spawn_prefab_from(
                            parent,
                            prepared.with(queued_child(parent, offset, index)),
                        );
                        trace(world, parent, || TraceKind::Spawn {
                            prefab,
                            by: SPAWN_CHILDREN,
                        });
                    }
                    Err(error) => {
                        if !self.warned.contains(&prefab) {
                            log::error!("SpawnChildren: prefab `{prefab}` spawns nothing: {error}");
                            self.warned.insert(prefab.clone());
                        }
                        trace(world, parent, || TraceKind::SpawnFailed {
                            prefab,
                            by: SPAWN_CHILDREN,
                            error,
                        });
                    }
                }
            }
            let commands = world.resource_mut::<CommandBuffer>();
            commands.call(parent, adopt_at_flush);
            commands.remove_component::<SpawnChildren>(parent);
        }
        self.due = due;
    }
}

impl Inspectable for SpawnChildren {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let children = if self.done {
            "queued".to_string()
        } else {
            self.children
                .iter()
                .map(|(prefab, _)| prefab.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        vec![("children", children)]
    }
}

#[cfg(test)]
#[path = "../tests/children.rs"]
mod tests;
