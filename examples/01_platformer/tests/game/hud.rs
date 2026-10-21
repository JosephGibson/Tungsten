//! The HUD: the health bar answers every hit by the next frame, at one
//! size and place on screen whatever the camera's zoom.

use example_01_platformer::features::level::components::Zoom;
use example_01_platformer::features::spells::hud::Hud;
use example_01_platformer::shell::widgets::text_of;
use tungsten::kit::{Health, damage};

use super::run;

#[test]
fn hearts_reflect_every_hp_value_immediately_at_constant_screen_size() {
    // The hearts became the health bar: its fill and its "current / max".
    let (mut harness, player) = run();
    let hud = *harness.world().resource::<Hud>();
    let track = harness.tree().parent(hud.health.fill).unwrap();
    let mut placed = None;
    for zoom in [0.35, 1.0, 3.0] {
        harness.world_mut().resource_mut::<Zoom>().0 = zoom;
        let mut full = None;
        for hp in [100.0, 66.0, 34.0, 1.0] {
            let world = harness.world_mut();
            world.insert(player, Health::new(100.0));
            if hp < 100.0 {
                damage(world, player, 100.0 - hp);
            }
            harness.step(1);
            assert_eq!(
                text_of(harness.tree(), hud.health.text),
                format!("{hp} / 100")
            );
            let width = harness.rect(hud.health.fill).unwrap().size().x;
            let full = *full.get_or_insert(width);
            assert!(
                (width - full * hp / 100.0).abs() < 0.01,
                "zoom {zoom}, {hp} HP: {width} of {full}"
            );
            let rect = harness.rect(track).unwrap();
            let at = *placed.get_or_insert(rect);
            assert_eq!(rect, at, "zoom {zoom}: the bar keeps its place and size");
            assert!(
                rect.min.x < 100.0 && rect.min.y < 100.0,
                "top left: {rect:?}"
            );
        }
    }
}
