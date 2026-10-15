//! The lab's screens: a HUD on the game layer (an icon and a live frame
//! counter), a window with a nine-slice frame, a wrapped body and two
//! buttons, and a popup on the popup layer overlapping the window, so its
//! text paints over the window's panel and opens the second text batch
//! (w01 §6's partition rule). The popup is modal (`D-149`): its buttons or
//! Escape hide it, and the window's "Drop" shows it again. World labels
//! (M54, `D-161`) draw under the UI: a fixed set for the `labels` fixture and
//! floating damage numbers in the interactive run.

use glam::Vec2;
use tungsten::core::ui::{
    Align, Anchor, CursorIcon, Justify, LayoutStyle, Sizing, StyleOverrides, TextOverrides,
    UiEvent, UiLayer, UiTree, Visibility, WidgetId,
};
use tungsten::core::{AssetRegistry, EventQueue, Time, World, WorldLabel, WorldLabels};

/// The `ui_themes` ID the lab applies.
pub const THEME_ID: &str = "ex05_default";
/// The HUD icon's sprite ID.
pub const ICON_ID: &str = "ex05_icon";
/// The window frame's sprite ID, a nine-slice.
pub const FRAME_ID: &str = "ex05_frame";

/// The widgets the lab's systems write to; a `World` resource.
#[derive(Debug)]
pub struct UiLab {
    /// The HUD root.
    pub hud: WidgetId,
    /// The frame counter label in the HUD.
    pub counter: WidgetId,
    /// The window root.
    pub window: WidgetId,
    /// The popup root, over the window.
    pub popup: WidgetId,
    /// The window's "Drop" button, which shows the popup.
    pub drop: WidgetId,
    /// The popup's "Cancel" button.
    pub cancel: WidgetId,
    /// The popup's "Drop it" button, focused when the popup shows.
    pub drop_it: WidgetId,
    /// Frames the counter has shown.
    pub frames: u64,
}

fn text(overrides: TextOverrides) -> StyleOverrides {
    StyleOverrides {
        text: overrides,
        ..StyleOverrides::default()
    }
}

/// Builds the three roots into `tree`. Sprite IDs are interned through
/// `sprites`, so a headless test without the manifest builds the same tree
/// (its images draw nothing and warn once).
pub fn build(tree: &mut UiTree, sprites: &mut AssetRegistry) -> UiLab {
    let icon = sprites.intern_sprite(ICON_ID);
    let frame = sprites.intern_sprite(FRAME_ID);

    // The HUD: pointer-transparent, an icon beside a counter in the mono
    // family, whose digits share one advance.
    let mut hud_style = LayoutStyle::row()
        .with_padding(8.0)
        .with_gap(8.0)
        .anchored(Anchor::TopStart, Vec2::new(16.0, 16.0));
    hud_style.align_items = Align::Center;
    let hud = tree.add_root(UiLayer::Game, hud_style);
    tree.set_hit_testable(hud, false);
    let hud_icon = tree.image(
        hud,
        icon,
        Vec2::new(32.0, 32.0),
        LayoutStyle::default().with_size(Sizing::Px(32.0), Sizing::Px(32.0)),
    );
    tree.set_hit_testable(hud_icon, false);
    let counter = tree.label(hud, "frame 0");
    tree.set_hit_testable(counter, false);
    tree.set_overrides(
        counter,
        text(TextOverrides {
            family: Some("mono".to_owned()),
            ..TextOverrides::default()
        }),
    );

    // The window: the nine-slice frame fills the panel under a title, a
    // wrapped body and a row of buttons.
    let window = tree.add_root(
        UiLayer::Game,
        LayoutStyle::column()
            .with_padding(20.0)
            .with_gap(12.0)
            .with_size(Sizing::Px(400.0), Sizing::Auto)
            .anchored(Anchor::Center, Vec2::new(-60.0, -20.0)),
    );
    tree.image(
        window,
        frame,
        Vec2::new(48.0, 48.0),
        LayoutStyle::default()
            .with_size(Sizing::Fill, Sizing::Fill)
            .anchored(Anchor::TopStart, Vec2::ZERO),
    );
    let title = tree.label(window, "Inventory");
    tree.set_overrides(
        title,
        text(TextOverrides {
            size: Some(24.0),
            line_height: Some(30.0),
            weight: Some(700),
            ..TextOverrides::default()
        }),
    );
    tree.label(
        window,
        "The lab paints panels, images and text through one ordered list. \
         Resize the window, press F5 for the layout bounds and F6 to hide the \
         game layers; edit the theme file to restyle it while it runs.",
    );
    let buttons = tree.panel(window, LayoutStyle::row().with_gap(12.0));
    tree.set_overrides(
        buttons,
        StyleOverrides {
            background: Some([0, 0, 0, 0]),
            border_width: Some(0.0),
            ..StyleOverrides::default()
        },
    );
    let equip = tree.button(buttons, "Equip");
    let drop = tree.button(buttons, "Drop");

    // The popup, on the popup layer over the window's body text and beside
    // its buttons. The roots fit a 1280×720 window at an OS scale up to 1.75,
    // where the logical viewport is about 730×410 (`D-127`).
    let popup = tree.add_root(
        UiLayer::GamePopup,
        LayoutStyle::column()
            .with_padding(16.0)
            .with_gap(8.0)
            .with_size(Sizing::Px(300.0), Sizing::Auto)
            .anchored(Anchor::Center, Vec2::new(140.0, 70.0)),
    );
    tree.set_modal(popup, true);
    let heading = tree.label(popup, "Confirm");
    tree.set_overrides(
        heading,
        text(TextOverrides {
            size: Some(20.0),
            line_height: Some(26.0),
            weight: Some(700),
            ..TextOverrides::default()
        }),
    );
    tree.label(popup, "Drop the lantern? It cannot be picked up again.");
    let mut row_style = LayoutStyle::row().with_gap(8.0);
    row_style.justify_content = Justify::End;
    let row = tree.panel(popup, row_style);
    tree.set_overrides(
        row,
        StyleOverrides {
            background: Some([0, 0, 0, 0]),
            border_width: Some(0.0),
            ..StyleOverrides::default()
        },
    );
    let cancel = tree.button(row, "Cancel");
    let drop_it = tree.button(row, "Drop it");
    tree.focus(drop_it);
    for button in [equip, drop, cancel, drop_it] {
        tree.set_cursor(button, Some(CursorIcon::Pointer));
    }

    UiLab {
        hud,
        counter,
        window,
        popup,
        drop,
        cancel,
        drop_it,
        frames: 0,
    }
}

/// Reacts to this frame's routing (`D-149`): the popup's "Cancel" or "Drop
/// it", or a `Cancel` while it shows, hides it, and the window's "Drop"
/// shows it again with "Drop it" focused.
pub fn popup_system(world: &mut World) {
    let (Some(lab), Some(events)) = (
        world.get_resource::<UiLab>(),
        world.get_resource::<EventQueue<UiEvent>>(),
    ) else {
        return;
    };
    let (popup, drop_it) = (lab.popup, lab.drop_it);
    let hide = events.activated(lab.cancel)
        || events.activated(drop_it)
        || events
            .iter_current()
            .any(|event| matches!(event, UiEvent::Cancel { scope, .. } if *scope == popup));
    let show = events.activated(lab.drop);
    let Some(tree) = world.get_resource_mut::<UiTree>() else {
        return;
    };
    if hide {
        tree.set_visibility(popup, Visibility::Hidden);
    } else if show {
        tree.set_visibility(popup, Visibility::Visible);
        tree.focus(drop_it);
    }
}

const RED: [u8; 4] = [255, 90, 90, 255];
const GREEN: [u8; 4] = [110, 230, 120, 255];
const BLUE: [u8; 4] = [150, 190, 255, 255];
const GOLD: [u8; 4] = [255, 200, 40, 255];
const WHITE: [u8; 4] = [255, 244, 236, 255];

/// The `labels` fixture's labels, pushed every frame: five on screen at
/// scales 1.0, 1.25 and 1.5 in three colours, a crit in `sans_bold`, one
/// straddling the window's left edge, which the panel partly covers, and one
/// off the view, which the extract culls. The camera stays at the origin, so
/// world units are physical pixels; the places suit the reference capture's
/// 1280×720 at an OS scale of 1.75, where the window's left edge is at 185.
pub fn fixture_labels_system(world: &mut World) {
    let Some(labels) = world.get_resource_mut::<WorldLabels>() else {
        return;
    };
    let on_screen = [
        ("12", Vec2::new(110.0, 600.0), 1.0, RED),
        ("250", Vec2::new(300.0, 680.0), 1.25, GREEN),
        ("Lantern", Vec2::new(1080.0, 160.0), 1.0, BLUE),
        ("7", Vec2::new(1040.0, 300.0), 1.5, RED),
        ("+30", Vec2::new(800.0, 690.0), 1.25, GREEN),
    ];
    for (text, anchor, scale, color) in on_screen {
        labels.push(
            WorldLabel::new(text, anchor, "sans", 20.0)
                .with_scale(scale)
                .with_color(color),
        );
    }
    labels.push(
        WorldLabel::new("99!", Vec2::new(500.0, 690.0), "sans_bold", 26.0)
            .with_scale(1.5)
            .with_color(GOLD),
    );
    labels
        .push(WorldLabel::new("under 5", Vec2::new(185.0, 330.0), "sans", 18.0).with_color(WHITE));
    labels.push(WorldLabel::new(
        "culled",
        Vec2::new(-400.0, 300.0),
        "sans",
        18.0,
    ));
}

/// Seconds between two damage numbers.
const SPAWN_INTERVAL: f32 = 0.25;
/// A damage number's life in seconds.
const LIFE: f32 = 0.8;
/// How far a number floats up over its life, in UI units.
const RISE: f32 = 40.0;
/// The pop: the scale falls from `POP_SCALE` to 1 over `POP_TIME` seconds.
const POP_SCALE: f32 = 1.6;
const POP_TIME: f32 = 0.15;
/// The world point the numbers spawn near.
const SPAWN_POINT: Vec2 = Vec2::new(1040.0, 560.0);

#[derive(Debug)]
struct DamageNumber {
    value: u32,
    crit: bool,
    anchor: Vec2,
    age: f32,
}

/// The interactive run's floating damage numbers, a `World` resource the
/// lab owns.
#[derive(Debug, Default)]
pub struct DamageNumbers {
    numbers: Vec<DamageNumber>,
    until_next: f32,
    spawned: u32,
}

/// Spawns a number every 0.25 s near a fixed point, every fourth a gold
/// crit in `sans_bold`; each floats 40 UI units up over 0.8 s, pops from
/// scale 1.6 to 1 over its first 0.15 s and fades out, ticked on
/// `Time::delta`.
pub fn damage_numbers_system(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    let Some(mut damage) = world.remove_resource::<DamageNumbers>() else {
        return;
    };
    damage.until_next -= dt;
    while damage.until_next <= 0.0 {
        damage.until_next += SPAWN_INTERVAL;
        let n = damage.spawned;
        damage.spawned += 1;
        let jitter = [-24.0, 8.0, -8.0, 24.0][(n % 4) as usize];
        damage.numbers.push(DamageNumber {
            value: 7 + (n * 37) % 90,
            crit: n % 4 == 3,
            anchor: SPAWN_POINT + Vec2::new(jitter, 0.0),
            age: 0.0,
        });
    }
    for number in &mut damage.numbers {
        number.age += dt;
    }
    damage.numbers.retain(|number| number.age < LIFE);
    if let Some(labels) = world.get_resource_mut::<WorldLabels>() {
        for number in &damage.numbers {
            let t = number.age / LIFE;
            let pop = (number.age / POP_TIME).min(1.0);
            let scale = POP_SCALE + (1.0 - POP_SCALE) * pop;
            let fade = ((LIFE - number.age) / (LIFE * 0.5)).clamp(0.0, 1.0);
            let (font, size, mut color) = if number.crit {
                ("sans_bold", 26.0, GOLD)
            } else {
                ("sans", 20.0, WHITE)
            };
            color[3] = (255.0 * fade) as u8;
            labels.push(
                WorldLabel::new(number.value.to_string(), number.anchor, font, size)
                    .with_offset(Vec2::new(0.0, -RISE * t))
                    .with_scale(scale)
                    .with_color(color),
            );
        }
    }
    world.insert_resource(damage);
}
