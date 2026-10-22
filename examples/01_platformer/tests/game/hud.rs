//! The HUD: the health bar answers every hit by the next frame, at one
//! size and place on screen whatever the camera's zoom; the summon slot
//! shows the selected type's icon, whichever way the type turns.

use example_01_platformer::features::level::components::Zoom;
use example_01_platformer::features::spells::components::Summon;
use example_01_platformer::features::spells::hud::Hud;
use example_01_platformer::features::spells::{SUMMON_NEXT, SUMMON_PREV, SUMMONS, summon_type};
use example_01_platformer::shell::widgets::text_of;
use glam::Vec2;
use tungsten::core::AssetRegistry;
use tungsten::kit::{Health, damage};
use tungsten::testing::UiHarness;

use super::{run, steps_this_frame};

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

/// Presses `action` until a fixed step has read it.
fn turn(harness: &mut UiHarness, action: &str) {
    harness.harness_mut().press_action(action);
    loop {
        harness.step(1);
        if steps_this_frame(harness) > 0 {
            break;
        }
    }
    harness.harness_mut().release_action(action);
    harness.step(1);
}

#[test]
fn cycling_the_summon_type_names_the_slots_icon() {
    let (mut harness, _) = run();
    let hud = *harness.world().resource::<Hud>();
    let icon = hud.slots[9].icon.expect("slot 0 summons");
    let sprite = |harness: &mut UiHarness, name: &str| {
        harness
            .world_mut()
            .resource_mut::<AssetRegistry>()
            .intern_sprite(name)
    };
    let dummy = sprite(&mut harness, SUMMONS[0].icon);
    assert_eq!(harness.tree().image_sprite(icon), Some(dummy));
    // Each way steps through the types, wrapping, and the slot names the
    // type's icon, whatever it showed before.
    let last = SUMMONS.len() - 1;
    for (action, selected) in [(SUMMON_NEXT, 1), (SUMMON_PREV, 0), (SUMMON_PREV, last)] {
        let other = sprite(&mut harness, "ex10_icon_gun");
        harness
            .world_mut()
            .resource_mut::<tungsten::core::ui::UiTree>()
            .set_image(icon, other, Vec2::splat(32.0));
        turn(&mut harness, action);
        assert_eq!(
            harness.world().resource::<Summon>().selected,
            selected,
            "{action}"
        );
        assert_eq!(summon_type(harness.world()).name, SUMMONS[selected].name);
        let shown = sprite(&mut harness, SUMMONS[selected].icon);
        assert_eq!(harness.tree().image_sprite(icon), Some(shown), "{action}");
    }
}

#[test]
fn full_mana_reads_100_of_100_on_one_line() {
    // The readouts never wrap, a full 100 / 100 included, and read whole
    // numbers: health rounded up while alive, mana down.
    use tungsten::core::text::TextWrap;
    let (mut harness, player) = run();
    let hud = *harness.world().resource::<Hud>();
    for meter in [hud.health, hud.mana] {
        let text = harness.tree().overrides(meter.text).unwrap().text.clone();
        assert_eq!(text.wrap, Some(TextWrap::None));
        let room = harness.tree().layout_style(meter.text).unwrap().min_width;
        assert!(room.is_some_and(|room| room >= 96.0), "{room:?}");
    }
    harness.step(1);
    assert_eq!(text_of(harness.tree(), hud.mana.text), "100 / 100");
    assert_eq!(text_of(harness.tree(), hud.health.text), "100 / 100");
    damage(harness.world_mut(), player, 12.5);
    harness.step(1);
    assert_eq!(text_of(harness.tree(), hud.health.text), "88 / 100");
}
