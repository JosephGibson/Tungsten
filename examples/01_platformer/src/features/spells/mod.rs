//! The spells: ten slots, each cast with `cast` through one of the
//! player's kit abilities, whose data holds its cost in mana, its cooldown,
//! its charges and the `casting` effect that pauses the mana's
//! regeneration, over the toys' rules. The slot actions and the wheel
//! choose one; the fire missile flies from the kit's spawn payload; slot 0
//! summons the type `summon_next` and `summon_prev` pick. The HUD shows the
//! health, the mana, the bar and the gun's magazine.

pub mod components;
pub mod hud;
pub mod systems;

use tungsten::Plugin;
use tungsten::core::{DataRegistry, Entity, Schedule, Stage, World, system};
use tungsten::kit::{ABILITY_TICK, Ability, Phase, Stats, ensure_phases};

use components::{Spell, Spellbook};

use crate::features::enemies::components::EnemyKind;

/// The spell bar's slots, keys 1 to 9 then 0.
pub const SLOTS: [Option<Spell>; 10] = [
    Some(Spell::FireMissile),
    Some(Spell::Flamethrower),
    Some(Spell::IceSpray),
    Some(Spell::Lightning),
    Some(Spell::ForcePush),
    Some(Spell::BlackHole),
    Some(Spell::IronBlock),
    Some(Spell::BowlingBall),
    Some(Spell::SmallBalls),
    Some(Spell::Summon),
];
/// The action that selects each slot, and the key the bar labels it with.
pub const SLOT_ACTIONS: [&str; 10] = [
    "spell_1", "spell_2", "spell_3", "spell_4", "spell_5", "spell_6", "spell_7", "spell_8",
    "spell_9", "spell_0",
];
pub const SLOT_KEYS: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
/// Casts the selected spell: once a press, or each fixed step it is held.
pub const CAST: &str = "cast";
/// Select the next or the previous spell, wrapping past the empty slots.
pub const NEXT: &str = "spell_next";
pub const PREV: &str = "spell_prev";
/// Pick the next or the previous summon type, wrapping, whichever slot is
/// selected.
pub const SUMMON_NEXT: &str = "summon_next";
pub const SUMMON_PREV: &str = "summon_prev";

/// A type the summon slot places: its name, its prefab, its icon and, for
/// an enemy, its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummonType {
    pub name: &'static str,
    pub prefab: &'static str,
    pub icon: &'static str,
    pub enemy: Option<EnemyKind>,
}

/// An enemy's summon type.
const fn enemy(kind: EnemyKind) -> SummonType {
    let info = kind.info();
    SummonType {
        name: kind.name(),
        prefab: info.prefab,
        icon: info.icon,
        enemy: Some(kind),
    }
}

/// The types slot 0 summons: the training dummy, then each enemy as its
/// kind lands.
pub const SUMMONS: [SummonType; 6] = [
    SummonType {
        name: "dummy",
        prefab: crate::features::dummy::DUMMY_PREFAB,
        icon: "ex10_icon_dummy",
        enemy: None,
    },
    enemy(EnemyKind::Wisp),
    enemy(EnemyKind::Crawler),
    enemy(EnemyKind::Beetle),
    enemy(EnemyKind::Bat),
    enemy(EnemyKind::Troll),
];

/// The type the summon slot places now.
#[must_use]
pub fn summon_type(world: &World) -> SummonType {
    let selected = world
        .get_resource::<components::Summon>()
        .map_or(0, |summon| summon.selected);
    SUMMONS[selected % SUMMONS.len()]
}
/// The fixed steps in a second, which a held spell's cost assumes: its
/// data's cost is one step's, its cost per second over this.
pub const STEPS_PER_SECOND: f32 = 60.0;

/// The spells as a plugin: the `spellbook` its prefab names; in the intent
/// phase after the kit's ability tick, the choice and the cast; in
/// `PreUpdate` before the shell's save, the missiles the last flush landed
/// taken in; in `Update`, the sprays a frame between two steps starts or
/// ends, and the HUD.
pub struct SpellsPlugin;

impl Plugin for SpellsPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.register_component::<Spellbook>("spellbook");
        ensure_phases(schedule);
        let cast = system("spells_cast", systems::cast).after(ABILITY_TICK);
        schedule.add(Stage::FixedUpdate, Phase::Intent.join(cast));
        // Before the shell's save captures them, so a save holds them whole.
        let adopt = system("missile_adopt", systems::adopt).before("shell_save");
        schedule.add(Stage::PreUpdate, adopt);
        schedule.add(Stage::Update, system("spells_held", systems::held));
        schedule.add(
            Stage::Update,
            system("spells_hud", hud::update).after("spells_held"),
        );
    }
}

/// The spell in `player`'s selected slot.
#[must_use]
pub fn selected(world: &World, player: Entity) -> Option<Spell> {
    let slot = world.get::<Spellbook>(player)?.slot;
    SLOTS.get(slot).copied().flatten()
}

/// Selects `slot`, 0 for key 1 to 9 for key 0, when it holds a spell.
/// Whether it did.
pub fn select(world: &mut World, player: Entity, slot: usize) -> bool {
    let filled = SLOTS.get(slot).is_some_and(Option::is_some);
    if let Some(book) = world.get_mut::<Spellbook>(player).filter(|_| filled) {
        book.slot = slot;
        return true;
    }
    false
}

/// What one use of `spell` costs, in mana: its ability's data.
#[must_use]
pub fn cost(world: &World, spell: Spell) -> Option<f32> {
    let ability = world
        .get_resource::<DataRegistry>()?
        .get::<Ability>(spell.ability())?;
    ability.cost().map(|(_, amount)| amount)
}

/// The mana `player` holds.
#[must_use]
pub fn mana(world: &World, player: Entity) -> f32 {
    world
        .get::<Stats>(player)
        .and_then(|stats| stats.base("mana"))
        .unwrap_or(0.0)
}

/// Builds the HUD for the run gameplay starts, owned by gameplay.
pub fn start(world: &mut World) {
    hud::start(world);
}

/// Forgets the run's HUD, whose roots leave with gameplay.
pub fn stop(world: &mut World) {
    world.remove_resource::<hud::Hud>();
}
