//! The interactor and the interactable (redo 8.1, P7-3): press to act on
//! the nearest thing in reach, with a prompt over it from the action's
//! binding.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::text::TextOutline;
use tungsten_core::ui::{InputDevice, PromptText, UiRouter};
use tungsten_core::{
    ActionMap, Entity, EventQueue, Inspectable, Plugin, Schedule, SpawnContext, World, WorldLabel,
    WorldLabels, system,
};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, RPG, TOP_DOWN, describe, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::saved::order_of;
use crate::foundation::tags::{TagFilter, TagFilterDef, Tags, check_declared};
use crate::foundation::trace::{TraceKind, trace};
use crate::intent::action_input::{ACTION_INPUT, ActionInput, Intent};
use crate::lifecycle::ended::Despawning;
use crate::motion::drive::centre_of;
use crate::present::labels::srgb_byte;
use crate::story::condition::{Condition, describe_conditions};

/// [`InteractPlugin`]'s focus and press system, in the intent phase.
pub const INTERACT: &str = "interact";
/// [`InteractPlugin`]'s prompt system, in the present phase.
pub const INTERACT_PROMPT: &str = "interact_prompt";

/// The scene components the plugin registers.
const INTERACTOR: &str = "interactor";
const INTERACTABLE: &str = "interactable";

/// The prompt's outline: near black, 1 UI unit.
const OUTLINE: TextOutline = TextOutline::new([12, 12, 16, 255], 1.0);

/// Whoever interacts: the player, or a game's AI through its [`Intent`].
/// [`InteractPlugin`]'s system gives it a focus each fixed step, the
/// nearest [`Interactable`] whose centre lies within its reach plus the
/// interactable's radius of its own, whose `who` passes its tags and whose
/// `when` holds, and sends an [`Interacted`] once a press of its `Intent`
/// slot while it has one. Its prompt, the focus's verb after the binding
/// of the action its [`ActionInput`] reads into that slot, floats over the
/// focus once a frame.
///
/// Scene name `interactor`:
/// `{ "slot": "interact", "reach": 48, "font": "engine_mono", "size": 14, "color": [1, 1, 1, 1] }`,
/// every field optional with those defaults; the colour is linear.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::{EventQueue, Plugin, Schedule, Stage, Time, Transform, World};
/// use tungsten_kit::{Interactable, Interacted, InteractPlugin, Interactor, Intent};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// InteractPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let at = |x: f32| Transform::from_position(Vec2::new(x, 0.0));
/// let warden = world.spawn_with((at(0.0), Interactor::new(), Intent::default()));
/// let keeper = world.spawn_with((at(40.0), Interactable::new("Talk")));
/// let _far = world.spawn_with((at(90.0), Interactable::new("Read")));
/// world.get_mut::<Intent>(warden).unwrap().set_slot("interact", true, true);
/// world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
/// schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// assert_eq!(world.get::<Interactor>(warden).unwrap().focus(), Some(keeper));
/// let sent: Vec<Interacted> =
///     world.resource::<EventQueue<Interacted>>().iter_current().copied().collect();
/// assert_eq!((sent[0].interactable, sent[0].interactor), (keeper, warden));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "InteractorDef", into = "InteractorDef")]
pub struct Interactor {
    slot: String,
    reach: f32,
    font: String,
    size: f32,
    color: [f32; 4],
    /// The interactable it would use now, found again each fixed step.
    focus: Option<Entity>,
    /// The prompt over the focus it was built for.
    prompt: Option<(Entity, PromptText)>,
}

impl Default for Interactor {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for Interactor {
    /// Equal settings; the focus and the prompt are found again.
    fn eq(&self, other: &Self) -> bool {
        self.slot == other.slot
            && self.reach == other.reach
            && self.font == other.font
            && self.size == other.size
            && self.color == other.color
    }
}

impl Interactor {
    /// An interactor pressing slot `interact`, reaching 48 px, its prompt
    /// in white `engine_mono` at 14 UI units.
    #[must_use]
    pub fn new() -> Self {
        Self {
            slot: "interact".to_owned(),
            reach: 48.0,
            font: "engine_mono".to_owned(),
            size: 14.0,
            color: [1.0, 1.0, 1.0, 1.0],
            focus: None,
            prompt: None,
        }
    }

    /// The interactor, pressing `slot` instead.
    #[must_use]
    pub fn with_slot(mut self, slot: impl Into<String>) -> Self {
        self.slot = slot.into();
        self
    }

    /// The interactor, reaching `reach` px from its centre.
    ///
    /// # Panics
    ///
    /// When `reach` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_reach(mut self, reach: f32) -> Self {
        assert!(
            reach.is_finite() && reach >= 0.0,
            "Interactor::with_reach: {reach} is not finite and at least 0"
        );
        self.reach = reach;
        self
    }

    /// The interactor, its prompt in face `font` at `size` UI units in the
    /// linear colour `color`.
    #[must_use]
    pub fn with_prompt_style(
        mut self,
        font: impl Into<String>,
        size: f32,
        color: [f32; 4],
    ) -> Self {
        self.font = font.into();
        self.size = size;
        self.color = color;
        self
    }

    /// The slot it presses.
    #[must_use]
    pub fn slot(&self) -> &str {
        &self.slot
    }

    /// Its reach in px.
    #[must_use]
    pub fn reach(&self) -> f32 {
        self.reach
    }

    /// The interactable it would use now, from the last fixed step.
    #[must_use]
    pub fn focus(&self) -> Option<Entity> {
        self.focus
    }
}

/// `components.interactor` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InteractorDef {
    #[serde(default = "default_slot")]
    slot: String,
    #[serde(default = "default_reach")]
    reach: f32,
    #[serde(default = "default_font")]
    font: String,
    #[serde(default = "default_size")]
    size: f32,
    #[serde(default = "default_color")]
    color: [f32; 4],
}

fn default_slot() -> String {
    "interact".to_owned()
}

fn default_reach() -> f32 {
    48.0
}

fn default_font() -> String {
    "engine_mono".to_owned()
}

fn default_size() -> f32 {
    14.0
}

fn default_color() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

impl TryFrom<InteractorDef> for Interactor {
    type Error = String;

    fn try_from(def: InteractorDef) -> Result<Self, Self::Error> {
        if def.slot.is_empty() {
            return Err("slot: an intent slot's name".into());
        }
        if !(def.reach.is_finite() && def.reach >= 0.0) {
            return Err(format!("reach: {} is not finite and at least 0", def.reach));
        }
        if !(def.size.is_finite() && def.size > 0.0) {
            return Err(format!("size: {} is not finite and above 0", def.size));
        }
        if def
            .color
            .iter()
            .any(|channel| !(0.0..=1.0).contains(channel))
        {
            return Err(format!(
                "color: {:?} has a channel outside 0 to 1",
                def.color
            ));
        }
        Ok(Self {
            slot: def.slot,
            reach: def.reach,
            font: def.font,
            size: def.size,
            color: def.color,
            focus: None,
            prompt: None,
        })
    }
}

impl From<Interactor> for InteractorDef {
    fn from(interactor: Interactor) -> Self {
        Self {
            slot: interactor.slot,
            reach: interactor.reach,
            font: interactor.font,
            size: interactor.size,
            color: interactor.color,
        }
    }
}

/// Something an [`Interactor`] can act on: a person to talk to, a lamp to
/// light, a notice to read. It takes the focus of an interactor in reach
/// whose tags pass `who` while its `when` holds, the interactable the
/// condition's subject and the interactor its other; a press then sends an
/// [`Interacted`], which its `interacted` cues read.
///
/// Scene name `interactable`:
/// `{ "verb": "Talk", "radius": 0, "offset": [0, -16], "who": { "all": ["team.player"] }, "when": { "flag": "lit" } }`,
/// `verb` required: the prompt's word, a radius added to the interactor's
/// reach, the prompt's shift in UI units off its centre (y down), a tag
/// filter on the interactor and a [`Condition`]; the load checks the
/// filter's tags and the condition's names.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_kit::{Condition, Interactable, TagFilter};
///
/// let lamp = Interactable::new("Light")
///     .with_radius(8.0)
///     .with_offset(Vec2::new(0.0, -24.0))
///     .with_who(TagFilter::new().all(["team.player"]))
///     .with_when(Condition::not(Condition::flag("lamp.lit").for_game()));
/// assert_eq!(lamp.verb(), "Light");
/// let data = serde_json::to_value(&lamp).unwrap();
/// assert_eq!(data["when"], serde_json::json!({ "not": { "flag": { "name": "lamp.lit", "target": "game" } } }));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "InteractableFields", into = "InteractableFields")]
pub struct Interactable {
    verb: String,
    radius: f32,
    offset: Vec2,
    who: TagFilter,
    when: Option<Condition>,
}

impl Interactable {
    /// An interactable that any interactor in reach may use, its prompt
    /// reading `verb` 16 UI units above its centre.
    #[must_use]
    pub fn new(verb: impl Into<String>) -> Self {
        Self {
            verb: verb.into(),
            radius: 0.0,
            offset: Vec2::new(0.0, -16.0),
            who: TagFilter::new(),
            when: None,
        }
    }

    /// The interactable, its radius added to an interactor's reach.
    ///
    /// # Panics
    ///
    /// When `radius` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_radius(mut self, radius: f32) -> Self {
        assert!(
            radius.is_finite() && radius >= 0.0,
            "Interactable::with_radius: {radius} is not finite and at least 0"
        );
        self.radius = radius;
        self
    }

    /// The interactable, its prompt shifted by `offset` UI units, y down.
    #[must_use]
    pub fn with_offset(mut self, offset: Vec2) -> Self {
        self.offset = offset;
        self
    }

    /// The interactable, used only by interactors whose tags pass `who`.
    #[must_use]
    pub fn with_who(mut self, who: TagFilter) -> Self {
        self.who = who;
        self
    }

    /// The interactable, offered only while `when` holds.
    #[must_use]
    pub fn with_when(mut self, when: Condition) -> Self {
        self.when = Some(when);
        self
    }

    /// The prompt's word.
    #[must_use]
    pub fn verb(&self) -> &str {
        &self.verb
    }

    /// Whether `interactor` may use it now: its tags pass `who` and `when`
    /// holds, the interactable the subject.
    fn offered(&self, world: &World, me: Entity, interactor: Entity) -> bool {
        self.who.matches(world.get::<Tags>(interactor))
            && self
                .when
                .as_ref()
                .is_none_or(|when| when.holds(world, Some(me), Some(interactor)))
    }
}

/// An interactable's fields, as its data form writes them.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InteractableFields {
    verb: String,
    #[serde(default)]
    radius: f32,
    #[serde(default = "default_offset")]
    offset: [f32; 2],
    #[serde(default)]
    who: TagFilter,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    when: Option<Condition>,
}

fn default_offset() -> [f32; 2] {
    [0.0, -16.0]
}

/// The checks every form of an interactable shares.
fn check_fields(verb: &str, radius: f32, offset: [f32; 2]) -> Result<(), String> {
    if verb.is_empty() {
        return Err("verb: the prompt's word".into());
    }
    if !(radius.is_finite() && radius >= 0.0) {
        return Err(format!("radius: {radius} is not finite and at least 0"));
    }
    if offset.iter().any(|v| !v.is_finite()) {
        return Err("offset: not finite".into());
    }
    Ok(())
}

impl TryFrom<InteractableFields> for Interactable {
    type Error = String;

    fn try_from(fields: InteractableFields) -> Result<Self, Self::Error> {
        check_fields(&fields.verb, fields.radius, fields.offset)?;
        Ok(Self {
            verb: fields.verb,
            radius: fields.radius,
            offset: Vec2::from(fields.offset),
            who: fields.who,
            when: fields.when,
        })
    }
}

impl From<Interactable> for InteractableFields {
    fn from(interactable: Interactable) -> Self {
        Self {
            verb: interactable.verb,
            radius: interactable.radius,
            offset: interactable.offset.into(),
            who: interactable.who,
            when: interactable.when,
        }
    }
}

/// `components.interactable` as a scene writes it: its filter kept as
/// written, so the load's check names the field as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, try_from = "InteractableSceneFields")]
struct InteractableDef(InteractableSceneFields);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InteractableSceneFields {
    verb: String,
    #[serde(default)]
    radius: f32,
    #[serde(default = "default_offset")]
    offset: [f32; 2],
    #[serde(default)]
    who: TagFilterDef,
    #[serde(default)]
    when: Option<Condition>,
}

impl TryFrom<InteractableSceneFields> for InteractableDef {
    type Error = String;

    fn try_from(fields: InteractableSceneFields) -> Result<Self, Self::Error> {
        check_fields(&fields.verb, fields.radius, fields.offset)?;
        Ok(Self(fields))
    }
}

impl From<InteractableDef> for Interactable {
    fn from(InteractableDef(fields): InteractableDef) -> Self {
        Self {
            verb: fields.verb,
            radius: fields.radius,
            offset: Vec2::from(fields.offset),
            who: fields.who.into(),
            when: fields.when,
        }
    }
}

/// An [`Interactor`] pressed its slot while focused on an [`Interactable`],
/// sent by [`InteractPlugin`]'s system once a press; the cue event
/// `interacted`, about the interactable, the interactor the other. Build
/// one with [`Interacted::new`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Interacted {
    /// The interactable used.
    pub interactable: Entity,
    /// The interactor that used it.
    pub interactor: Entity,
    /// The interactable's centre.
    pub position: Vec2,
}

impl Interacted {
    /// An event with these fields, as the interact system sends one.
    #[must_use]
    pub fn new(interactable: Entity, interactor: Entity, position: Vec2) -> Self {
        Self {
            interactable,
            interactor,
            position,
        }
    }
}

/// Registers `EventQueue<Interacted>`, the `interactor` and `interactable`
/// scene components, [`INTERACT`] in the intent phase, after
/// `action_input`: for each interactor in the kit's step order, its focus
/// from one scan of the interactables a step, ties to the earlier
/// interactable in that order, and an [`Interacted`] for a pressed slot
/// with a focus; and [`INTERACT_PROMPT`] in the present phase, a world
/// label over each live focus. Both do nothing in a World with no
/// interactor.
#[derive(Debug, Default, Clone, Copy)]
pub struct InteractPlugin;

impl Plugin for InteractPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Interactor>(world, "Interactor");
        register_rows::<Interactable>(world, "Interactable");
        describe_interact(world);
        world.register_event::<Interacted>();
        world.register_component_with(
            INTERACTOR,
            |interactor: Interactor, _: &SpawnContext<'_>| (interactor,),
        );
        world.register_component_with_assets(
            INTERACTABLE,
            |def: &InteractableDef, world: &World| {
                for (field, name) in def.0.who.names() {
                    check_declared(world, &format!("who.{field}"), name)?;
                }
                match &def.0.when {
                    Some(when) => when.check(world, "when"),
                    None => Ok(()),
                }
            },
            |def: InteractableDef, _: &SpawnContext<'_>| (Interactable::from(def),),
        );
        let mut scan = Scan::default();
        schedule.add(
            Phase::Intent.stage(),
            Phase::Intent.join(
                system(INTERACT, move |world: &mut World| {
                    interact(world, &mut scan);
                })
                .after_if_present(ACTION_INPUT),
            ),
        );
        let mut held = Vec::new();
        schedule.add(
            Phase::Present.stage(),
            Phase::Present.join(system(INTERACT_PROMPT, move |world: &mut World| {
                interact_prompt(world, &mut held);
            })),
        );
    }
}

/// The interact system's scratch lists, kept so a warm step allocates
/// nothing.
#[derive(Default)]
struct Scan {
    /// Each live interactable: its entity, centre, radius and order key.
    targets: Vec<(Entity, Vec2, f32, (bool, u64))>,
    /// Each interactor, with its order key.
    interactors: Vec<(Entity, (bool, u64))>,
    /// The step's presses.
    sent: Vec<Interacted>,
}

/// Each interactor's focus and press, in [`order_of`]'s order.
fn interact(world: &mut World, scan: &mut Scan) {
    if !world.has_any::<Interactor>() {
        return;
    }
    let order = order_of(world);
    scan.targets.clear();
    scan.targets.extend(
        world
            .query::<(Entity, &Interactable)>()
            .filter(|&(entity, _)| !world.has::<Despawning>(entity))
            .filter_map(|(entity, interactable)| {
                let centre = centre_of(world, entity)?;
                Some((entity, centre, interactable.radius, order.key(entity)))
            }),
    );
    scan.targets.sort_by_key(|&(.., key)| key);
    scan.interactors.clear();
    scan.interactors.extend(
        world
            .query::<(Entity, &Interactor)>()
            .map(|(entity, _)| (entity, order.key(entity))),
    );
    scan.interactors.sort_by_key(|&(_, key)| key);
    scan.sent.clear();
    for i in 0..scan.interactors.len() {
        let me = scan.interactors[i].0;
        let focus = focus_of(world, me, &scan.targets);
        let pressed = focus.is_some() && {
            let slot = &world.get::<Interactor>(me).expect("queried").slot;
            world
                .get::<Intent>(me)
                .is_some_and(|intent| intent.pressed(slot))
        };
        world.get_mut::<Interactor>(me).expect("queried").focus = focus.map(|(entity, _)| entity);
        if pressed && let Some((target, at)) = focus {
            scan.sent.push(Interacted::new(target, me, at));
        }
    }
    if scan.sent.is_empty() {
        return;
    }
    for event in &scan.sent {
        trace(world, event.interactable, || TraceKind::Event {
            name: "interacted",
            other: Some(event.interactor),
            amount: None,
        });
    }
    if let Some(queue) = world.get_resource_mut::<EventQueue<Interacted>>() {
        for event in scan.sent.drain(..) {
            queue.send(event);
        }
    }
}

/// `me`'s focus among `targets`, sorted by order key: the nearest in reach
/// that it may use, the earliest of equals; with its centre.
fn focus_of(
    world: &World,
    me: Entity,
    targets: &[(Entity, Vec2, f32, (bool, u64))],
) -> Option<(Entity, Vec2)> {
    let reach = world.get::<Interactor>(me)?.reach;
    let centre = centre_of(world, me)?;
    let mut best: Option<(f32, Entity, Vec2)> = None;
    for &(target, at, radius, _) in targets {
        if target == me {
            continue;
        }
        let within = reach + radius;
        let distance = at.distance_squared(centre);
        if distance > within * within || best.is_some_and(|(near, ..)| distance >= near) {
            continue;
        }
        let offered = world
            .get::<Interactable>(target)
            .is_some_and(|interactable| interactable.offered(world, target, me));
        if offered {
            best = Some((distance, target, at));
        }
    }
    best.map(|(_, target, at)| (target, at))
}

/// Pushes each interactor's prompt over its live focus to the frame's
/// `WorldLabels`, rebuilding its text when the focus moved; `held` is the
/// scratch list of interactors.
fn interact_prompt(world: &mut World, held: &mut Vec<Entity>) {
    if !world.has_any::<Interactor>() || !world.has_resource::<WorldLabels>() {
        return;
    }
    let device = world
        .get_resource::<UiRouter>()
        .map_or(InputDevice::Keyboard, UiRouter::last_device);
    held.clear();
    held.extend(
        world
            .query::<(Entity, &Interactor)>()
            .map(|(entity, _)| entity),
    );
    for &me in held.iter() {
        let Some(focus) = world
            .get::<Interactor>(me)
            .and_then(Interactor::focus)
            .filter(|&focus| world.is_alive(focus))
        else {
            continue;
        };
        let (Some(interactable), Some(anchor)) =
            (world.get::<Interactable>(focus), centre_of(world, focus))
        else {
            continue;
        };
        let offset = interactable.offset;
        let stale = world
            .get::<Interactor>(me)
            .and_then(|interactor| interactor.prompt.as_ref())
            .is_none_or(|(built, _)| *built != focus);
        let built = if stale {
            let slot = &world.get::<Interactor>(me).expect("queried").slot;
            let action = slot_action(world, me, slot).unwrap_or_else(|| slot.clone());
            let template = format!("{{action:{action}}} {}", interactable.verb);
            (focus, PromptText::new(&template))
        } else {
            world
                .get_mut::<Interactor>(me)
                .and_then(|interactor| interactor.prompt.take())
                .expect("fresh")
        };
        let (focus, mut prompt) = built;
        let text = prompt
            .text(world.get_resource::<ActionMap>(), device)
            .to_owned();
        let interactor = world.get_mut::<Interactor>(me).expect("queried");
        interactor.prompt = Some((focus, prompt));
        let [r, g, b, a] = interactor.color;
        let label = WorldLabel::new(text, anchor, interactor.font.clone(), interactor.size)
            .with_offset(offset)
            .with_pivot(Vec2::new(0.5, 1.0))
            .with_color([
                srgb_byte(r),
                srgb_byte(g),
                srgb_byte(b),
                (a.clamp(0.0, 1.0) * 255.0).round() as u8,
            ])
            .with_outline(OUTLINE);
        world.resource_mut::<WorldLabels>().push(label);
    }
}

/// The action `entity`'s [`ActionInput`] reads into `slot`, from its data
/// form, which names its slots; `None` without one or the slot.
fn slot_action(world: &World, entity: Entity, slot: &str) -> Option<String> {
    let input = world.get::<ActionInput>(entity)?;
    let data = serde_json::to_value(input).ok()?;
    data.get("slots")?.get(slot)?.as_str().map(str::to_owned)
}

/// The descriptions of `interactor`, `interactable` and the condition
/// shapes, which the interactable's `when` names first.
fn describe_interact(world: &mut World) {
    describe(
        world,
        ComponentDescription::new(
            INTERACTOR,
            "Whoever interacts: a press of its intent slot uses the nearest interactable in reach.",
        )
        .field(
            FieldDescription::new("slot", FieldType::Text, "the intent slot it presses")
                .default(r#""interact""#),
        )
        .field(
            FieldDescription::new(
                "reach",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "px from its centre to an interactable's, plus that one's radius",
            )
            .default("48.0"),
        )
        .field(
            FieldDescription::new("font", FieldType::Text, "the prompt's face or family ID")
                .default(r#""engine_mono""#),
        )
        .field(
            FieldDescription::new(
                "size",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "the prompt's size in UI units",
            )
            .default("14.0"),
        )
        .field(
            FieldDescription::new("color", FieldType::Color, "the prompt's linear colour")
                .default("[1.0, 1.0, 1.0, 1.0]"),
        )
        .event("Interacted")
        .genre(TOP_DOWN)
        .genre(RPG)
        .sample("{}")
        .roundtrip(round_trip::<Interactor>),
    );
    describe(
        world,
        ComponentDescription::new(
            INTERACTABLE,
            "Something an interactor in reach uses with a press, its prompt the verb after the binding.",
        )
        .field(FieldDescription::new(
            "verb",
            FieldType::Text,
            "the prompt's word after the binding: `Talk`, `Read`",
        ))
        .field(
            FieldDescription::new(
                "radius",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "px added to an interactor's reach",
            )
            .default("0.0"),
        )
        .field(
            FieldDescription::new(
                "offset",
                FieldType::Vec2,
                "the prompt's shift in UI units off its centre, y down",
            )
            .default("[0.0, -16.0]"),
        )
        .field(
            FieldDescription::new(
                "who",
                FieldType::Filter,
                "the interactors whose tags pass; the load checks its tags",
            )
            .default("{}"),
        )
        .field(
            FieldDescription::new(
                "when",
                FieldType::Shape("condition"),
                "offered only while it holds, the interactable the subject, the interactor the other",
            )
            .default("null"),
        )
        .event("Interacted")
        .genre(TOP_DOWN)
        .genre(RPG)
        .sample(r#"{ "verb": "Talk" }"#)
        .roundtrip(round_trip::<Interactable>),
    );
    describe_conditions(world);
}

impl Inspectable for Interactor {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![
            ("slot", self.slot.clone()),
            ("reach", format!("{:.0}", self.reach)),
            (
                "focus",
                self.focus
                    .map_or_else(|| "none".to_owned(), |focus| format!("{focus:?}")),
            ),
        ]
    }
}

impl Inspectable for Interactable {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![
            ("verb", self.verb.clone()),
            ("radius", format!("{:.0}", self.radius)),
            ("who", self.who.summary()),
            (
                "when",
                if self.when.is_some() { "set" } else { "none" }.to_owned(),
            ),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/interact.rs"]
mod tests;
