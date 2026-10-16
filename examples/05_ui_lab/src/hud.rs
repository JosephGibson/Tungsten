//! HUD rows: pointer-transparent roots on the game layer whose text changes
//! every frame without allocating. The lab HUD shows an icon and a frame
//! counter in the mono family, whose digits share one advance; the play
//! HUD shows a game-time timer and a prompt that names the pause action's
//! binding for the last device (`D-163`). Both write with `write!` into a
//! retained `String` and `UiTree::set_plain_text`, so an unchanged text
//! costs a compare (w01 §14).

use std::fmt::Write as _;

use glam::Vec2;
use tungsten::core::ui::{Align, Anchor, LayoutStyle, Sizing, TextOverrides, UiLayer, UiTree};
use tungsten::core::ui::{StyleOverrides, WidgetId};
use tungsten::core::{AssetRegistry, Time, World};

use crate::{theme, widgets};

/// The HUD icon's sprite ID.
pub const ICON_ID: &str = "ex05_icon";

/// The lab HUD's widgets and the frames it has counted; a `World`
/// resource.
#[derive(Debug)]
pub struct LabHud {
    /// The row's root.
    pub root: WidgetId,
    /// The frame counter.
    pub counter: WidgetId,
    /// Frames the counter has shown.
    pub frames: u64,
}

/// The mono family, whose digits share one advance.
fn mono() -> StyleOverrides {
    widgets::text(widgets::value_text(TextOverrides {
        family: Some("mono".to_owned()),
        ..TextOverrides::default()
    }))
}

/// Builds the lab HUD into `tree` at the top start corner: an icon beside
/// a counter, none of it hit by the pointer. The icon's sprite is interned
/// through `sprites`, so a headless test builds the same tree.
pub fn build_lab_hud(tree: &mut UiTree, sprites: &mut AssetRegistry) -> LabHud {
    let icon = sprites.intern_sprite(ICON_ID);
    let mut style = LayoutStyle::row()
        .with_padding(theme::SPACE[1])
        .with_gap(theme::SPACE[1])
        .anchored(Anchor::TopStart, Vec2::splat(theme::SCREEN_MARGIN));
    style.align_items = Align::Center;
    let root = tree.add_root(UiLayer::Game, style);
    tree.set_hit_testable(root, false);
    let hud_icon = tree.image(
        root,
        icon,
        Vec2::splat(theme::ICON),
        LayoutStyle::default().with_size(Sizing::Px(theme::ICON), Sizing::Px(theme::ICON)),
    );
    tree.set_hit_testable(hud_icon, false);
    let counter = tree.label(root, "frame 0");
    tree.set_hit_testable(counter, false);
    tree.set_overrides(counter, mono());
    LabHud {
        root,
        counter,
        frames: 0,
    }
}

/// The counter's text, retained so the counter writes without allocating;
/// a `World` resource.
#[derive(Debug, Default)]
pub struct CounterText(String);

/// Writes the frame count into the lab HUD's counter.
pub fn counter_system(world: &mut World) {
    let Some(lab_hud) = world.get_resource_mut::<LabHud>() else {
        return;
    };
    lab_hud.frames += 1;
    let (frames, counter) = (lab_hud.frames, lab_hud.counter);
    let Some(mut text) = world.remove_resource::<CounterText>() else {
        return;
    };
    text.0.clear();
    let _ = write!(text.0, "frame {frames}");
    if let Some(tree) = world.get_resource_mut::<UiTree>() {
        tree.set_plain_text(counter, &text.0);
    }
    world.insert_resource(text);
}

/// The play state's HUD row: a `World` resource while `play` is on the
/// stack.
#[derive(Debug)]
pub struct PlayHud {
    /// The row's root.
    pub root: WidgetId,
    /// Game seconds, written with `set_plain_text` from the retained text.
    pub timer: WidgetId,
    /// "Press {action:state_pause} to pause".
    pub prompt: WidgetId,
    text: String,
}

/// Builds the play HUD into `tree` at the bottom start corner: the timer in
/// the mono family and the pause prompt, which the router rebuilds when the
/// binding or the last device changes.
pub fn build_play_hud(tree: &mut UiTree) -> PlayHud {
    let mut style = LayoutStyle::row()
        .with_padding(theme::SPACE[1])
        .with_gap(theme::SPACE[3])
        .anchored(Anchor::BottomStart, Vec2::splat(theme::SCREEN_MARGIN));
    style.align_items = Align::Center;
    let root = tree.add_root(UiLayer::Game, style);
    tree.set_hit_testable(root, false);
    let timer = tree.label(root, "0.0 s");
    tree.set_overrides(timer, mono());
    let prompt = tree.label(root, "");
    tree.set_prompt(prompt, "Press {action:state_pause} to pause");
    PlayHud {
        root,
        timer,
        prompt,
        text: String::with_capacity(16),
    }
}

/// Writes the game clock's seconds into the play HUD's timer.
pub fn write_play_timer(world: &mut World) {
    let seconds = world.resource::<Time>().elapsed();
    if let Some(mut hud) = world.remove_resource::<PlayHud>() {
        hud.text.clear();
        let _ = write!(hud.text, "{seconds:.1} s");
        world
            .resource_mut::<UiTree>()
            .set_plain_text(hud.timer, &hud.text);
        world.insert_resource(hud);
    }
}
