//! The HUD: the health bar answers every hit by the next frame, at one
//! size and place on screen whatever the camera's zoom; the summon slot
//! shows the selected type's icon and its name, whichever way the type
//! turns; each slot shows its live key; the bars keep their own colours
//! and the ring takes the focus colour; a slot flashes as a long cooldown
//! ends and red on a press refused as not ready.

use example_01_platformer::features::level::components::Zoom;
use example_01_platformer::features::spells::components::Spell;
use example_01_platformer::features::spells::components::Summon;
use example_01_platformer::features::spells::hud::Hud;
use example_01_platformer::features::spells::{
    SLOT_ACTIONS, SUMMON_NEXT, SUMMON_PREV, SUMMONS, slot_of, summon_type,
};
use example_01_platformer::shell::widgets::text_of;
use glam::Vec2;
use tungsten::core::ui::Visibility;
use tungsten::core::ui::WidgetId;
use tungsten::core::{ActionMap, AssetRegistry, Binding, KeyCode, MouseButton};
use tungsten::kit::{Health, Modifier, Shield, Stats, damage};
use tungsten::testing::UiHarness;

use super::{choose, run, steps_this_frame};

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
    let icon = hud.slots[slot_of(Spell::Summon).unwrap()]
        .icon
        .expect("the summon's slot");
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
        // Its name over slot 0, capitalised, on a plate that shows.
        let name = text_of(harness.tree(), hud.name).to_lowercase();
        assert_eq!(name, SUMMONS[selected].name, "{action}");
        assert!(
            harness.tree().opacity(hud.name_plate) == Some(1.0),
            "{action}"
        );
    }
    // Gone after its 1.2 s.
    harness.step(80);
    assert_eq!(harness.tree().opacity(hud.name_plate), Some(0.0));
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
        assert!(room.is_some_and(|room| room >= 84.0), "{room:?}");
    }
    harness.step(1);
    assert_eq!(text_of(harness.tree(), hud.mana.text), "100 / 100");
    assert_eq!(text_of(harness.tree(), hud.health.text), "100 / 100");
    damage(harness.world_mut(), player, 12.5);
    harness.step(1);
    assert_eq!(text_of(harness.tree(), hud.health.text), "88 / 100");
}

#[test]
fn each_slot_shows_its_live_key_and_a_rebind_at_once() {
    let (mut harness, _) = run();
    harness.step(1);
    let hud = *harness.world().resource::<Hud>();
    let keys: Vec<String> = hud
        .slots
        .iter()
        .map(|slot| text_of(harness.tree(), slot.key))
        .collect();
    assert_eq!(
        keys,
        ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-"]
    );
    let left = Binding::Mouse {
        button: MouseButton::Left,
    };
    harness
        .world_mut()
        .resource_mut::<ActionMap>()
        .replace_bindings(SLOT_ACTIONS[0], vec![left]);
    harness.step(1);
    assert_eq!(text_of(harness.tree(), hud.slots[0].key), "LMB");
}

#[test]
fn the_bars_keep_their_own_colours_and_the_ring_takes_focus() {
    let (mut harness, player) = run();
    harness.step(1);
    let hud = *harness.world().resource::<Hud>();
    let colors = harness.tree().theme().tokens.colors;
    // Full, then low, its border beating: never the accent.
    for hp in [100.0, 20.0] {
        let world = harness.world_mut();
        world.insert(player, Health::new(100.0));
        if hp < 100.0 {
            damage(world, player, 100.0 - hp);
        }
        for _ in 0..30 {
            harness.step(1);
            let tree = harness.tree();
            for meter in [hud.health, hud.mana] {
                let mut ids = vec![
                    meter.track,
                    meter.well,
                    meter.fill,
                    meter.strip,
                    meter.pulse,
                    meter.edge,
                    meter.text,
                ];
                ids.extend_from_slice(tree.children(meter.strip));
                for id in ids {
                    let style = tree.overrides(id).cloned().unwrap_or_default();
                    for color in [style.background, style.border, style.text.color] {
                        assert_ne!(color, Some(colors.accent), "{hp} HP: {id:?}");
                    }
                }
            }
        }
    }
    let selected = harness
        .world()
        .get::<example_01_platformer::features::spells::components::Spellbook>(player)
        .unwrap()
        .slot;
    let ring = harness.tree().overrides(hud.slots[selected].ring).cloned();
    assert_eq!(ring.and_then(|style| style.border), Some(colors.focus));
}

/// The colour a slot's flash shows, none while it is clear.
fn flash(harness: &UiHarness, flash: WidgetId) -> Option<[u8; 4]> {
    let color = harness.tree().overrides(flash)?.background?;
    (color[3] > 0).then_some(color)
}

#[test]
fn a_slot_flashes_when_ready_and_red_when_refused() {
    let (mut harness, _) = run();
    harness.step(1);
    let hud = *harness.world().resource::<Hud>();
    let colors = harness.tree().theme().tokens.colors;
    // The force push, slot 5: a second's cooldown.
    choose(&mut harness, Spell::ForcePush);
    let push = hud.slots[slot_of(Spell::ForcePush).unwrap()]
        .flash
        .expect("the force push's flash");
    harness.harness_mut().press_action("cast");
    harness.step(1);
    harness.harness_mut().release_action("cast");
    harness.step(1);
    assert_eq!(flash(&harness, push), None, "cast, no flash");
    // Pressed again while it cools: refused, red.
    harness.harness_mut().press_action("cast");
    harness.step(1);
    harness.harness_mut().release_action("cast");
    let red = flash(&harness, push).expect("a red flash");
    assert_eq!(red[..3], colors.danger[..3]);
    // Ready again: white, the frame its cooldown ends, then gone.
    let mut white = false;
    for _ in 0..90 {
        harness.step(1);
        if let Some(color) = flash(&harness, push)
            && color[..3] == [255; 3]
        {
            white = true;
            break;
        }
    }
    assert!(white, "a white flash as it is ready");
    harness.step(20);
    assert_eq!(flash(&harness, push), None, "gone after 0.25 s");
}

#[test]
fn the_shield_reads_its_share_of_the_bar_and_its_plus_and_hides_at_zero() {
    let (mut harness, player) = run();
    let hud = *harness.world().resource::<Hud>();
    let stats = harness.world_mut().get_mut::<Stats>(player).unwrap();
    stats.add_modifier(Stats::MAX_HEALTH, "tome.pact", Modifier::Mul(0.5));
    let shield = |current: f32| {
        let fields = serde_json::json!({ "current": current, "max": 50.0 });
        serde_json::from_value::<Shield>(fields).expect("a shield")
    };
    for (current, share, plus) in [(50.0, 1.0, "+50"), (25.0, 0.5, "+25")] {
        harness.world_mut().insert(player, shield(current));
        harness.step(1);
        assert_eq!(text_of(harness.tree(), hud.health.text), "50 / 50");
        assert_eq!(text_of(harness.tree(), hud.shield.text), plus);
        let well = harness.rect(hud.health.well).unwrap().size().x;
        let layer = harness.rect(hud.shield.fill).unwrap().size().x;
        assert!(
            (layer - well * share).abs() < 0.01,
            "{current}: {layer} of {well}"
        );
    }
    harness.world_mut().insert(player, shield(0.0));
    harness.step(1);
    let tree = harness.tree();
    assert_eq!(
        tree.visibility(hud.shield.text),
        Some(Visibility::Collapsed)
    );
    assert_eq!(
        tree.visibility(hud.shield.fill),
        Some(Visibility::Collapsed)
    );
}

#[test]
fn the_flask_takes_key_5_the_summon_minus_and_the_wheel_zooms() {
    let (mut harness, player) = run();
    harness.harness_mut().set_dt(1.0 / 60.0);
    harness.step(1);
    let selected =
        |h: &UiHarness| example_01_platformer::features::spells::selected(h.world(), player);
    for (key, spell) in [
        (KeyCode::Digit5, Spell::ToxicFlask),
        (KeyCode::Minus, Spell::Summon),
    ] {
        harness.key_down(key);
        for _ in 0..4 {
            harness.step(1);
            if steps_this_frame(&harness) > 0 {
                break;
            }
        }
        harness.key_up(key);
        harness.step(1);
        assert_eq!(selected(&harness), Some(spell), "{key:?}");
    }
    // The wheel down zooms out, up back in, and the spell stays chosen.
    let zoom = |h: &UiHarness| h.world().resource::<Zoom>().0;
    let before = zoom(&harness);
    harness.wheel(-1.0);
    harness.step(2);
    let out = zoom(&harness);
    assert!(out < before, "down zooms out: {before} → {out}");
    harness.wheel(1.0);
    harness.step(2);
    assert_eq!(zoom(&harness), before, "up zooms back in");
    assert_eq!(selected(&harness), Some(Spell::Summon), "no spell turned");
}
