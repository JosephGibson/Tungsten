//! The `story` scene on the headless harness (redo 8.1): the fixtures'
//! manifest loads beside the kit, its screen files check with the story's
//! names, and the demo's path runs: the name typed, the talk, the quest
//! taken, the lamp lit, the journal, and the quest handed in.

use std::path::PathBuf;

use example_03_fixtures::story::{self, StoryScene};
use tungsten::App;
use tungsten::core::ui::{UiBindings, UiKey, UiScreenRegistry};
use tungsten::core::{Config, DataRegistry, Entity, KeyCode};
use tungsten::kit::{
    Barks, Conversation, Counters, Dialogue, Flags, Interactable, Interactor, Quest, QuestStatus,
    Quests, Speaker,
};
use tungsten::story::{DIALOGUE_SCREEN, JOURNAL_SCREEN};
use tungsten::testing::UiHarness;

/// The scene on the harness: configured as `run` does, the fixtures'
/// manifest loaded, the room set up, its slide-ins settled.
fn rig() -> UiHarness {
    let mut app = App::new(Config::default()).expect("App::new failed");
    story::configure(&mut app);
    let mut harness = UiHarness::new(app);
    harness
        .harness_mut()
        .load_manifest(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/manifest.json"))
        .expect("the fixtures' manifest loads beside the kit");
    story::setup(harness.world_mut());
    harness.step(12);
    harness
}

fn top(harness: &UiHarness) -> Option<&'static str> {
    harness
        .world()
        .resource::<tungsten::StateStack>()
        .active_id()
}

fn scene(harness: &UiHarness) -> &StoryScene {
    harness.world().resource::<StoryScene>()
}

fn focus(harness: &UiHarness) -> Option<Entity> {
    harness
        .world()
        .get::<Interactor>(scene(harness).warden)
        .and_then(Interactor::focus)
}

fn line(harness: &UiHarness) -> Option<String> {
    let talk = harness.world().resource::<Dialogue>().current()?;
    Some(talk.line()?.text().to_owned())
}

fn quest(harness: &UiHarness) -> Option<(QuestStatus, String)> {
    let quests = harness.world().get::<Quests>(scene(harness).warden)?;
    let state = quests.get(story::QUEST)?;
    Some((state.status(), state.stage().to_owned()))
}

fn tap(harness: &mut UiHarness, key: KeyCode) {
    harness.key_down(key);
    harness.step(1);
    harness.key_up(key);
    harness.step(1);
}

/// Holds `action` until the warden's focus is `target`, at most 600 frames.
fn walk_until(harness: &mut UiHarness, action: &str, target: Entity) {
    harness.harness_mut().press_action(action);
    for _ in 0..600 {
        if focus(harness) == Some(target) {
            break;
        }
        harness.step(1);
    }
    harness.harness_mut().release_action(action);
    harness.step(2);
    assert_eq!(focus(harness), Some(target), "{action} reached its target");
}

/// Accept until the talk ends: the rest of each revealing line, then the
/// next, at most 40 presses; `false` when choices show first.
fn read_on(harness: &mut UiHarness) -> bool {
    for _ in 0..40 {
        let Some(talk) = harness.world().resource::<Dialogue>().current() else {
            return true;
        };
        if talk.choosing() {
            return false;
        }
        tap(harness, KeyCode::Enter);
    }
    panic!("the talk ran past 40 presses");
}

/// Takes the shown choice `index` through its row.
fn choose(harness: &mut UiHarness, index: usize) {
    let row = harness
        .find_key(UiKey::new(&format!("choices/{index}")))
        .unwrap_or_else(|| panic!("choice row {index}"));
    assert!(harness.focus(row));
    tap(harness, KeyCode::Enter);
}

#[test]
fn the_story_scene_loads_and_its_screens_check() {
    let harness = rig();
    assert_eq!(
        top(&harness),
        Some(story::NAME_SCREEN),
        "the card opens first"
    );
    let world = harness.world();
    let data = world.resource::<DataRegistry>();
    for id in ["keeper", "warden", "notice"] {
        assert!(data.get::<Speaker>(id).is_some(), "speaker {id}");
    }
    for id in [story::KEEPER_TALK, story::NOTICE_TALK] {
        assert!(data.get::<Conversation>(id).is_some(), "conversation {id}");
    }
    assert!(data.get::<Quest>(story::QUEST).is_some());
    let scene = scene(&harness);
    assert!(world.get::<Interactor>(scene.warden).is_some());
    assert!(world.get::<Quests>(scene.warden).is_some());
    for entity in [scene.keeper, scene.lamp, scene.notice] {
        assert!(world.get::<Interactable>(entity).is_some());
    }
    let names = world.resource::<UiBindings>().names();
    let files = world.resource::<UiScreenRegistry>();
    for (id, data_screen) in [
        (DIALOGUE_SCREEN, false),
        (JOURNAL_SCREEN, true),
        (story::NAME_SCREEN, true),
    ] {
        let file = files
            .get(id)
            .unwrap_or_else(|| panic!("the manifest lists `{id}`"));
        assert!(
            file.check(&names).is_empty(),
            "{id}: {:?}",
            file.check(&names)
        );
        if data_screen {
            assert_eq!(file.data_screen_refusal(), None, "{id}");
        }
    }
}

#[test]
fn the_warden_talks_lights_the_lamp_and_moves_the_quest() {
    let mut harness = rig();
    let StoryScene { keeper, lamp, .. } = *scene(&harness);

    // The card: a name typed and committed.
    let field = harness
        .find_key(UiKey::new(story::NAME_KEY))
        .expect("the card's field");
    assert_eq!(
        harness.tree().focused(),
        Some(field),
        "the card focuses its field"
    );
    harness.type_text("Ada");
    harness.step(1);
    tap(&mut harness, KeyCode::Enter);
    harness.step(12);
    assert_eq!(scene(&harness).name, "Ada");
    assert_eq!(top(&harness), Some(story::PLAY), "the card closed");

    // The keeper: a walk, Interact, the greeting read, the lamp taken on.
    walk_until(&mut harness, "move_right", keeper);
    harness.tap_action(story::INTERACT);
    harness.step(12);
    assert_eq!(top(&harness), Some(DIALOGUE_SCREEN));
    assert_eq!(line(&harness).as_deref(), Some("Evening, warden."));
    assert!(!read_on(&mut harness), "the offer's choices");
    choose(&mut harness, 0);
    assert_eq!(
        line(&harness).as_deref(),
        Some("Bless you. Mind the steps.")
    );
    assert_eq!(quest(&harness), Some((QuestStatus::Active, "light".into())));
    assert!(read_on(&mut harness), "the thanks end the talk");
    harness.step(12);
    assert_eq!(top(&harness), Some(story::PLAY), "the box closed");

    // The lamp: lit, its bark, the quest at its report.
    walk_until(&mut harness, "move_up", lamp);
    harness.tap_action(story::INTERACT);
    harness.step(2);
    let world = harness.world();
    assert!(world.resource::<Flags>().has(story::LAMP_LIT));
    assert_eq!(world.resource::<Barks>().text_of(lamp), Some("It catches."));
    assert_eq!(
        quest(&harness),
        Some((QuestStatus::Active, "report".into()))
    );

    // The journal over the warden's quests.
    harness.tap_action(story::JOURNAL);
    harness.step(12);
    assert_eq!(top(&harness), Some(JOURNAL_SCREEN));
    assert!(harness.shows("Lamplight"));
    assert!(harness.shows("Tell Tamsin it burns."));
    tap(&mut harness, KeyCode::Escape);
    harness.step(12);
    assert_eq!(top(&harness), Some(story::PLAY), "the journal closed");

    // The keeper again: the quest handed in for its coins.
    walk_until(&mut harness, "move_down", keeper);
    harness.tap_action(story::INTERACT);
    harness.step(12);
    assert_eq!(
        line(&harness).as_deref(),
        Some("It burns. Thank you, warden.")
    );
    assert_eq!(quest(&harness), Some((QuestStatus::Done, "report".into())));
    assert_eq!(harness.world().resource::<Counters>().get("coins"), 5);
}
