//! The lab's screens: a HUD on the game layer (an icon and a live frame
//! counter), a window with a nine-slice frame, a wrapped body and two
//! buttons, and a popup on the popup layer overlapping the window, so its
//! text paints over the window's panel and opens the second text batch
//! (w01 §6's partition rule). The popup is modal (`D-149`): its buttons or
//! Escape hide it, and the window's "Drop" shows it again.

use glam::Vec2;
use tungsten::core::ui::{
    Align, Anchor, CursorIcon, Justify, LayoutStyle, Sizing, StyleOverrides, TextOverrides,
    UiEvent, UiLayer, UiTree, Visibility, WidgetId,
};
use tungsten::core::{AssetRegistry, EventQueue, World};

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
