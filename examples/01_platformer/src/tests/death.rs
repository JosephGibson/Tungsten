use super::*;
use crate::brick::IronBrick;
use crate::burning::BallBurn;
use crate::death::{COVER_SECS, DIM_SECS, DeathScreen, UNCOVER_SECS};
use crate::fireball::{FireballMissile, TrailLight};
use crate::gameplay::{Health, PlayerLantern, SceneTime};
use crate::state::{EffectSequence, EffectSounds, LightingFixture, SmallBall};
use tungsten::core::post::{PostPass, PostStack};
use tungsten::core::{
    Light, Particle, ParticleActive, ParticleEmitter, SoundData, SoundRegistry, WorldRngSeed,
};
use tungsten::physics::PhysicsBuffers;

/// A launch without a window: `configure_app`, then the startup hook's
/// `populate_world` over test registries standing in for the manifests.
fn launched() -> Harness {
    let mut app = App::new(Config::default()).expect("App::new failed");
    crate::setup::configure_app(&mut app);
    let mut harness = Harness::new(app);
    let world = harness.world_mut();
    load_presentation_assets(world);
    let sounds = world.get_resource_mut::<SoundRegistry>().unwrap();
    for id in [
        "sfx_blip",
        "ex10_black_hole_sfx",
        "music_main",
        "ex10_fireball_cast_sfx",
        "ex10_fireball_blast_sfx",
        "ex10_extinguish_sfx",
        "ex10_player_hit_sfx",
        "ex10_iron_crush_sfx",
        "ex10_ice_beam_sfx",
        "ex10_ice_spray_sfx",
        "ex10_ice_freeze_sfx",
        "ex10_ice_end_sfx",
        "ex10_ice_shatter_sfx",
    ] {
        let silence = SoundData {
            samples: vec![0.0],
            sample_rate: 44_100,
            channels: 1,
        };
        sounds.register(id.into(), silence, 0.5, false);
    }
    crate::setup::populate_world(world);
    harness
}

/// What a launch sets, entity by entity and in the per-run resources. Entity
/// ids are left out: a restart reuses freed slots.
fn snapshot(world: &World) -> Vec<String> {
    let mut lines: Vec<String> = world
        .query::<Entity>()
        .map(|e| {
            let mut line = Vec::new();
            let flags: [(&str, bool); 13] = [
                ("ball", world.has::<Ball>(e)),
                ("brick", world.has::<IronBrick>(e)),
                ("burn", world.has::<BallBurn>(e)),
                ("missile", world.has::<FireballMissile>(e)),
                ("trail light", world.has::<TrailLight>(e)),
                ("hole", world.has::<BlackHole>(e)),
                ("emitter", world.has::<ParticleEmitter>(e)),
                ("particle", world.has::<Particle>(e)),
                ("transient", world.has::<crate::state::TransientEmitter>(e)),
                ("explosion", world.has::<crate::gameplay::Explosion>(e)),
                ("hazard", world.has::<crate::gameplay::Hazard>(e)),
                ("platform", world.has::<crate::gameplay::MovingPlatform>(e)),
                ("tilemap", world.has::<TilemapInstance>(e)),
            ];
            line.extend(flags.iter().filter(|f| f.1).map(|f| f.0.to_string()));
            line.push(format!("{:?}", world.get::<Player>(e)));
            line.push(format!("{:?}", world.get::<Position>(e)));
            line.push(format!("{:?}", world.get::<Velocity>(e)));
            line.push(format!("{:?}", world.get::<Transform>(e)));
            line.push(format!("{:?}", world.get::<Collider>(e)));
            line.push(format!("{:?}", world.get::<RigidBody>(e)));
            line.push(format!("{:?}", world.get::<CurrentSprite>(e)));
            line.push(format!("{:?}", world.get::<SmallBall>(e)));
            line.push(format!("{:?}", world.get::<BallHue>(e)));
            line.push(format!("{:?}", world.get::<Light>(e)));
            line.push(format!(
                "{:?}",
                world.get::<crate::state::PlayerPresentation>(e)
            ));
            if let Some(h) = world.get::<Health>(e) {
                line.push(format!(
                    "hearts {} {} {}",
                    h.hearts, h.immunity, h.control_lock
                ));
            }
            if let Some(lantern) = world.get::<PlayerLantern>(e) {
                line.push(format!("lantern {}", lantern.enabled));
            }
            line.join(" ")
        })
        .collect();
    lines.sort();
    let controller = world.get_resource::<CameraController>().unwrap();
    let audio = world.get_resource::<AudioState>().unwrap();
    let text = world.get_resource::<TextDisplayState>().unwrap();
    lines.extend([
        format!("{:?}", world.get_resource::<DeathScreen>()),
        format!("{:?}", world.get_resource::<BallSpawnState>()),
        format!("{:?}", world.get_resource::<ActiveBlackHole>()),
        format!("{:?}", world.get_resource::<EffectSounds>()),
        format!("{:?}", world.get_resource::<LightingFixture>()),
        format!("{:?}", world.get_resource::<PostStack>()),
        format!("{:?}", world.get_resource::<CameraState>()),
        format!("{:?}", world.get_resource::<ParticleActive>()),
        format!("{:?}", world.get_resource::<WorldRngSeed>()),
        format!("buffers {}", world.has_resource::<PhysicsBuffers>()),
        format!(
            "effects {}",
            world.get_resource::<EffectSequence>().unwrap().0
        ),
        format!("scene {}", world.get_resource::<SceneTime>().unwrap().0),
        format!(
            "camera {:?} {} {} {}",
            matches!(controller.mode, CameraMode::Follow(_)),
            controller.zoom_multiplier,
            controller.shake_trauma,
            controller.shake_phase
        ),
        format!("audio {} {}", audio.music_playing, audio.master_volume),
        format!(
            "text {} {} {} {} {}",
            text.timer, text.contacts, text.music_on, text.vol_pct, text.zoom_pct
        ),
        format!(
            "commands {}",
            world.get_resource::<CommandBuffer>().unwrap().len()
        ),
        format!(
            "shakes {}",
            world
                .get_resource::<EventQueue<ShakeEvent>>()
                .unwrap()
                .len()
        ),
    ]);
    lines
}

fn kill_player(world: &mut World) {
    let player = world.query::<(Entity, &Player)>().next().unwrap().0;
    for _ in 0..3 {
        world.get_mut::<Health>(player).unwrap().immunity = 0.0;
        crate::gameplay::damage_player(world, player, PLAYER_SPAWN, 1);
    }
}

fn frames(secs: f32) -> u32 {
    (secs * 60.0).ceil() as u32 + 1
}

#[test]
fn restart_rebuilds_the_world_as_a_fresh_launch() {
    let fresh = snapshot(launched().world());
    let mut played = launched();
    set_dt(&mut played, 1.0 / 60.0);
    // Play through the real input: marbles, a brick, a fireball, a held
    // black hole, zoom, music, the lantern; then lose every heart.
    played.set_cursor(700.0, 300.0);
    played
        .world_mut()
        .get_resource_mut::<ActionMap>()
        .unwrap()
        .replace_bindings(
            "spawn_black_hole",
            vec![Binding::Mouse {
                button: MouseButton::Right,
            }],
        );
    for action in [
        "spawn_small_ball",
        "place_brick",
        "cast_fireball",
        "spawn_black_hole",
        "zoom_in",
        "audio_toggle_music",
    ] {
        played.press_action(action);
    }
    input_mut(&mut played).key_down(KeyCode::KeyL);
    played.step(40);
    kill_player(played.world_mut());
    played.step(2);
    let world = played.world();
    assert!(world.query::<(Entity, &SmallBall)>().count() > 0);
    assert_eq!(world.query::<(Entity, &IronBrick)>().count(), 1);
    assert!(world.get_resource::<ActiveBlackHole>().unwrap().0.is_some());
    assert_ne!(snapshot(world), fresh);

    crate::setup::restart_world(played.world_mut());
    assert_eq!(snapshot(played.world()), fresh);
}

#[test]
fn the_death_screen_dims_takes_the_restart_and_uncovers_a_new_run() {
    let mut harness = launched();
    set_dt(&mut harness, 1.0 / 60.0);
    kill_player(harness.world_mut());
    let screen = |harness: &Harness| *harness.world().get_resource::<DeathScreen>().unwrap();
    let fades = |harness: &Harness| {
        let stack = harness.world().get_resource::<PostStack>().unwrap();
        stack
            .0
            .iter()
            .filter(|p| matches!(p, PostPass::Fade(_)))
            .count()
    };
    // A press before the frame is dim does nothing.
    harness.press_action("restart");
    harness.step(1);
    harness.release_action("restart");
    assert!(matches!(screen(&harness), DeathScreen::Dying { .. }));
    assert_eq!(fades(&harness), 1);
    harness.step(frames(DIM_SECS));
    assert!(screen(&harness).accepts_restart());
    harness.press_action("restart");
    harness.step(1);
    harness.release_action("restart");
    harness.step(frames(COVER_SECS));
    assert!(matches!(screen(&harness), DeathScreen::Uncovering { .. }));
    let world = harness.world();
    let player = world.query::<(Entity, &Player)>().next().unwrap().0;
    assert_eq!(world.get::<Health>(player).unwrap().hearts, 3);
    assert!(world.get::<Collider>(player).is_some());
    assert!(!world.get::<Player>(player).unwrap().air_jump_used);
    harness.step(frames(UNCOVER_SECS));
    assert_eq!(screen(&harness), DeathScreen::Alive);
    assert_eq!(fades(&harness), 0);
    assert!(
        !harness
            .world()
            .get_resource::<PostStack>()
            .unwrap()
            .0
            .iter()
            .any(|p| matches!(p, PostPass::Pixelate(_)))
    );
}

#[test]
fn a_fall_is_fatal_during_immunity_and_the_screen_advances_while_game_time_is_paused() {
    let mut harness = launched();
    set_dt(&mut harness, 1.0 / 60.0);
    let world = harness.world_mut();
    let player = world.query::<(Entity, &Player)>().next().unwrap().0;
    world.get_mut::<Health>(player).unwrap().immunity = 10.0;
    world.get_mut::<Position>(player).unwrap().0.y = crate::state::KILL_Y + 1.0;
    crate::systems::despawn_out_of_bounds(world);
    assert_eq!(world.get::<Health>(player).unwrap().hearts, 0);
    assert!(world.get::<RigidBody>(player).is_none());
    world.get_resource_mut::<Time>().unwrap().set_paused(true);
    harness.step(frames(DIM_SECS));
    let world = harness.world();
    assert!(
        world
            .get_resource::<DeathScreen>()
            .unwrap()
            .accepts_restart()
    );
    assert_eq!(world.get::<Health>(player).unwrap().hearts, 0);
    assert!(
        world
            .get_resource::<PostStack>()
            .unwrap()
            .0
            .iter()
            .any(|p| matches!(p, PostPass::Pixelate(block) if *block == 48.0))
    );
    let sections = crate::extract::extract_text(world);
    assert!(sections.iter().any(|s| s.content == "YOU DIED"));
    assert!(
        sections
            .iter()
            .any(|s| s.content == "Press Enter to restart")
    );
    assert!(
        !sections
            .iter()
            .any(|s| s.content.contains("FPS") || s.content.contains("A/D"))
    );
}
