use std::path::PathBuf;

use tungsten_core::save::{capture_world, restore_world};
use tungsten_core::ui::{ScreenFile, UiKey};
use tungsten_core::{Config, Entity, KeyCode};

use super::*;
use crate::App;
use crate::kit::{Flags, KitPlugins, talk as start_talk};
use crate::screens::ScreensPlugin;
use crate::state::StateStack;
use crate::testing::UiHarness;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/story")
}

/// A harness with the kit, the screens and the box, its portraits interned,
/// the story fixture loaded when `file`, else its data alone; a keeper and
/// a warden.
fn rig(file: bool) -> (UiHarness, Entity, Entity) {
    let mut app = App::new(Config::default()).expect("App::new failed");
    app.add_plugins(KitPlugins::set());
    app.add_plugin(ScreensPlugin);
    app.add_plugin(StoryUiPlugin);
    app.world_mut()
        .resource_mut::<AssetRegistry>()
        .intern_sprites(["keeper_face", "keeper_worried"]);
    let mut harness = UiHarness::new(app);
    harness
        .harness_mut()
        .load_manifest(fixture().join("manifest.json"))
        .expect("the story fixture loads");
    if !file {
        let mut registry = UiScreenRegistry::new();
        std::mem::swap(
            harness.world_mut().resource_mut::<UiScreenRegistry>(),
            &mut registry,
        );
    }
    let world = harness.world_mut();
    let keeper = world.spawn();
    let warden = world.spawn();
    (harness, keeper, warden)
}

/// The talk started and the box's slide in finished.
fn talking(harness: &mut UiHarness, keeper: Entity, warden: Entity) {
    assert!(start_talk(
        harness.world_mut(),
        "keeper.evening",
        keeper,
        warden
    ));
    harness.step(12);
}

fn top(harness: &UiHarness) -> Option<&'static str> {
    harness.world().resource::<StateStack>().active_id()
}

fn line(harness: &UiHarness) -> Option<String> {
    let talk = harness.world().resource::<Dialogue>().current()?;
    Some(talk.line()?.text().to_owned())
}

fn tap(harness: &mut UiHarness, key: KeyCode) {
    harness.key_down(key);
    harness.step(1);
    harness.key_up(key);
    harness.step(1);
}

/// The box's line node, keyed `text`.
fn text_node(harness: &UiHarness) -> WidgetId {
    harness
        .find_key(UiKey::new(TEXT_KEY))
        .expect("the line node")
}

fn shown(harness: &UiHarness) -> Option<u32> {
    harness.world().resource::<DialogueBoxView>().revealing()
}

/// Presses Accept until the line is whole.
fn reveal_whole(harness: &mut UiHarness) {
    if shown(harness).is_some() {
        tap(harness, KeyCode::Enter);
    }
    assert_eq!(shown(harness), None, "whole");
}

#[test]
fn a_talk_opens_the_box_and_its_end_pops_it() {
    let (mut harness, keeper, warden) = rig(true);
    harness.step(2);
    assert_ne!(top(&harness), Some(DIALOGUE_SCREEN), "no talk, no box");
    talking(&mut harness, keeper, warden);
    assert_eq!(top(&harness), Some(DIALOGUE_SCREEN));
    assert!(harness.world().resource::<DialogueBoxView>().up());
    assert!(harness.shows("Old Tamsin"), "the speaker's name bound");
    Dialogue::end(harness.world_mut());
    harness.step(12);
    assert_ne!(top(&harness), Some(DIALOGUE_SCREEN), "the end popped it");
    assert!(!harness.world().resource::<DialogueBoxView>().up());
}

#[test]
fn accept_shows_the_rest_then_advances() {
    let (mut harness, keeper, warden) = rig(true);
    talking(&mut harness, keeper, warden);
    assert_eq!(line(&harness).as_deref(), Some("Evening, warden."));
    assert!(shown(&harness).is_some(), "still revealing after 12 frames");
    tap(&mut harness, KeyCode::Enter);
    assert_eq!(shown(&harness), None, "the rest shown");
    assert_eq!(
        line(&harness).as_deref(),
        Some("Evening, warden."),
        "not advanced yet"
    );
    tap(&mut harness, KeyCode::Enter);
    assert_eq!(line(&harness).as_deref(), Some("Cold tonight."));
    assert!(shown(&harness).is_some(), "a new line reveals again");
}

#[test]
fn the_reveal_climbs_at_the_speakers_pace_and_lays_out_once_a_line() {
    let (mut harness, keeper, warden) = rig(true);
    harness.harness_mut().set_dt(1.0 / 60.0);
    talking(&mut harness, keeper, warden);
    let text = text_node(&harness);
    let before = harness.layout_totals();
    let start = shown(&harness).expect("revealing");
    harness.step(9);
    let after = shown(&harness).expect("still revealing: 16 characters at 40 a second");
    assert!(
        (5..=7).contains(&(after - start)),
        "0.15 s at 40 a second: {start} → {after}"
    );
    let count = harness
        .world()
        .resource::<tungsten_core::ui::UiReveals>()
        .get(text);
    assert_eq!(count, Some(after), "the label's count");
    assert_eq!(
        harness.layout_totals(),
        before,
        "the climb lays nothing out"
    );
    harness.step(60);
    assert_eq!(shown(&harness), None, "whole by its length");
    let whole = harness.layout_totals();
    tap(&mut harness, KeyCode::Enter);
    assert_eq!(line(&harness).as_deref(), Some("Cold tonight."));
    assert_eq!(
        harness.layout_totals().roots_laid_out,
        whole.roots_laid_out + 1,
        "a new line lays its root out once"
    );
}

#[test]
fn a_choice_row_chooses_its_index() {
    let (mut harness, keeper, warden) = rig(true);
    talking(&mut harness, keeper, warden);
    for _ in 0..3 {
        reveal_whole(&mut harness);
        tap(&mut harness, KeyCode::Enter);
    }
    let talk = harness.world().resource::<Dialogue>().current().unwrap();
    assert!(talk.choosing());
    let asked = harness
        .find_key(UiKey::new("choices/1"))
        .expect("the second row");
    assert!(harness.focus(asked));
    tap(&mut harness, KeyCode::Enter);
    assert!(harness.world().resource::<Flags>().has("lamp.asked"));
    assert_eq!(
        line(&harness).as_deref(),
        Some("The lamp by the steps is out again."),
        "back at the offer"
    );
    reveal_whole(&mut harness);
    tap(&mut harness, KeyCode::Enter);
    assert!(
        harness.find_key(UiKey::new("choices/1")).is_none(),
        "the asked choice gone"
    );
    let light = harness
        .find_key(UiKey::new("choices/0"))
        .expect("the first row");
    assert!(harness.focus(light));
    tap(&mut harness, KeyCode::Enter);
    assert!(harness.world().resource::<Flags>().has("lamp.promised"));
    assert_eq!(line(&harness).as_deref(), Some("Bless you."));
}

#[test]
fn cancel_takes_the_back_choice_or_nothing() {
    let (mut harness, keeper, warden) = rig(true);
    talking(&mut harness, keeper, warden);
    tap(&mut harness, KeyCode::Escape);
    assert_eq!(
        top(&harness),
        Some(DIALOGUE_SCREEN),
        "no back choice shows: nothing"
    );
    assert_eq!(line(&harness).as_deref(), Some("Evening, warden."));
    for _ in 0..3 {
        reveal_whole(&mut harness);
        tap(&mut harness, KeyCode::Enter);
    }
    assert!(
        harness
            .world()
            .resource::<Dialogue>()
            .current()
            .unwrap()
            .choosing()
    );
    tap(&mut harness, KeyCode::Escape);
    assert!(
        harness.world().resource::<Dialogue>().current().is_none(),
        "`Not tonight.` ends the talk"
    );
    harness.step(12);
    assert_ne!(top(&harness), Some(DIALOGUE_SCREEN));
}

#[test]
fn the_portrait_follows_the_lines_mood() {
    let (mut harness, keeper, warden) = rig(true);
    talking(&mut harness, keeper, warden);
    let image = harness
        .world()
        .resource::<DialogueBoxView>()
        .portrait()
        .expect("the mount's image");
    let sprite = |harness: &UiHarness, name: &str| {
        harness.world().resource::<AssetRegistry>().sprite_id(name)
    };
    let showing = |harness: &UiHarness| {
        let tree = harness.tree();
        (tree.image_sprite(image), tree.visibility(image))
    };
    assert_eq!(
        showing(&harness),
        (sprite(&harness, "keeper_face"), Some(Visibility::Visible)),
        "neutral"
    );
    for _ in 0..2 {
        reveal_whole(&mut harness);
        tap(&mut harness, KeyCode::Enter);
    }
    assert_eq!(
        showing(&harness).0,
        sprite(&harness, "keeper_worried"),
        "worried"
    );
    reveal_whole(&mut harness);
    tap(&mut harness, KeyCode::Enter);
    assert_eq!(
        showing(&harness).1,
        Some(Visibility::Collapsed),
        "choices: no line"
    );
}

#[test]
fn without_a_screen_file_the_box_warns_and_the_talk_stays() {
    let (mut harness, keeper, warden) = rig(false);
    talking(&mut harness, keeper, warden);
    assert_ne!(top(&harness), Some(DIALOGUE_SCREEN));
    assert!(harness.world().resource::<DialogueBoxView>().warned);
    harness.step(30);
    assert_eq!(
        line(&harness).as_deref(),
        Some("Evening, warden."),
        "the talk stays"
    );
    assert!(
        Dialogue::advance(harness.world_mut()),
        "for the game's own Rust"
    );
}

#[test]
fn the_screen_file_checks_with_the_plugins_names() {
    let (harness, ..) = rig(true);
    let json = std::fs::read_to_string(fixture().join("ui/screens/dialogue.json")).unwrap();
    let file = ScreenFile::from_json(&json).unwrap();
    let names = harness.world().resource::<UiBindings>().names();
    assert!(file.check(&names).is_empty(), "{:?}", file.check(&names));
    assert!(file.data_screen_refusal().is_none());
}

#[test]
fn a_restore_pops_the_box() {
    let (mut harness, keeper, warden) = rig(true);
    let saved = capture_world(harness.world()).unwrap();
    talking(&mut harness, keeper, warden);
    assert_eq!(top(&harness), Some(DIALOGUE_SCREEN));
    restore_world(harness.world_mut(), &saved, &(Restored,)).unwrap();
    assert!(harness.world().resource::<Dialogue>().current().is_none());
    harness.step(12);
    assert_ne!(top(&harness), Some(DIALOGUE_SCREEN), "the box closed");
}

/// What a restore's spawns take.
#[derive(Debug, Clone, Copy)]
struct Restored;
