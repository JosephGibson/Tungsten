//! Toggles and sliders (`D-162`): a "Controls" window on the play screen
//! whose controls switch the damage numbers, set their rate and show a
//! volume readout. A system reads each control's new value from
//! `UiEvent::ValueChanged` through `EventQueue::value_changed`, once per
//! change, whatever device made it; the controls take input once nothing
//! modal covers them.

use tungsten::core::text::TextAlign;
use tungsten::core::ui::{
    Align, Anchor, LayoutStyle, Sizing, SliderRange, TextOverrides, UiEvent, UiTree, UiValue,
    WidgetId,
};
use tungsten::core::{EventQueue, World};

use crate::ui::labels::DamageNumbers;
use crate::ui::theme;
use crate::ui::widgets;

/// The controls window's widgets, a `World` resource the controls system
/// reads.
#[derive(Debug)]
pub struct UiControls {
    /// The window root.
    pub window: WidgetId,
    /// "Damage numbers": spawns them while on.
    pub damage: WidgetId,
    /// "Spawn rate": damage numbers a second, 1 to 8.
    pub spawn_rate: WidgetId,
    /// "Volume": 0 to 100 by 5, shown in the readout.
    pub volume: WidgetId,
    /// The volume's readout label, "80%" at the start.
    pub volume_readout: WidgetId,
    /// "Locked": a disabled toggle that shows the disabled look.
    pub locked: WidgetId,
}

/// Builds the controls window into `tree`: a window at the top end corner
/// on the game layer, as wide as its setting rows, titled "Controls", with
/// a "Damage numbers" toggle (on), a "Spawn rate" slider (1 to 8 a second
/// by 1, at 4), a "Volume" slider (0 to 100 by 5, at 80) whose readout sits
/// in its caption column, and a disabled "Locked" toggle, each row on the
/// shared caption and control columns.
pub fn build_controls(tree: &mut UiTree) -> UiControls {
    let window = widgets::window(tree, Anchor::TopEnd, widgets::setting_width(0.0));
    widgets::heading(tree, window, "Controls");
    let (_, damage) = widgets::toggle_row(tree, window, "Damage numbers", true);
    let (_, spawn_rate) = widgets::slider_row(
        tree,
        window,
        "Spawn rate",
        SliderRange::new(1.0, 8.0).with_step(1.0),
        4.0,
    );
    // The volume's caption cell holds its readout at the end, so the
    // slider keeps the control column.
    let mut style = LayoutStyle::row().with_gap(theme::ROW_GAP);
    style.align_items = Align::Center;
    let row = tree.panel(window, style);
    tree.set_overrides(row, widgets::transparent());
    let mut cell_style =
        LayoutStyle::row().with_size(Sizing::Px(theme::CAPTION_WIDTH), Sizing::Auto);
    cell_style.align_items = Align::Center;
    let cell = tree.panel(row, cell_style);
    tree.set_overrides(cell, widgets::transparent());
    let caption = tree.label(cell, "Volume");
    tree.set_layout_style(
        caption,
        LayoutStyle::default().with_size(Sizing::Fill, Sizing::Auto),
    );
    let volume_readout = tree.label(cell, "80%");
    tree.set_overrides(
        volume_readout,
        widgets::text(widgets::value_text(TextOverrides {
            align: Some(TextAlign::End),
            ..TextOverrides::default()
        })),
    );
    let volume = tree.slider(row, SliderRange::new(0.0, 100.0).with_step(5.0), 80.0);
    tree.set_layout_style(
        volume,
        LayoutStyle::default().with_size(Sizing::Px(theme::CONTROL_WIDTH), Sizing::Auto),
    );
    widgets::name_control(tree, volume, "Volume");

    let (_, locked) = widgets::toggle_row(tree, window, "Locked", false);
    tree.set_enabled(locked, false);

    UiControls {
        window,
        damage,
        spawn_rate,
        volume,
        volume_readout,
        locked,
    }
}

/// Applies this frame's control changes (`UiEvent::ValueChanged`): the
/// damage numbers' switch and rate, and the volume's readout.
pub fn controls_system(world: &mut World) {
    let (Some(controls), Some(events)) = (
        world.get_resource::<UiControls>(),
        world.get_resource::<EventQueue<UiEvent>>(),
    ) else {
        return;
    };
    let damage_on = events
        .value_changed(controls.damage)
        .and_then(UiValue::as_bool);
    let rate = events
        .value_changed(controls.spawn_rate)
        .and_then(UiValue::as_number);
    let volume = events
        .value_changed(controls.volume)
        .and_then(UiValue::as_number);
    let readout = controls.volume_readout;
    if let Some(volume) = volume
        && let Some(tree) = world.get_resource_mut::<UiTree>()
    {
        tree.set_text(readout, format!("{volume:.0}%"));
    }
    if let Some(damage) = world.get_resource_mut::<DamageNumbers>() {
        if let Some(on) = damage_on {
            damage.set_enabled(on);
        }
        if let Some(rate) = rate {
            damage.set_rate(rate);
        }
    }
}
