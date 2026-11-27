use glam::Vec2;

use super::*;
use crate::AssetRegistry;
use crate::display::ScaleFactor;
use crate::ecs::{EventQueue, World};
use crate::input::{ActionMap, InputState, KeyCode, Modifiers, RoutedInput};
use crate::time::Time;
use crate::ui::{
    FixedAdvanceMeasure, PaintCommand, PaintList, Sizing, UiDebug, UiEvent, UiLayer, UiRouter,
    ui_route_system,
};

const VIEWPORT: Vec2 = Vec2::new(400.0, 300.0);

/// A world the router reads, with a root holding the field `name` (keyed
/// `name`, holding "Ada") and a button under it, its text measured by one
/// fixed-advance store across frames.
struct Rig {
    world: World,
    store: FixedAdvanceMeasure,
    field: WidgetId,
    button: WidgetId,
}

impl Rig {
    fn new() -> Self {
        let mut world = World::new();
        world.insert_resource(InputState::new());
        world.insert_resource(ActionMap::default_map());
        world.insert_resource(Time::new());
        world.insert_resource(ScaleFactor::default());
        world.insert_resource(UiRouter::default());
        world.insert_resource(RoutedInput::default());
        world.register_event::<UiEvent>();
        let mut tree = UiTree::new();
        let root = tree.add_root(
            UiLayer::Game,
            LayoutStyle::default().with_size(Sizing::Px(400.0), Sizing::Px(300.0)),
        );
        let field = tree.text_field(root, "Ada");
        tree.set_key(field, "name");
        let button = tree.button(root, "Go");
        world.insert_resource(tree);
        let mut rig = Self {
            world,
            store: FixedAdvanceMeasure::default(),
            field,
            button,
        };
        rig.layout();
        rig
    }

    fn tree(&self) -> &UiTree {
        self.world.resource::<UiTree>()
    }

    fn tree_mut(&mut self) -> &mut UiTree {
        self.world.resource_mut::<UiTree>()
    }

    fn layout(&mut self) {
        let Self { world, store, .. } = self;
        world.resource_mut::<UiTree>().layout(VIEWPORT, store);
    }

    /// One frame: the raw input `feed` gives, the routing, the layout; its
    /// events.
    fn frame(&mut self, feed: impl FnOnce(&mut InputState)) -> Vec<UiEvent> {
        self.world.flush_events();
        let input = self.world.resource_mut::<InputState>();
        input.begin_frame();
        feed(input);
        self.world.resource_mut::<Time>().advance_frame(1.0 / 32.0);
        ui_route_system(&mut self.world);
        self.layout();
        self.world
            .resource::<EventQueue<UiEvent>>()
            .iter_current()
            .copied()
            .collect()
    }

    fn tap(&mut self, key: KeyCode) -> Vec<UiEvent> {
        let events = self.frame(|input| {
            input.key_down(key);
        });
        self.frame(|input| input.key_up(key));
        events
    }

    fn value(&self) -> &str {
        self.tree().field_value(self.field).unwrap()
    }

    fn caret(&self) -> usize {
        self.tree().field_caret(self.field).unwrap()
    }

    fn focused(&mut self) {
        let field = self.field;
        self.tree_mut().focus(field);
        self.frame(|_| {});
    }

    fn routed(&self) -> &InputState {
        self.world.resource::<RoutedInput>()
    }
}

fn changed(events: &[UiEvent]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, UiEvent::FieldChanged { .. }))
        .count()
}

#[test]
fn typing_inserts_at_the_caret_up_to_its_max() {
    let mut rig = Rig::new();
    rig.focused();
    assert_eq!(rig.caret(), 3, "focus puts the caret at the end");
    let events = rig.frame(|input| input.type_text("\u{8}ms"));
    assert_eq!(rig.value(), "Adams", "control characters left out");
    assert_eq!(changed(&events), 2);
    rig.tap(KeyCode::Home);
    rig.frame(|input| input.type_text("Ø "));
    assert_eq!(
        (rig.value(), rig.caret()),
        ("Ø Adams", 2),
        "a character, not a byte"
    );
    let field = rig.field;
    rig.tree_mut().set_field_max(field, Some(8));
    rig.frame(|input| input.type_text("xyz"));
    assert_eq!(rig.value(), "Ø xAdams", "held to 8 characters");
    rig.tree_mut().set_field_max(field, Some(4));
    assert_eq!(rig.value(), "Ø xA", "a lower bound cuts the value");
    assert!(
        rig.routed().typed().next().is_none(),
        "the field's, not the game's"
    );
    // Unfocused, typed text is the game's.
    rig.tree_mut().blur();
    rig.frame(|input| input.type_text("go"));
    assert_eq!(rig.routed().typed().collect::<String>(), "go");
}

#[test]
fn editing_keys_move_and_delete() {
    let mut rig = Rig::new();
    rig.focused();
    assert_eq!(changed(&rig.tap(KeyCode::Backspace)), 1);
    assert_eq!((rig.value(), rig.caret()), ("Ad", 2));
    rig.tap(KeyCode::ArrowLeft);
    assert_eq!(rig.caret(), 1);
    assert_eq!(changed(&rig.tap(KeyCode::Delete)), 1);
    assert_eq!((rig.value(), rig.caret()), ("A", 1));
    rig.tap(KeyCode::ArrowRight);
    rig.tap(KeyCode::ArrowRight);
    assert_eq!(rig.caret(), 1, "held at the end");
    assert_eq!(
        changed(&rig.tap(KeyCode::Delete)),
        0,
        "nothing after the caret"
    );
    rig.tap(KeyCode::Home);
    assert_eq!(
        changed(&rig.tap(KeyCode::Backspace)),
        0,
        "nothing before it"
    );
    rig.tap(KeyCode::End);
    assert_eq!(rig.caret(), 1);
    // A held Backspace repeats.
    let field = rig.field;
    rig.tree_mut().set_field_value(field, "Abc");
    rig.frame(|input| {
        input.key_down(KeyCode::Backspace);
        input.key_down(KeyCode::Backspace);
    });
    assert_eq!(rig.value(), "A", "the press and its repeat");
}

#[test]
fn enter_commits_and_escape_restores() {
    let mut rig = Rig::new();
    rig.focused();
    rig.frame(|input| input.type_text("m"));
    let events = rig.tap(KeyCode::Enter);
    let committed: Vec<&UiEvent> = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                UiEvent::FieldCommitted { .. } | UiEvent::Activated { .. }
            )
        })
        .collect();
    assert!(matches!(
        committed[..],
        [
            UiEvent::FieldCommitted { key: Some(_), .. },
            UiEvent::Activated { .. }
        ]
    ));
    assert_eq!(rig.tree().focused(), Some(rig.field), "Enter keeps focus");
    rig.frame(|input| input.type_text("os"));
    assert_eq!(rig.value(), "Adamos");
    let events = rig.tap(KeyCode::Escape);
    assert_eq!(rig.value(), "Adam", "back to the commit");
    assert_eq!(changed(&events), 1);
    assert_eq!(rig.tree().focused(), None, "Escape lets focus go");
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, UiEvent::Cancel { .. })),
        "the field's Escape is no ui_cancel"
    );
    // A click focuses; it is no commit.
    let field = rig.field;
    rig.tree_mut().focus(field);
    rig.frame(|_| {});
    let events = rig.tap(KeyCode::Escape);
    assert_eq!(changed(&events), 0, "nothing to restore");
}

#[test]
fn a_focused_field_keeps_its_keys_from_the_game_and_the_ui_actions() {
    let mut rig = Rig::new();
    rig.focused();
    for key in [
        KeyCode::KeyW,
        KeyCode::Space,
        KeyCode::ArrowLeft,
        KeyCode::Escape,
    ] {
        rig.frame(|input| {
            input.key_down(key);
        });
        assert!(!rig.routed().is_pressed(key), "{key:?} reached the game");
        rig.frame(|input| input.key_up(key));
        assert!(!rig.routed().just_released(key), "{key:?}'s release too");
        rig.focused();
    }
    assert_eq!(
        rig.tree().focused(),
        Some(rig.field),
        "Left moved the caret, not focus"
    );
    // Tab still moves focus on.
    rig.tap(KeyCode::Tab);
    assert_eq!(rig.tree().focused(), Some(rig.button));
    // Shift+Tab back.
    rig.frame(|input| {
        input.set_modifiers(Modifiers::SHIFT);
        input.key_down(KeyCode::Tab);
    });
    assert_eq!(rig.tree().focused(), Some(rig.field));
    rig.frame(|input| {
        input.key_up(KeyCode::Tab);
        input.set_modifiers(Modifiers::empty());
    });
    // Accept on the field commits; no other activation.
    let events = rig.tap(KeyCode::Enter);
    let activated: Vec<WidgetId> = events
        .iter()
        .filter_map(|event| match event {
            UiEvent::Activated { widget, .. } => Some(*widget),
            _ => None,
        })
        .collect();
    assert_eq!(activated, [rig.field]);
}

#[test]
fn the_caret_sits_after_the_measured_prefix() {
    let mut rig = Rig::new();
    rig.focused();
    let x = |rig: &Rig| rig.tree().field(rig.field).unwrap().caret_x;
    let after_three = x(&rig);
    assert!(after_three > 0.0);
    rig.tap(KeyCode::Home);
    assert_eq!(x(&rig), 0.0, "before the first glyph");
    rig.tap(KeyCode::ArrowRight);
    let one = x(&rig);
    assert!(
        (one * 3.0 - after_three).abs() < 1e-3,
        "fixed advances: {one} {after_three}"
    );
    // The caret paints while the field holds focus, the line's height, at
    // the label's start plus the prefix.
    let mut list = PaintList::default();
    rig.tree_mut()
        .paint(1.0, &UiDebug::default(), &AssetRegistry::new(), &mut list);
    let label = rig.tree().children(rig.field)[0];
    let label_x = rig.tree().rect(label).unwrap().min.x;
    let caret = list.commands.iter().find_map(|command| match command {
        PaintCommand::Rect { rect, .. } if (rect.width() - 1.0).abs() < 1e-3 => Some(*rect),
        _ => None,
    });
    let caret = caret.expect("a 1-unit caret");
    assert!(
        (caret.min.x - (label_x + one)).abs() < 1e-3,
        "{caret:?} {label_x} {one}"
    );
    rig.tree_mut().blur();
    rig.layout();
    let mut list = PaintList::default();
    rig.tree_mut()
        .paint(1.0, &UiDebug::default(), &AssetRegistry::new(), &mut list);
    assert!(
        !list
            .commands
            .iter()
            .any(|command| matches!(command, PaintCommand::Rect { rect, .. } if (rect.width() - 1.0).abs() < 1e-3)),
        "no caret without focus"
    );
}
