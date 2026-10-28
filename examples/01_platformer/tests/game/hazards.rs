//! The hazards: lava's surface, which holds the player, its touch, which
//! burns and throws on every touch, hurting once a window, and a dying touch
//! starting the death screen; an enemy that falls in, bitten and thrown the
//! same way; its crust's freeze and thaw, one front down through the basin
//! each way with a sound and bursts at each change; what falls into lava,
//! taken or set off; a hit's flash, shake and sound; the fire hazards the
//! level's data may place, which burst fast balls they sweep; and the fire
//! geysers' cycle, their eruption's burn and throw, and the ice spray's
//! choke.

use example_01_platformer::features::audio::components::{Sound, Sounded};
use example_01_platformer::features::combat::BURNING;
use example_01_platformer::features::enemies::{
    self,
    components::{Enemy, EnemyKind},
};
use example_01_platformer::features::level::components::{Doused, Hazard};
use example_01_platformer::features::props::{self, components::PropKind};
use example_01_platformer::features::toys;
use example_01_platformer::game::GAMEPLAY;
use example_01_platformer::shell::game_over::GAME_OVER;
use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{
    AssetRegistry, AudioCommand, Bundle, CameraController, Collider, Entity,
    ParticleConfigRegistry, ParticleEmitter, Position, PrevPosition, RigidBodyBundle,
    SoundRegistry, Sprite, Transform, Visibility, World,
};
use tungsten::kit::{ContactDamage, DamageRequest, Damaged, Health, HitFlash, Tags, damage, heal};
use tungsten::testing::UiHarness;

use example_01_platformer::features::elements::FIRE_CENTRE;
use example_01_platformer::features::elements::systems::fire_contacts;
use example_01_platformer::features::toys::RING_CAP;
use example_01_platformer::features::toys::components::{Ball, Ring, ShockRing};
use example_01_platformer::shared::{BURST_CAP, Burst, despawn_tree};

use example_01_platformer::features::level::geyser::{
    self, Geyser, GeyserPhase, VENT_CENTRE, VENT_HEIGHT,
};
use example_01_platformer::features::level::lava::Lava;
use example_01_platformer::features::spells::components::Spell;
use tungsten::core::RigidBody;
use tungsten::kit::ImmunePart;

use super::player_half;
use super::{
    aim_at, ball, body, bursts, calm, centre, choose, fall_dead, ground, pin_camera, run, stand,
    teleport, top,
};

const DT: f32 = 1.0 / 60.0;

fn health(harness: &UiHarness, player: Entity) -> f32 {
    harness.world().get::<Health>(player).unwrap().current()
}

/// The level's lava basins, in spawn order.
fn lava(harness: &UiHarness) -> Vec<Entity> {
    harness
        .world()
        .query::<(Entity, &Hazard)>()
        .filter(|(_, hazard)| hazard.lava)
        .map(|(entity, _)| entity)
        .collect()
}

/// Plays of the manifest sound `id` in the last frame.
fn plays(harness: &UiHarness, id: &str) -> usize {
    let handle = harness.world().resource::<SoundRegistry>().get_by_id(id);
    harness
        .harness()
        .audio()
        .iter()
        .filter(
            |command| matches!(command, AudioCommand::Play { handle: h, .. } if Some(*h) == handle),
        )
        .count()
}

#[test]
fn audio_controls_and_damage_flash_shake_remain_wired() {
    let (mut harness, player) = run();
    harness.step(10);
    // A body that is no hazard against the player: no hit.
    let at = centre(&harness, player) + Vec2::new(26.0, 0.0);
    harness.world_mut().spawn_with(
        RigidBodyBundle::dynamic(Position(at), Collider::circle(8.0))
            .with((SceneEntity { state_id: GAMEPLAY },)),
    );
    harness.step(5);
    let world = harness.world();
    assert_eq!(health(&harness, player), 100.0);
    assert!(!world.get::<HitFlash>(player).unwrap().is_active());
    assert_eq!(world.resource::<CameraController>().shake_trauma, 0.0);
    // A hit flashes, shakes and plays its sound.
    damage(harness.world_mut(), player, 34.0);
    harness.step(1);
    let world = harness.world();
    assert!(world.get::<HitFlash>(player).unwrap().is_active());
    assert!(world.resource::<CameraController>().shake_trauma > 0.0);
    assert_eq!(plays(&harness, "ex10_player_hit_sfx"), 1);
}

#[test]
fn a_burn_tick_plays_no_hurt_and_a_killing_one_plays_the_death() {
    let (mut harness, player) = run();
    harness.step(10);
    let tick = |harness: &mut UiHarness, amount: f32| {
        DamageRequest::new(player, amount)
            .with_periodic(true)
            .queue(harness.world_mut());
        harness.step(1);
    };
    tick(&mut harness, 5.0);
    assert_eq!(health(&harness, player), 95.0);
    assert_eq!(
        plays(&harness, "ex10_player_hit_sfx"),
        0,
        "a tick plays no hurt"
    );
    let world = harness.world();
    assert_eq!(
        world.resource::<CameraController>().shake_trauma,
        0.0,
        "nor shakes"
    );
    tick(&mut harness, 200.0);
    assert!(harness.world().get::<Health>(player).unwrap().is_dead());
    assert_eq!(
        (
            plays(&harness, "ex10_player_hit_sfx"),
            plays(&harness, "ex10_player_death_sfx")
        ),
        (0, 1),
        "the killing tick plays the death in place of the hurt"
    );
    assert!(harness.world().resource::<CameraController>().shake_trauma > 0.0);
}

/// The skin of `basin`, the lava's top under which nothing that stands on
/// it sinks: its corner's row plus the skin's 6 px.
fn skin(harness: &UiHarness, basin: Entity) -> f32 {
    harness.world().get::<Transform>(basin).unwrap().position.y + 6.0
}

/// The non-periodic hits the player took in the last frame.
fn blows(harness: &UiHarness, player: Entity) -> Vec<f32> {
    harness
        .harness()
        .events::<Damaged>()
        .filter(|hit| hit.entity == player && !hit.periodic)
        .map(|hit| hit.amount)
        .collect()
}

#[test]
fn hazards_damage_once_per_cooldown_and_the_last_heart_starts_the_death_screen() {
    let (mut harness, player) = run();
    calm(&mut harness);
    harness.step(10);
    let lake = lava(&harness)[0];
    let x = harness.world().get::<Transform>(lake).unwrap().position.x + 96.0;
    let surface = skin(&harness, lake);
    stand(&mut harness, player, x, surface);
    harness.step(1);
    assert_eq!(blows(&harness, player), [16.0], "lava's bite");
    for _ in 0..10 {
        stand(&mut harness, player, x, surface);
        harness.step(1);
        assert!(blows(&harness, player).is_empty(), "once per window");
    }
    // Away until the burn and the window run out, brought low, then a touch
    // that kills.
    let away = Vec2::new(x - 4.0 * 64.0, surface - 2.0 * 64.0);
    teleport(&mut harness, player, away);
    harness.step(300);
    heal(harness.world_mut(), player, 500.0);
    damage(harness.world_mut(), player, 90.0);
    harness.step(70);
    stand(&mut harness, player, x, surface);
    harness.step(2);
    assert!(harness.world().get::<Health>(player).unwrap().is_dead());
    fall_dead(&mut harness);
    assert_eq!(top(&harness), Some(GAME_OVER));
}

#[test]
fn lava_throws_the_player_up_and_out_and_never_lets_them_sink() {
    let (mut harness, player) = run();
    calm(&mut harness);
    harness.step(10);
    let mut basins = lava(&harness);
    let corner = |harness: &UiHarness, basin: Entity| {
        harness.world().get::<Transform>(basin).unwrap().position.x
    };
    basins.sort_by(|a, b| corner(&harness, *a).total_cmp(&corner(&harness, *b)));
    // The lake's two pools, 4 columns each, and the shelf's pool, 3.
    assert_eq!(basins.len(), 3);
    for (basin, columns) in basins.into_iter().zip([4.0, 4.0, 3.0]) {
        let left = harness.world().get::<Transform>(basin).unwrap().position.x;
        let right = left + columns * 64.0;
        let top = skin(&harness, basin);
        for (x, bank) in [(left + 30.0, -1.0), (right - 30.0, 1.0)] {
            // On the near bank, out of the lava, until the window runs out.
            let shore = Vec2::new(left - 96.0, top - 3.0 * player_half().y);
            teleport(&mut harness, player, shore);
            heal(harness.world_mut(), player, 500.0);
            harness.step(70);
            stand(&mut harness, player, x, top);
            harness.step(1);
            assert_eq!(
                blows(&harness, player),
                [16.0],
                "{basin:?} at {x}: the bite"
            );
            let tags = harness.world().get::<Tags>(player).unwrap();
            assert!(tags.has(BURNING), "{basin:?} at {x}: the bite burns");
            let start = centre(&harness, player);
            harness.step(8);
            let at = centre(&harness, player);
            assert!(
                at.y < start.y - 30.0,
                "{basin:?} at {x}: thrown up, {start} to {at}"
            );
            assert!(
                (at.x - start.x) * bank > 0.0,
                "{basin:?} at {x}: toward its nearer bank"
            );
            // Inside the window: thrown again, unhurt.
            stand(&mut harness, player, x, top);
            harness.step(1);
            assert!(
                blows(&harness, player).is_empty(),
                "{basin:?} at {x}: the window"
            );
            harness.step(6);
            let again = centre(&harness, player).y + player_half().y;
            assert!(again < top - 30.0, "{basin:?} at {x}: thrown again");
            // Never below the skin, whatever it lands on, for two seconds.
            for frame in 0..120 {
                harness.step(1);
                let at = centre(&harness, player);
                let feet = at.y + player_half().y;
                let over = (left..right).contains(&at.x);
                assert!(
                    !over || feet <= top + 1.0,
                    "{basin:?} frame {frame}: sank to {feet} under {top}"
                );
            }
        }
    }
}

#[test]
fn lava_bites_an_enemy_that_falls_in_and_throws_it_up_and_out() {
    let (mut harness, _) = run();
    calm(&mut harness);
    harness.step(10);
    let mut basins = lava(&harness);
    let corner = |harness: &UiHarness, basin: Entity| {
        harness.world().get::<Transform>(basin).unwrap().position.x
    };
    basins.sort_by(|a, b| corner(&harness, *a).total_cmp(&corner(&harness, *b)));
    // The lake's two pools, 4 columns each, and the shelf's pool, 3: a
    // goblin dropped over each end of each.
    let ends = basins
        .into_iter()
        .zip([4.0, 4.0, 3.0])
        .flat_map(|(basin, columns)| {
            let left = corner(&harness, basin);
            let top = skin(&harness, basin);
            [
                (left + 40.0, top, -1.0),
                (left + columns * 64.0 - 40.0, top, 1.0),
            ]
        });
    for (x, top, bank) in ends.collect::<Vec<_>>() {
        let at = Vec2::new(x, top - 96.0);
        let goblin = enemies::summon(harness.world_mut(), EnemyKind::Goblin, at).unwrap();
        let mut bite = Vec::new();
        for _ in 0..60 {
            harness.step(1);
            bite = blows(&harness, goblin);
            if !bite.is_empty() {
                break;
            }
        }
        assert_eq!(bite, [16.0], "at {x}: the player's bite");
        let tags = harness.world().get::<Tags>(goblin).unwrap();
        assert!(tags.has(BURNING), "at {x}: the bite burns");
        let start = enemies::centre(harness.world(), goblin).unwrap();
        harness.step(8);
        let thrown = enemies::centre(harness.world(), goblin).unwrap();
        assert!(
            thrown.y < start.y - 30.0,
            "at {x}: thrown up, {start} to {thrown}"
        );
        assert!(
            (thrown.x - start.x) * bank > 0.0,
            "at {x}: toward its nearer bank"
        );
        despawn_tree(harness.world_mut(), goblin);
        harness.step(1);
    }
}

#[test]
fn lava_bites_with_fire_and_the_fires_douse_leaves_it_hot() {
    let (mut harness, player) = run();
    harness.step(10);
    let basins = lava(&harness);
    assert_eq!(basins.len(), 3, "the lake's two pools and the shelf's");
    let basin = basins[0];
    let corner = harness.world().get::<Transform>(basin).unwrap().position;
    // On the lake's skin, over its first lava column.
    let top = skin(&harness, basin);
    stand(&mut harness, player, corner.x + 32.0, top);
    harness.step(1);
    assert_eq!(health(&harness, player), 84.0, "16 fire");
    harness.step(1);
    assert!(
        centre(&harness, player).y < top - player_half().y,
        "the touch knocks the player up and away"
    );
    // The douse puts a fire out; lava stays hot (the spray crusts it).
    example_01_platformer::features::elements::douse_hazard(harness.world_mut(), basin);
    let world = harness.world();
    assert!(!world.has::<Doused>(basin) && world.has::<ContactDamage>(basin));
}

/// Whether `basin` has a crust set.
fn crusted(harness: &UiHarness, basin: Entity) -> bool {
    harness
        .world()
        .get::<Lava>(basin)
        .is_some_and(|lava| lava.crust.is_some())
}

#[test]
fn the_ice_spray_crusts_a_basin_that_holds_the_player_and_melts_to_throw_them() {
    let (mut harness, player) = run();
    calm(&mut harness);
    harness.step(10);
    let mut basins = lava(&harness);
    let corner = |harness: &UiHarness, basin: Entity| {
        harness.world().get::<Transform>(basin).unwrap().position
    };
    basins.sort_by(|a, b| corner(&harness, *a).x.total_cmp(&corner(&harness, *b).x));
    let pool = basins[0];
    let at = corner(&harness, pool);
    let surface = skin(&harness, pool);
    // On the bank beside the pool, the ice spray aimed at its surface.
    stand(&mut harness, player, at.x - 40.0, at.y - 64.0);
    harness.step(2);
    pin_camera(&mut harness, at - Vec2::new(500.0, 400.0));
    choose(&mut harness, Spell::IceSpray);
    // The cursor held on the pool's surface each frame, as the camera
    // settles.
    let target = Vec2::new(at.x + 96.0, surface + 2.0);
    let spray = |harness: &mut UiHarness, steps: u32| {
        harness.harness_mut().press_action("cast");
        for _ in 0..steps {
            aim_at(harness, target);
            harness.step(1);
        }
        harness.harness_mut().release_action("cast");
        harness.step(1);
    };
    spray(&mut harness, 18);
    assert!(!crusted(&harness, pool), "0.3 s of spray sets no crust");
    harness.step(90);
    spray(&mut harness, 33);
    assert!(crusted(&harness, pool), "0.5 s sets it");
    let world = harness.world();
    assert!(!world.has::<ContactDamage>(pool), "its touch held aside");
    assert_eq!(
        world.get::<RigidBody>(pool).unwrap().layers,
        1,
        "solid as rock"
    );
    // The player stands on it untouched, and a ball rests on it.
    stand(&mut harness, player, at.x + 96.0, surface);
    let rock = ball(&mut harness, Vec2::new(at.x + 200.0, surface - 40.0), false);
    for _ in 0..60 {
        harness.step(1);
        assert!(
            blows(&harness, player).is_empty(),
            "the crust bites nothing"
        );
    }
    let feet = centre(&harness, player).y + player_half().y;
    assert!(
        (feet - surface).abs() < 2.0,
        "standing on the crust: {feet} on {surface}"
    );
    assert!(body(&harness, rock).y < surface, "the ball rests on it");
    // It melts within its six seconds, throwing the player and dropping the ball.
    let mut thrown = false;
    for _ in 0..400 {
        harness.step(1);
        thrown |= blows(&harness, player) == [16.0];
        if !crusted(&harness, pool) {
            for _ in 0..4 {
                harness.step(1);
                thrown |= blows(&harness, player) == [16.0];
            }
            break;
        }
    }
    assert!(!crusted(&harness, pool), "melted");
    assert!(thrown, "the melt bites the player on it");
    harness.step(10);
    assert!(
        centre(&harness, player).y + player_half().y < surface - 20.0,
        "thrown up"
    );
    // 14 frames past the melt: the lava takes the ball within 60.
    harness.step(46);
    assert!(!harness.world().is_alive(rock), "the ball fell in and went");
}

/// A look row's state: hidden, a crust frame, a freeze partial or a melt
/// partial (01 (k)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    Hidden,
    Crust(usize),
    Freeze(usize),
    Melt(usize),
}

/// The look's rows down `basin`'s first column, top first.
fn first_column(harness: &UiHarness, basin: Entity) -> Vec<Row> {
    let world = harness.world();
    let left = world.get::<Transform>(basin).unwrap().position.x;
    let assets = world.resource::<AssetRegistry>();
    let mut rows: Vec<(f32, Row)> = world
        .query::<(&Sprite, &Transform, &Visibility)>()
        .filter(|(_, at, _)| (at.position.x - left).abs() < 1.0)
        .filter_map(|(sprite, at, shown)| {
            let name = assets.sprite_name(sprite.asset_id)?;
            let parts: Vec<&str> = name.strip_prefix("ex10_lava_")?.split('_').collect();
            let &[kind, _, n, _] = parts.as_slice() else {
                return None;
            };
            let n: usize = n.parse().ok()?;
            let row = match (shown.visible, kind) {
                (false, "crust" | "freeze" | "melt") => Row::Hidden,
                (true, "crust") => Row::Crust(n),
                (true, "freeze") => Row::Freeze(n),
                (true, "melt") => Row::Melt(n),
                _ => return None,
            };
            Some((at.position.y, row))
        })
        .collect();
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    rows.into_iter().map(|(_, row)| row).collect()
}

/// The bursts spawned since `seen`, by their configs' names, which `seen`
/// then holds.
fn new_bursts(harness: &UiHarness, seen: &mut std::collections::HashSet<Entity>) -> Vec<String> {
    let world = harness.world();
    let configs = world.resource::<ParticleConfigRegistry>();
    let mut out = Vec::new();
    for (entity, _, emitter) in world.query::<(Entity, &Burst, &ParticleEmitter)>() {
        if seen.insert(entity) {
            out.push(
                configs
                    .name_for_id(emitter.config)
                    .unwrap_or("?")
                    .to_owned(),
            );
        }
    }
    out
}

/// The lava sounds of the last frame, in order.
fn lava_sounds(harness: &UiHarness) -> Vec<Sound> {
    let lava = [
        Sound::LavaChilled,
        Sound::LavaSet,
        Sound::LavaCracked,
        Sound::LavaMelted,
    ];
    harness
        .harness()
        .events::<Sounded>()
        .map(|sounded| sounded.sound)
        .filter(|sound| lava.contains(sound))
        .collect()
}

#[test]
fn the_lava_freezes_and_thaws_through_its_frames_with_a_sound_at_each_change() {
    let (mut harness, player) = run();
    calm(&mut harness);
    harness.harness_mut().set_dt(DT);
    harness.step(10);
    let mut basins = lava(&harness);
    let corner = |harness: &UiHarness, basin: Entity| {
        harness.world().get::<Transform>(basin).unwrap().position
    };
    basins.sort_by(|a, b| corner(&harness, *a).x.total_cmp(&corner(&harness, *b).x));
    // The lake's west pool, three rows deep, sprayed until it sets, then
    // watched through its hold and its melt.
    let pool = basins[0];
    let at = corner(&harness, pool);
    let surface = skin(&harness, pool);
    stand(&mut harness, player, at.x - 40.0, at.y - 64.0);
    harness.step(2);
    pin_camera(&mut harness, at - Vec2::new(500.0, 400.0));
    choose(&mut harness, Spell::IceSpray);
    let target = Vec2::new(at.x + 96.0, surface + 2.0);
    let mut seen = std::collections::HashSet::new();
    let _ = new_bursts(&harness, &mut seen);
    let lava_configs = [
        "ex10_lava_mist",
        "ex10_lava_frost",
        "ex10_steam_burst",
        "ex10_lava_sparks",
        "ex10_lava_spatter",
    ];
    let mut columns = Vec::new();
    let mut changes: Vec<(Vec<Sound>, Vec<String>)> = Vec::new();
    let mut spraying = true;
    harness.harness_mut().press_action("cast");
    for _ in 0..600 {
        if spraying {
            aim_at(&mut harness, target);
        }
        harness.step(1);
        let bursts: Vec<String> = new_bursts(&harness, &mut seen)
            .into_iter()
            .filter(|name| lava_configs.contains(&name.as_str()))
            .collect();
        let sounds = lava_sounds(&harness);
        if !sounds.is_empty() || !bursts.is_empty() {
            changes.push((sounds, bursts));
        }
        columns.push(first_column(&harness, pool));
        if spraying && crusted(&harness, pool) {
            harness.harness_mut().release_action("cast");
            spraying = false;
        }
        if !spraying
            && !crusted(&harness, pool)
            && columns
                .last()
                .is_some_and(|rows| rows.iter().all(|row| *row == Row::Hidden))
        {
            break;
        }
    }
    assert_eq!(columns.last().unwrap().len(), 3, "three rows down the pool");
    // The freeze's front down row by row, each row whole before the next
    // shows a partial; the crust's frames 1 to 8 in order; the melt's front
    // down the same way; then nothing.
    let mut stage = 0;
    let (mut freeze_at, mut crust_at, mut melt_at) = ((0, 0), 0, (0, 0));
    let mut frames = std::collections::BTreeSet::new();
    for rows in &columns {
        let whole = rows.iter().take_while(|row| **row == Row::Crust(0)).count();
        let gone = rows.iter().take_while(|row| **row == Row::Hidden).count();
        if rows.iter().all(|row| *row == Row::Hidden) {
            assert!(
                stage == 0 || stage == 3 || stage == 4,
                "the look gone mid-way: {rows:?}"
            );
            if stage == 3 {
                stage = 4;
            }
        } else if rows.iter().any(|row| matches!(row, Row::Freeze(_))) || rows[0] == Row::Crust(0) {
            assert!(stage <= 1, "a freeze after the crust: {rows:?}");
            stage = 1;
            let front = match rows.get(whole) {
                Some(Row::Freeze(j)) => *j,
                _ => 8,
            };
            assert!(
                rows[whole + usize::from(front < 8)..]
                    .iter()
                    .all(|row| *row == Row::Hidden),
                "under the freeze's front: {rows:?}"
            );
            assert!(
                (whole, front) >= freeze_at,
                "the freeze's front rose: {rows:?}"
            );
            freeze_at = (whole, front);
        } else if let Row::Crust(n) = rows[0] {
            assert!(
                stage == 1 || stage == 2,
                "a crust frame out of turn: {rows:?}"
            );
            stage = 2;
            assert!(
                rows.iter().all(|row| *row == Row::Crust(n)),
                "one frame down the column: {rows:?}"
            );
            assert!(n >= crust_at, "the crust's frames back: {rows:?}");
            crust_at = n;
            frames.insert(n);
        } else {
            assert!(stage == 2 || stage == 3, "a melt out of turn: {rows:?}");
            stage = 3;
            let front = match rows.get(gone) {
                Some(Row::Melt(j)) => *j,
                _ => 8,
            };
            assert!(
                rows[gone + usize::from(front < 8)..]
                    .iter()
                    .all(|row| *row == Row::Crust(8)),
                "under the melt's front: {rows:?}"
            );
            assert!((gone, front) >= melt_at, "the melt's front rose: {rows:?}");
            melt_at = (gone, front);
        }
    }
    assert_eq!(stage, 4, "frozen, held, melted and gone");
    assert_eq!(frames, (1..=8).collect(), "the crust's frames");
    // A sound at each change, and its bursts, a burst to each two of the
    // pool's four columns, none dropped by the cap.
    let sounds: Vec<Sound> = changes
        .iter()
        .flat_map(|(sounds, _)| sounds.clone())
        .collect();
    let want = [
        Sound::LavaChilled,
        Sound::LavaSet,
        Sound::LavaCracked,
        Sound::LavaCracked,
        Sound::LavaCracked,
        Sound::LavaMelted,
    ];
    assert_eq!(sounds, want);
    let count =
        |bursts: &[String], name: &str| bursts.iter().filter(|burst| *burst == name).count();
    let at_sound = |sound: Sound| -> Vec<Vec<String>> {
        changes
            .iter()
            .filter(|(sounds, _)| sounds.contains(&sound))
            .map(|(_, bursts)| bursts.clone())
            .collect()
    };
    let chilled = &at_sound(Sound::LavaChilled)[0];
    assert_eq!(
        (
            count(chilled, "ex10_lava_mist"),
            count(chilled, "ex10_lava_frost")
        ),
        (2, 1),
        "{chilled:?}"
    );
    assert_eq!(count(&at_sound(Sound::LavaSet)[0], "ex10_steam_burst"), 3);
    for cracked in at_sound(Sound::LavaCracked) {
        assert_eq!(count(&cracked, "ex10_lava_sparks"), 2, "{cracked:?}");
    }
    assert_eq!(
        count(&at_sound(Sound::LavaMelted)[0], "ex10_lava_spatter"),
        2
    );
    let all: Vec<String> = changes
        .iter()
        .flat_map(|(_, bursts)| bursts.clone())
        .collect();
    assert_eq!(
        count(&all, "ex10_lava_mist"),
        6,
        "mist as the freeze starts and at each further third"
    );
    // An interrupted spray on the shelf's pool, 0.2 s on and 0.1 s off three
    // times: its front's mist at each new depth once, one chill.
    let shelf = basins[2];
    let at = corner(&harness, shelf);
    let surface = skin(&harness, shelf);
    stand(&mut harness, player, at.x - 40.0, at.y - 64.0);
    harness.step(30);
    pin_camera(&mut harness, at - Vec2::new(500.0, 400.0));
    let _ = new_bursts(&harness, &mut seen);
    let target = Vec2::new(at.x + 96.0, surface + 2.0);
    let mut all = Vec::new();
    let mut sounds = Vec::new();
    for _ in 0..3 {
        harness.harness_mut().press_action("cast");
        for _ in 0..12 {
            aim_at(&mut harness, target);
            harness.step(1);
            all.extend(new_bursts(&harness, &mut seen));
            sounds.extend(lava_sounds(&harness));
        }
        harness.harness_mut().release_action("cast");
        for _ in 0..6 {
            harness.step(1);
            all.extend(new_bursts(&harness, &mut seen));
            sounds.extend(lava_sounds(&harness));
        }
    }
    for _ in 0..60 {
        harness.step(1);
        all.extend(new_bursts(&harness, &mut seen));
        sounds.extend(lava_sounds(&harness));
    }
    assert!(!crusted(&harness, shelf), "no crust from the broken spray");
    assert_eq!(sounds, [Sound::LavaChilled], "one chill");
    assert_eq!(count(&all, "ex10_lava_frost"), 1);
    assert_eq!(
        count(&all, "ex10_lava_mist"),
        4,
        "mist at its start and at the one further third it reached"
    );
}

/// A body dropped into the world with its centre at a point.
type Drop = fn(&mut World, Vec2) -> Entity;

/// The sounds of `sound` in the last frame.
fn sounded(harness: &UiHarness, sound: Sound) -> usize {
    harness
        .harness()
        .events::<Sounded>()
        .filter(|sounded| sounded.sound == sound)
        .count()
}

#[test]
fn lava_takes_what_falls_in_and_sets_off_a_keg() {
    let (mut harness, _) = run();
    calm(&mut harness);
    harness.harness_mut().set_dt(DT);
    harness.step(10);
    let mut basins = lava(&harness);
    let corner = |harness: &UiHarness, basin: Entity| {
        harness.world().get::<Transform>(basin).unwrap().position
    };
    basins.sort_by(|a, b| corner(&harness, *a).x.total_cmp(&corner(&harness, *b).x));
    // The lake, the second basin from the west, and a point over its middle.
    let lake = basins[1];
    let over = Vec2::new(
        corner(&harness, lake).x + 128.0,
        skin(&harness, lake) - 96.0,
    );
    let drops: [(&str, Drop); 4] = [
        ("a small ball", |world, at| {
            toys::spawn_ball(world, at, true)
        }),
        ("a bowling ball", |world, at| {
            toys::spawn_ball(world, at, false)
        }),
        ("a crate", |world, at| {
            props::spawn(world, PropKind::Crate, at)
        }),
        ("an iron block", toys::place_iron),
    ];
    for (name, drop) in drops {
        let body = drop(harness.world_mut(), over);
        let (mut gone, mut heard) = (false, 0);
        for _ in 0..60 {
            harness.step(1);
            let sizzles = sounded(&harness, Sound::LavaTook);
            assert!(sizzles <= 1, "one sizzle a step");
            heard += sizzles;
            if !harness.world().is_alive(body) {
                gone = true;
                break;
            }
        }
        assert!(
            gone && heard == 1,
            "{name}: taken within a second, with a sizzle"
        );
    }
    // A keg is set off, its blast heard as it goes.
    let keg = props::spawn(harness.world_mut(), PropKind::Keg, over);
    let mut blasted = false;
    for _ in 0..60 {
        harness.step(1);
        assert!(sounded(&harness, Sound::LavaTook) <= 1, "one sizzle a step");
        blasted |= sounded(&harness, Sound::KegBlasted) > 0;
        if !harness.world().is_alive(keg) {
            break;
        }
    }
    assert!(blasted, "the keg went off");
    assert!(!harness.world().is_alive(keg), "and is gone");
}

#[test]
fn a_hit_plays_its_sound_and_immunity_ignores_repeat_hits_until_it_runs_out() {
    let (mut harness, player) = run();
    harness.step(10);
    damage(harness.world_mut(), player, 34.0);
    assert_eq!(health(&harness, player), 66.0);
    harness.step(1);
    assert_eq!(plays(&harness, "ex10_player_hit_sfx"), 1);
    damage(harness.world_mut(), player, 34.0);
    assert_eq!(health(&harness, player), 66.0);
    // The window counts down by itself; a hit just inside it still misses.
    harness.step((1.1 / DT) as u32 - 3);
    assert!(
        harness
            .world()
            .get::<Health>(player)
            .unwrap()
            .is_invulnerable()
    );
    damage(harness.world_mut(), player, 34.0);
    assert_eq!(health(&harness, player), 66.0);
    harness.step(3);
    assert!(
        !harness
            .world()
            .get::<Health>(player)
            .unwrap()
            .is_invulnerable()
    );
    damage(harness.world_mut(), player, 34.0);
    assert_eq!(health(&harness, player), 32.0);
}

/// A bare fire hazard whose touch box sits about `centre` now and about
/// `before` as the step began.
fn crossing_fire(harness: &mut UiHarness, centre: Vec2, before: Vec2) -> Entity {
    let (at, prev) = (centre - FIRE_CENTRE, before - FIRE_CENTRE);
    harness.world_mut().spawn_with((
        Hazard {
            fire: true,
            lava: false,
        },
        Position(at),
        PrevPosition(prev),
        Transform::from_position(at),
        SceneEntity { state_id: GAMEPLAY },
    ))
}

#[test]
fn moving_fire_sweeps_fast_balls_once_and_explosions_do_not_hurt_players() {
    let (mut harness, player) = run();
    let o = Vec2::new(900.0, 500.0);
    teleport(&mut harness, player, o + Vec2::new(0.0, 100.0));
    for _ in 0..2 {
        crossing_fire(
            &mut harness,
            o - Vec2::new(0.0, 100.0),
            o + Vec2::new(0.0, 100.0),
        );
    }
    // The ball and both fires cross between steps.
    let heavy = ball(&mut harness, o, false);
    let world = harness.world_mut();
    let offset = world.get::<Collider>(heavy).unwrap().offset;
    world.get_mut::<Position>(heavy).unwrap().0 = o + Vec2::new(100.0, 0.0) - offset;
    world.get_mut::<PrevPosition>(heavy).unwrap().0 = o - Vec2::new(100.0, 0.0) - offset;
    fire_contacts(harness.world_mut());
    assert!(!harness.world().is_alive(heavy));
    assert_eq!(
        bursts(&harness, "ex10_ball_explosion"),
        1,
        "one burst for one ball"
    );
    assert_eq!(ball_rings(&harness), 1);
    // The burst beside the player hurts nothing.
    assert_eq!(health(&harness, player), 100.0);
    harness.step(150);
    assert_eq!(bursts(&harness, "ex10_ball_explosion"), 0);
    assert_eq!(ball_rings(&harness), 0);
}

fn ball_rings(harness: &UiHarness) -> usize {
    harness
        .world()
        .query::<&ShockRing>()
        .filter(|ring| ring.kind == Ring::Ball)
        .count()
}

#[test]
fn explosion_bursts_and_rings_stay_bounded_under_dense_ball_contact() {
    let (mut harness, _) = run();
    let o = Vec2::new(900.0, 500.0);
    crossing_fire(&mut harness, o, o);
    for _ in 0..100 {
        ball(&mut harness, o, false);
    }
    fire_contacts(harness.world_mut());
    assert_eq!(harness.world().query::<&Ball>().count(), 0);
    assert_eq!(ball_rings(&harness), RING_CAP);
    assert_eq!(bursts(&harness, "ex10_ball_explosion"), BURST_CAP);
}

/// The tests' floor for a geyser, its top, and the mouth's column on it.
const GEYSER_TOP: f32 = 1000.0;
const GEYSER_X: f32 = 1200.0;

/// A geyser from its prefab, its mouth over `x` on a floor whose top is
/// `top`, `offset` seconds into its cycle: it begins on its first step.
fn geyser_at(harness: &mut UiHarness, x: f32, top: f32, offset: f32) -> Entity {
    let point = Vec2::new(x - VENT_CENTRE.x, top - VENT_HEIGHT);
    let world = harness.world_mut();
    let geyser = world.spawn_prefab("geyser", point).unwrap();
    world.get_mut::<Geyser>(geyser).unwrap().offset = offset;
    world.insert(geyser, SceneEntity { state_id: GAMEPLAY });
    geyser
}

/// Whether `geyser`'s touch is on.
fn erupting(harness: &UiHarness, geyser: Entity) -> bool {
    harness.world().has::<ContactDamage>(geyser)
}

fn state(harness: &UiHarness, geyser: Entity) -> Geyser {
    harness.world().get::<Geyser>(geyser).unwrap().clone()
}

#[test]
fn a_geysers_touch_is_off_through_its_idle_and_sputter_and_on_for_its_eruption() {
    let (mut harness, _) = run();
    calm(&mut harness);
    harness.harness_mut().set_dt(DT);
    ground(&mut harness, GEYSER_TOP);
    let geyser = geyser_at(&mut harness, GEYSER_X, GEYSER_TOP, 0.0);
    // 2.5 s idle and 0.6 s sputter, 186 steps, then 0.8 s erupting, 48.
    let mut on = Vec::new();
    for step in 1..=260 {
        harness.step(1);
        if erupting(&harness, geyser) {
            on.push(step);
        }
    }
    assert_eq!(on.len(), 48, "{on:?}");
    assert!((185..=187).contains(&on[0]), "{on:?}");
    assert!(
        on.windows(2).all(|w| w[1] == w[0] + 1),
        "one eruption: {on:?}"
    );
}

#[test]
fn a_geysers_eruption_burns_the_player_standing_on_it_and_throws_them_up() {
    let (mut harness, player) = run();
    calm(&mut harness);
    harness.harness_mut().set_dt(DT);
    ground(&mut harness, GEYSER_TOP);
    harness.step(1);
    stand(&mut harness, player, GEYSER_X, GEYSER_TOP);
    harness.step(20);
    // 2.6 s in, it erupts within its sputter's 0.6 s.
    let geyser = geyser_at(&mut harness, GEYSER_X, GEYSER_TOP, 2.6);
    let mut hit = Vec::new();
    let mut steps = 0;
    for _ in 0..60 {
        harness.step(1);
        steps += 1;
        hit = blows(&harness, player);
        if !hit.is_empty() {
            break;
        }
    }
    assert!(erupting(&harness, geyser), "erupting");
    assert!(steps <= 36, "within the sputter: {steps} steps");
    assert_eq!(hit, [12.0], "12 fire");
    let tags = harness.world().get::<Tags>(player).unwrap();
    assert!(tags.has(BURNING), "it burns");
    let mut fastest = 0.0_f32;
    let mut last = centre(&harness, player);
    for _ in 0..4 {
        harness.step(1);
        let at = centre(&harness, player);
        fastest = fastest.max((last.y - at.y) / DT);
        last = at;
    }
    assert!(fastest > 500.0, "thrown up at {fastest} px/s");
}

#[test]
fn a_wisp_over_an_erupting_geyser_takes_no_fire_and_does_not_burn() {
    let (mut harness, _) = run();
    calm(&mut harness);
    harness.harness_mut().set_dt(DT);
    ground(&mut harness, GEYSER_TOP);
    let at = Vec2::new(GEYSER_X, GEYSER_TOP - 80.0);
    let wisp = enemies::summon(harness.world_mut(), EnemyKind::Wisp, at).unwrap();
    let geyser = geyser_at(&mut harness, GEYSER_X, GEYSER_TOP, 2.6);
    let (mut met, mut erupted) = (0, false);
    for _ in 0..90 {
        harness.step(1);
        erupted |= erupting(&harness, geyser);
        met += harness
            .harness()
            .events::<ImmunePart>()
            .filter(|part| part.entity == wisp)
            .count();
        assert!(
            harness
                .harness()
                .events::<Damaged>()
                .all(|hit| hit.entity != wisp),
            "no damage"
        );
        let tags = harness.world().get::<Tags>(wisp).unwrap();
        assert!(!tags.has(BURNING), "no burn");
    }
    assert!(erupted && met > 0, "it stood in the plume: {met} parts");
}

#[test]
fn the_ice_spray_on_a_geysers_vent_chokes_it_for_six_seconds_then_it_idles() {
    let (mut harness, player) = run();
    calm(&mut harness);
    harness.harness_mut().set_dt(DT);
    ground(&mut harness, GEYSER_TOP);
    harness.step(1);
    let geyser = geyser_at(&mut harness, GEYSER_X, GEYSER_TOP, 0.0);
    stand(&mut harness, player, GEYSER_X - 160.0, GEYSER_TOP);
    harness.step(2);
    pin_camera(
        &mut harness,
        Vec2::new(GEYSER_X - 600.0, GEYSER_TOP - 400.0),
    );
    choose(&mut harness, Spell::IceSpray);
    let mouth = geyser::mouth(harness.world(), geyser).unwrap();
    harness.harness_mut().press_action("cast");
    let mut held = 0;
    for _ in 0..60 {
        aim_at(&mut harness, mouth);
        harness.step(1);
        held += 1;
        if state(&harness, geyser).choked > 0.0 {
            break;
        }
    }
    harness.harness_mut().release_action("cast");
    assert!(state(&harness, geyser).choked > 0.0, "choked");
    assert!(held <= 36, "within 0.6 s: {held} steps");
    // Six seconds without an eruption, then its idle's 2.5 s to its sputter.
    let mut choked = 0;
    for _ in 0..400 {
        harness.step(1);
        choked += 1;
        assert!(!erupting(&harness, geyser), "no eruption while choked");
        assert_eq!(state(&harness, geyser).phase, GeyserPhase::Idle);
        if state(&harness, geyser).choked <= 0.0 {
            break;
        }
    }
    assert!((358..=362).contains(&choked), "six seconds: {choked} steps");
    let mut idle = 0;
    for _ in 0..200 {
        harness.step(1);
        idle += 1;
        if state(&harness, geyser).phase == GeyserPhase::Sputter {
            break;
        }
        assert!(!erupting(&harness, geyser), "idle");
    }
    assert!(
        (149..=151).contains(&idle),
        "then idles 2.5 s: {idle} steps"
    );
}

#[test]
fn the_ice_spray_aimed_at_the_first_floors_map_geyser_chokes_it() {
    let (mut harness, player) = run();
    // The map's enemies off, its geysers kept.
    let world = harness.world_mut();
    let enemies: Vec<Entity> = world
        .query::<(Entity, &Enemy)>()
        .map(|(entity, _)| entity)
        .collect();
    for enemy in enemies {
        despawn_tree(world, enemy);
    }
    harness.harness_mut().set_dt(DT);
    // `hollow_floor_1`'s, at column 225 on row 63.
    let geyser = harness
        .world()
        .query::<(Entity, &Geyser, &Transform)>()
        .min_by(|a, b| a.2.position.x.total_cmp(&b.2.position.x))
        .map(|(entity, ..)| entity)
        .unwrap();
    let mouth = geyser::mouth(harness.world(), geyser).unwrap();
    assert_eq!(mouth.x, 225.0 * 64.0, "the first floor's geyser");
    stand(&mut harness, player, mouth.x - 160.0, 63.0 * 64.0);
    harness.step(2);
    pin_camera(&mut harness, mouth - Vec2::new(600.0, 400.0));
    choose(&mut harness, Spell::IceSpray);
    harness.harness_mut().press_action("cast");
    let mut held = 0;
    for _ in 0..60 {
        aim_at(&mut harness, mouth);
        harness.step(1);
        held += 1;
        if state(&harness, geyser).choked > 0.0 {
            break;
        }
    }
    harness.harness_mut().release_action("cast");
    assert!(state(&harness, geyser).choked > 0.0, "choked");
    assert!(held <= 36, "within 0.6 s: {held} steps");
}
