//! Action input: the action map read into each entity's [`Intent`], which
//! the movers take as their input.

use std::collections::BTreeMap;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::{
    ActionMap, InputState, Inspectable, MissingAsset, Plugin, RoutedInput, Schedule, SpawnContext,
    World, system,
};

use crate::foundation::describe::{
    ComponentDescription, FieldDescription, FieldType, GRID, PLATFORMER, SHMUP, TOP_DOWN, describe,
    round_trip,
};
use crate::foundation::inspect::{point, register_rows};
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::{optional_vec2, vec2};

/// [`ActionInputPlugin`]'s system, in the intent phase.
pub const ACTION_INPUT: &str = "action_input";

/// The scene component the plugin registers.
const ACTION_INPUT_COMPONENT: &str = "action_input";

/// Which actions steer the entity: two or four movement actions, four aim
/// actions when it aims, and named slots such as `jump`, each an action of
/// the `ActionMap`. [`ActionInputPlugin`]'s system reads them each fixed
/// step into the entity's [`Intent`], from the UI's leftover input
/// (`RoutedInput`) when the resource exists and the raw `InputState`
/// otherwise; a game's AI writes an `Intent` directly, so the movers tell
/// no difference.
///
/// Scene name `action_input`:
/// `{ "move": ["move_left", "move_right", "move_up", "move_down"], "aim": [left, right, up, down], "slots": { "jump": "jump" } }`,
/// `move` of two actions for one axis, `aim` and `slots` optional. The
/// load checks every action against the action map's declared actions, so
/// a misspelt one fails the load naming its field (`move.1`, `slots.jump`)
/// rather than steering nothing; a World without an `ActionMap` passes.
/// It is data: it serializes in the scene form.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::{
///     ActionMap, Binding, InputState, KeyCode, Plugin, Schedule, Stage, World,
/// };
/// use tungsten_kit::{ActionInput, ActionInputPlugin, Intent};
///
/// let mut actions = ActionMap::new();
/// actions.replace_bindings("move_left", vec![Binding::Key { code: KeyCode::ArrowLeft }]);
/// actions.replace_bindings("move_right", vec![Binding::Key { code: KeyCode::ArrowRight }]);
/// actions.replace_bindings("jump", vec![Binding::Key { code: KeyCode::Space }]);
/// let mut world = World::new();
/// world.insert_resource(actions);
/// world.insert_resource(InputState::new());
/// let mut schedule = Schedule::new();
/// ActionInputPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let hero = world.spawn_with((
///     ActionInput::new("move_left", "move_right").with_slot("jump", "jump"),
///     Intent::default(),
/// ));
/// let input = world.resource_mut::<InputState>();
/// input.key_down(KeyCode::ArrowRight);
/// input.key_down(KeyCode::Space);
/// schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// let intent = world.get::<Intent>(hero).unwrap();
/// assert_eq!(intent.movement(), Vec2::new(1.0, 0.0));
/// assert!(intent.held("jump") && intent.pressed("jump"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ActionInputDef", into = "ActionInputDef")]
pub struct ActionInput {
    /// `[left, right]` or `[left, right, up, down]`.
    move_axes: Vec<String>,
    /// `[left, right, up, down]`.
    aim_axes: Option<[String; 4]>,
    /// `(slot, action)`, sorted by slot.
    slots: Vec<(String, String)>,
}

/// `components.action_input` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionInputDef {
    #[serde(rename = "move")]
    move_actions: Vec<String>,
    #[serde(default)]
    aim: Option<[String; 4]>,
    #[serde(default)]
    slots: BTreeMap<String, String>,
}

impl TryFrom<ActionInputDef> for ActionInput {
    type Error = String;

    fn try_from(def: ActionInputDef) -> Result<Self, Self::Error> {
        if def.move_actions.len() != 2 && def.move_actions.len() != 4 {
            return Err(format!(
                "move: 2 actions ([left, right]) or 4 ([left, right, up, down]), got {}",
                def.move_actions.len()
            ));
        }
        Ok(Self {
            move_axes: def.move_actions,
            aim_axes: def.aim,
            slots: def.slots.into_iter().collect(),
        })
    }
}

impl From<ActionInput> for ActionInputDef {
    fn from(input: ActionInput) -> Self {
        Self {
            move_actions: input.move_axes,
            aim: input.aim_axes,
            slots: input.slots.into_iter().collect(),
        }
    }
}

impl ActionInput {
    /// Input moving along one axis: `left` and `right` are action names.
    #[must_use]
    pub fn new(left: impl Into<String>, right: impl Into<String>) -> Self {
        Self {
            move_axes: vec![left.into(), right.into()],
            aim_axes: None,
            slots: Vec::new(),
        }
    }

    /// The input, also moving along the vertical axis by `up` and `down`.
    #[must_use]
    pub fn with_vertical(mut self, up: impl Into<String>, down: impl Into<String>) -> Self {
        self.move_axes.truncate(2);
        self.move_axes.push(up.into());
        self.move_axes.push(down.into());
        self
    }

    /// The input, also aiming by `axes`: `[left, right, up, down]`.
    #[must_use]
    pub fn with_aim(mut self, axes: [impl Into<String>; 4]) -> Self {
        self.aim_axes = Some(axes.map(Into::into));
        self
    }

    /// The input, also reading `action` into the slot `slot`; a slot named
    /// again takes the new action.
    #[must_use]
    pub fn with_slot(mut self, slot: impl Into<String>, action: impl Into<String>) -> Self {
        let (slot, action) = (slot.into(), action.into());
        match self.slots.binary_search_by(|(name, _)| name.cmp(&slot)) {
            Ok(at) => self.slots[at].1 = action,
            Err(at) => self.slots.insert(at, (slot, action)),
        }
        self
    }

    /// Every action it names, with the field that names it, for a load's
    /// check.
    fn actions(&self) -> impl Iterator<Item = (String, &str)> {
        self.move_axes
            .iter()
            .enumerate()
            .map(|(i, action)| (format!("move.{i}"), action.as_str()))
            .chain(self.aim_axes.iter().flat_map(|axes| {
                axes.iter()
                    .enumerate()
                    .map(|(i, action)| (format!("aim.{i}"), action.as_str()))
            }))
            .chain(
                self.slots
                    .iter()
                    .map(|(slot, action)| (format!("slots.{slot}"), action.as_str())),
            )
    }

    /// `(value(right) - value(left), value(down) - value(up))` of `axes`,
    /// clamped to length 1; `y` is 0 with two axes.
    fn axis_value(actions: &ActionMap, input: &InputState, axes: &[String]) -> Vec2 {
        let value = |i: usize| {
            axes.get(i)
                .map_or(0.0, |action| actions.value(input, action))
        };
        let v = Vec2::new(value(1) - value(0), value(3) - value(2));
        if v.length_squared() > 1.0 {
            v.normalize()
        } else {
            v
        }
    }
}

/// What the entity means to do this step: a movement direction of length
/// at most 1, an aim when it aims, and named slots that are held or were
/// just pressed. [`ActionInputPlugin`]'s system writes it from the
/// entity's [`ActionInput`], and a game's AI writes it directly; the
/// movers read it when the entity has one, in place of their own input
/// setters: a [`TopDownMover`](crate::TopDownMover) takes the movement as
/// its direction, and a [`PlatformerController`](crate::PlatformerController)
/// the movement's `x` as its run and the `jump` slot as its jump.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_kit::Intent;
///
/// // An AI that moves right and fires.
/// let mut intent = Intent::default();
/// intent.set_movement(Vec2::new(3.0, 0.0));
/// intent.set_slot("fire", true, true);
/// assert_eq!(intent.movement(), Vec2::new(1.0, 0.0), "clamped to length 1");
/// assert!(intent.pressed("fire") && intent.held("fire") && !intent.held("jump"));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "IntentFields")]
pub struct Intent {
    #[serde(with = "vec2")]
    movement: Vec2,
    #[serde(
        default,
        with = "optional_vec2",
        skip_serializing_if = "Option::is_none"
    )]
    aim: Option<Vec2>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    slots: Vec<Slot>,
}

/// An intent as data writes it, before its directions are clamped and its
/// slots sorted.
#[derive(Deserialize)]
struct IntentFields {
    #[serde(with = "vec2")]
    movement: Vec2,
    #[serde(default, with = "optional_vec2")]
    aim: Option<Vec2>,
    #[serde(default)]
    slots: Vec<Slot>,
}

impl TryFrom<IntentFields> for Intent {
    type Error = String;

    fn try_from(fields: IntentFields) -> Result<Self, Self::Error> {
        let mut slots = fields.slots;
        slots.sort_by(|a, b| a.name.cmp(&b.name));
        if let Some(pair) = slots.windows(2).find(|pair| pair[0].name == pair[1].name) {
            return Err(format!("slots: `{}` is named twice", pair[0].name));
        }
        Ok(Self {
            movement: clamp_direction(fields.movement),
            aim: fields.aim.map(clamp_direction),
            slots,
        })
    }
}

/// A named slot's state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Slot {
    name: String,
    held: bool,
    pressed: bool,
}

impl Intent {
    /// The movement direction, of length at most 1.
    #[must_use]
    pub fn movement(&self) -> Vec2 {
        self.movement
    }

    /// Sets the movement direction; one longer than 1 is normalized, and
    /// one that is not finite stops.
    pub fn set_movement(&mut self, movement: Vec2) {
        self.movement = clamp_direction(movement);
    }

    /// The aim direction, of length at most 1; `None` when the entity does
    /// not aim.
    #[must_use]
    pub fn aim(&self) -> Option<Vec2> {
        self.aim
    }

    /// Sets the aim direction, clamped as [`set_movement`](Self::set_movement)
    /// clamps.
    pub fn set_aim(&mut self, aim: Option<Vec2>) {
        self.aim = aim.map(clamp_direction);
    }

    /// Whether `slot` is held.
    #[must_use]
    pub fn held(&self, slot: &str) -> bool {
        self.slot(slot).is_some_and(|s| s.held)
    }

    /// Whether `slot` was just pressed this step.
    #[must_use]
    pub fn pressed(&self, slot: &str) -> bool {
        self.slot(slot).is_some_and(|s| s.pressed)
    }

    /// Sets `slot`'s state, adding the slot when new.
    pub fn set_slot(&mut self, slot: impl Into<String>, held: bool, pressed: bool) {
        let name = slot.into();
        match self.slots.binary_search_by(|s| s.name.cmp(&name)) {
            Ok(at) => {
                self.slots[at].held = held;
                self.slots[at].pressed = pressed;
            }
            Err(at) => self.slots.insert(
                at,
                Slot {
                    name,
                    held,
                    pressed,
                },
            ),
        }
    }

    fn slot(&self, slot: &str) -> Option<&Slot> {
        self.slots
            .binary_search_by(|s| s.name.as_str().cmp(slot))
            .ok()
            .map(|at| &self.slots[at])
    }
}

/// `direction` of length at most 1; not finite stops.
fn clamp_direction(direction: Vec2) -> Vec2 {
    if !direction.is_finite() {
        Vec2::ZERO
    } else if direction.length_squared() > 1.0 {
        direction.normalize()
    } else {
        direction
    }
}

/// Registers the `action_input` scene component, which spawns
/// `(ActionInput, Intent::default())`, and [`ACTION_INPUT`] in the intent
/// phase: each fixed step it writes every entity's [`Intent`] from its
/// [`ActionInput`] through the `ActionMap`, reading the `RoutedInput` when
/// the resource exists and the `InputState` otherwise; a slot's `pressed`
/// is the step's `just_pressed`. Without the map or any input it writes
/// nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct ActionInputPlugin;

impl Plugin for ActionInputPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<ActionInput>(world, "ActionInput");
        register_rows::<Intent>(world, "Intent");
        describe(
            world,
            ComponentDescription::new(
                ACTION_INPUT_COMPONENT,
                "The actions that steer the entity, read into its Intent each fixed step.",
            )
            .field(FieldDescription::new(
                "move",
                FieldType::List(Box::new(FieldType::Text)),
                "[left, right] or [left, right, up, down]: the movement actions",
            ))
            .field(
                FieldDescription::new(
                    "aim",
                    FieldType::List(Box::new(FieldType::Text)),
                    "[left, right, up, down]: the aim actions",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "slots",
                    FieldType::Object("{ slot: action }"),
                    "named slots, such as `jump`, read as held and just pressed",
                )
                .default("{}"),
            )
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .genre(SHMUP)
            .genre(GRID)
            .sample(r#"{ "move": ["move_left", "move_right"] }"#)
            .roundtrip(round_trip::<ActionInput>),
        );
        world.register_component_with_assets(
            ACTION_INPUT_COMPONENT,
            check_actions,
            |def: ActionInput, _: &SpawnContext<'_>| (def, Intent::default()),
        );
        schedule.add(
            Phase::Intent.stage(),
            Phase::Intent.join(system(ACTION_INPUT, action_input)),
        );
    }
}

/// Every action `input` names is one the `ActionMap` declares; a World
/// without the map passes.
fn check_actions(input: &ActionInput, world: &World) -> Result<(), MissingAsset> {
    let Some(actions) = world.get_resource::<ActionMap>() else {
        return Ok(());
    };
    for (field, action) in input.actions() {
        if !actions.iter().any(|(name, _)| name == action) {
            return Err(MissingAsset::new(field, "action", action));
        }
    }
    Ok(())
}

/// Writes every `Intent` from its `ActionInput`. The map and the input
/// leave the World for the pass and go back after it, never cloned.
fn action_input(world: &mut World) {
    let Some(actions) = world.remove_resource::<ActionMap>() else {
        return;
    };
    let routed = world.remove_resource::<RoutedInput>();
    let raw = if routed.is_none() {
        world.remove_resource::<InputState>()
    } else {
        None
    };
    if let Some(input) = routed.as_deref().or(raw.as_ref()) {
        for (source, intent) in world.query_mut::<(&ActionInput, &mut Intent)>() {
            intent.movement = ActionInput::axis_value(&actions, input, &source.move_axes);
            intent.aim = source
                .aim_axes
                .as_ref()
                .map(|axes| ActionInput::axis_value(&actions, input, axes));
            for (slot, action) in &source.slots {
                let held = actions.is_pressed(input, action);
                let pressed = actions.just_pressed(input, action);
                intent.set_slot(slot.as_str(), held, pressed);
            }
        }
    }
    world.insert_resource(actions);
    if let Some(routed) = routed {
        world.insert_resource(routed);
    }
    if let Some(raw) = raw {
        world.insert_resource(raw);
    }
}

impl Inspectable for ActionInput {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let mut rows = vec![("move", self.move_axes.join(", "))];
        if let Some(axes) = &self.aim_axes {
            rows.push(("aim", axes.join(", ")));
        }
        if !self.slots.is_empty() {
            let slots: Vec<String> = self
                .slots
                .iter()
                .map(|(slot, action)| format!("{slot} = {action}"))
                .collect();
            rows.push(("slots", slots.join(", ")));
        }
        rows
    }
}

impl Inspectable for Intent {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let mut rows = vec![("movement", point(self.movement))];
        if let Some(aim) = self.aim {
            rows.push(("aim", point(aim)));
        }
        for slot in &self.slots {
            let state = match (slot.held, slot.pressed) {
                (true, true) => "pressed, held",
                (true, false) => "held",
                (false, true) => "pressed",
                (false, false) => "up",
            };
            rows.push(("slot", format!("{} {state}", slot.name)));
        }
        rows
    }
}

#[cfg(test)]
#[path = "../tests/action_input.rs"]
mod tests;
