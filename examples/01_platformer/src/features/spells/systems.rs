//! The spells' systems: the choice and the cast in the fixed step, the
//! sprays a frame between two steps starts or ends, and the missiles the
//! spawn payload lands.

use glam::Vec2;
use tungsten::core::{Entity, Time, With, Without, World};
use tungsten::kit::{Aim, Health, Used, cursor_to_world, use_ability};

use super::components::{Refusal, Refused, Spell, Spellbook, Summon};
use super::{
    CAST, NEXT, PREV, SLOT_ACTIONS, SLOTS, SUMMON_NEXT, SUMMON_PREV, SUMMONS, Summoned, cost, mana,
    select, selected, summon_type,
};
use crate::features::audio::{self, components::Sound};
use crate::features::lightning;
use crate::features::player::{self, BODY_OFFSET};
use crate::features::run::Run;
use crate::features::toys::components::{FireMissile, Spray};
use crate::features::toys::{self, BALL_CAP, MISSILE_CAP};
use crate::features::{dummy, enemies, props};
use crate::game::GAMEPLAY;
use crate::shared::{SaveId, just_pressed, pressed, spawn_key_burst};
use crate::shell;

/// The run's player while it lives.
fn caster(world: &World) -> Option<Entity> {
    let player = world.get_resource::<Run>()?.player;
    let dead = world.get::<Health>(player).is_some_and(Health::is_dead);
    (world.is_alive(player) && !dead).then_some(player)
}

/// The direction `player` aims in: toward the cursor.
fn aim(world: &World, player: Entity) -> Vec2 {
    world
        .get::<Aim>(player)
        .and_then(Aim::direction)
        .unwrap_or(Vec2::X)
}

/// `spells_cast`, in the intent phase after the kit's ability tick: the
/// slot actions and the wheel choose a spell, then `cast` uses it, once a
/// press or each step it is held, as its ability allows. A held spell let
/// go, or one its ability refuses, stops: a spray no step keeps ends in the
/// react phase, and the small balls forget what they owed and stop their
/// loop, spending nothing at the balls' cap. One refused for mana stays off
/// until the cast is released (I2). A held spell's refusal is said on the
/// step it is first refused, once a press ([`refuse_cast`]).
pub fn cast(world: &mut World) {
    let Some(player) = caster(world) else {
        toys::stop_small_balls(world);
        return;
    };
    choose(world, player);
    let held = pressed(world, CAST);
    if !held {
        released(world);
    }
    let mut placing = false;
    match selected(world, player) {
        Some(spell) if spell.held() => {
            // The small balls' cap refuses before the spend (I1).
            let capped = spell == Spell::SmallBalls && toys::ball_count(world) >= BALL_CAP;
            if held && !refused(world) {
                let refusal = if capped {
                    Some(Refusal::NoRoom)
                } else {
                    let used = hold(world, player, spell);
                    if used == Used::NoCost {
                        refuse(world);
                    }
                    placing = used == Used::Used && spell == Spell::SmallBalls;
                    Refusal::of(used)
                };
                if let Some(refusal) = refusal {
                    say(world, player, refusal);
                }
            }
        }
        Some(spell) if just_pressed(world, CAST) => press(world, player, spell),
        _ => {}
    }
    if !placing {
        toys::stop_small_balls(world);
    }
}

/// Says why a cast was refused at `at`, the cursor's point or else the
/// courier's: no mana and not ready each play their sound, and no room
/// fizzles there with its own, a key burst.
pub fn refuse_cast(world: &mut World, refusal: Refusal, at: Vec2) {
    match refusal {
        Refusal::NoMana => audio::play(world, Sound::NoMana, at),
        Refusal::NotReady => audio::play(world, Sound::NotReady, at),
        Refusal::NoRoom => {
            spawn_key_burst(world, "ex10_fizzle", at);
            audio::play(world, Sound::Fizzled, at);
        }
    }
}

/// Says a held spell's refusal, at the cursor or else the courier, unless
/// one was said since the press.
fn say(world: &mut World, player: Entity, refusal: Refusal) {
    let said = world.get_resource::<Refused>().is_some_and(|r| r.said);
    if said {
        return;
    }
    if let Some(refused) = world.get_resource_mut::<Refused>() {
        refused.said = true;
    }
    let at = cursor_to_world(world).or_else(|| player::centre(world, player));
    refuse_cast(world, refusal, at.unwrap_or_default());
}

/// `summon_next` and `summon_prev` turn the summon's type, wrapping; the
/// slot actions select their slot; `spell_next` and `spell_prev` step to
/// the next or the previous spell, wrapping past the empty slots. A change
/// of the slot or the summon's type clicks.
fn choose(world: &mut World, player: Entity) {
    let before = choice(world, player);
    pick(world, player);
    if choice(world, player) != before
        && let Some(at) = player::centre(world, player)
    {
        audio::play(world, Sound::Switched, at);
    }
}

/// The selected slot and the summon's type.
fn choice(world: &World, player: Entity) -> (Option<usize>, Option<usize>) {
    (
        world.get::<Spellbook>(player).map(|book| book.slot),
        world.get_resource::<Summon>().map(|summon| summon.selected),
    )
}

/// The choice's actions, as [`choose`] reads them.
fn pick(world: &mut World, player: Entity) {
    let turn = match (
        just_pressed(world, SUMMON_NEXT),
        just_pressed(world, SUMMON_PREV),
    ) {
        (true, false) => 1,
        (false, true) => SUMMONS.len() - 1,
        _ => 0,
    };
    if let Some(summon) = world.get_resource_mut::<Summon>() {
        summon.selected = (summon.selected + turn) % SUMMONS.len();
    }
    if let Some(slot) = SLOT_ACTIONS
        .iter()
        .position(|action| just_pressed(world, action))
    {
        select(world, player, slot);
    }
    let step = match (just_pressed(world, NEXT), just_pressed(world, PREV)) {
        (true, false) => 1,
        (false, true) => SLOTS.len() - 1,
        _ => return,
    };
    let Some(current) = world.get::<Spellbook>(player).map(|book| book.slot) else {
        return;
    };
    let next = (1..=SLOTS.len())
        .map(|i| (current + step * i) % SLOTS.len())
        .find(|&slot| SLOTS[slot].is_some());
    if let Some(slot) = next {
        select(world, player, slot);
    }
}

/// Whether the held spell was refused for mana since the cast was pressed.
fn refused(world: &World) -> bool {
    world
        .get_resource::<Refused>()
        .is_some_and(|refused| refused.mana)
}

/// Notes that the held spell was refused for mana.
fn refuse(world: &mut World) {
    if let Some(flag) = world.get_resource_mut::<Refused>() {
        flag.mana = true;
    }
}

/// Forgets the held spell's refusals: the cast is released.
fn released(world: &mut World) {
    if let Some(flag) = world.get_resource_mut::<Refused>() {
        *flag = Refused::default();
    }
}

/// One step of the held `spell`: its ability's use, then its spray along
/// the aim or its small balls at the cursor. The use's outcome.
fn hold(world: &mut World, player: Entity, spell: Spell) -> Used {
    let used = use_ability(world, player, spell.ability());
    if used != Used::Used {
        return used;
    }
    let aim = aim(world, player);
    if let Some(element) = spell.spray() {
        toys::spray(world, player, aim, element);
    } else if let Some(at) = cursor_to_world(world) {
        let dt = world.resource::<Time>().delta();
        toys::place_small_balls(world, at, dt);
    }
    player::cast(world, player, aim, false);
    Used::Used
}

/// A press of `spell`: refused before its ability spends anything while
/// its toy is at its cap, it needs the cursor and has none, a summon's box
/// would overlap a solid tile there, or an iron block, a crate or a keg
/// would not fit there (`toys::iron_fits`, `props::fits`); then its use,
/// then its toy. A refusal says why, once ([`refuse_cast`]).
fn press(world: &mut World, player: Entity, spell: Spell) {
    let Some(centre) = player::centre(world, player) else {
        return;
    };
    let at = cursor_to_world(world);
    let refused = match spell {
        Spell::FireMissile => toys::missile_count(world) >= MISSILE_CAP,
        Spell::BowlingBall => toys::ball_count(world) >= BALL_CAP,
        Spell::BlackHole | Spell::Lightning => at.is_none(),
        Spell::IronBlock => at.is_none_or(|at| !toys::iron_fits(world, at)),
        Spell::Summon => at.is_none_or(|at| {
            let summoned = summon_type(world).summoned;
            let fits = match (summoned, summoned.prop()) {
                (_, Some(kind)) => props::fits(world, kind, at),
                (Summoned::Enemy(kind), None) => enemies::fits(world, kind, at),
                _ => dummy::fits(world, at),
            };
            !fits
        }),
        _ => false,
    };
    let used = if refused {
        None
    } else {
        Some(use_ability(world, player, spell.ability()))
    };
    let refusal = match used {
        None => Some(Refusal::NoRoom),
        Some(used) if used != Used::Used => Refusal::of(used),
        Some(_) => None,
    };
    if refused || used != Some(Used::Used) {
        if let Some(refusal) = refusal {
            refuse_cast(world, refusal, at.unwrap_or(centre));
        }
        return;
    }
    let aim = aim(world, player);
    let at = at.unwrap_or(centre);
    match spell {
        // The ability's spawn payload fires it; `adopt` takes it in.
        Spell::FireMissile => audio::play(world, Sound::Cast, centre),
        Spell::ForcePush => {
            toys::force_push(world, centre);
            audio::play(world, Sound::Pushed, centre);
        }
        Spell::BlackHole => {
            toys::place_black_hole(world, at);
        }
        Spell::IronBlock => {
            toys::place_iron(world, at);
        }
        Spell::BowlingBall => {
            toys::toss_bowling_ball(world, centre, aim);
        }
        Spell::Lightning => {
            lightning::strike(world, player, at);
        }
        Spell::Summon => {
            let summoned = summon_type(world);
            match (summoned.summoned, summoned.summoned.prop()) {
                (_, Some(kind)) => {
                    props::place(world, kind, at);
                }
                (Summoned::Enemy(kind), None) => {
                    enemies::summon(world, kind, at);
                }
                _ => {
                    dummy::summon(world, summoned.prefab, at);
                }
            }
            audio::play(world, Sound::Summoned, at);
        }
        Spell::Flamethrower | Spell::IceSpray | Spell::SmallBalls => {}
    }
    player::cast(world, player, aim, spell == Spell::BowlingBall);
}

/// `spells_held`, in `Update`: a spray shows from the frame its cast
/// begins and stops in the frame it ends, between two steps too. `cast`
/// held on a spray the player can pay a step of starts it when none runs
/// and none was refused for mana since the press, and one it cannot pay
/// says so, once a press; a release, another spell, a death or the
/// player's loss ends it at once. Its cost and its exposure wait for the
/// step. The small balls' loop stops in the frame the cast lets go of them
/// or gameplay leaves the top, under pause or game over, which no fixed
/// step sees.
pub fn held(world: &mut World) {
    if !pressed(world, CAST) {
        released(world);
    }
    let pouring = caster(world).is_some_and(|player| {
        pressed(world, CAST) && selected(world, player) == Some(Spell::SmallBalls)
    });
    if !pouring || shell::top(world) != Some(GAMEPLAY) {
        toys::stop_small_balls_loop(world);
    }
    let wanted = caster(world).and_then(|player| {
        let spell = selected(world, player)?;
        let element = spell.spray()?;
        pressed(world, CAST).then_some((player, spell, element))
    });
    let ended: Vec<Entity> = world
        .query::<(Entity, &Spray)>()
        .filter(|(_, spray)| {
            wanted.is_none_or(|(player, _, element)| {
                spray.caster != player || spray.element != element
            })
        })
        .map(|(entity, _)| entity)
        .collect();
    for spray in ended {
        toys::systems::end_spray(world, spray);
    }
    let Some((player, spell, element)) = wanted else {
        return;
    };
    let payable = cost(world, spell).is_some_and(|cost| mana(world, player) >= cost);
    if !payable && toys::spraying(world, player).is_none() {
        say(world, player, Refusal::NoMana);
    }
    if payable && !refused(world) && toys::spraying(world, player).is_none() {
        let aim = aim(world, player);
        toys::start_spray(world, player, aim, element);
    }
}

/// `missile_adopt`, in `PreUpdate` before the shell's save: each missile
/// the fire missile's spawn payload landed at the last flush joins the
/// toys from where the payload set it, the caster's point plus the offset
/// along the aim, its centre moved to the caster's centre's place.
pub fn adopt(world: &mut World) {
    let fresh: Vec<Entity> = world
        .query_filtered::<Entity, (With<FireMissile>, Without<SaveId>)>()
        .collect();
    for missile in fresh {
        toys::adopt_missile(world, missile, BODY_OFFSET);
    }
}
