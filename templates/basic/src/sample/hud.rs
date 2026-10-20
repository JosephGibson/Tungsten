//! The sample's HUD: a root gameplay owns, drawn and frozen under a pause
//! (`RootCover::Show`), with a health bar and its "current / max", the gems
//! counted and a prompt row that names the bound keys for the last device.

use tungsten::core::World;
use tungsten::core::ui::{Anchor, LayoutStyle, Sizing, UiLayer, UiTree, WidgetId};
use tungsten::kit::{Counters, Health};

use super::{GEM_STARTS, SampleRun};
use crate::shell::{theme, widgets};

/// The HUD's widgets: a `World` resource while gameplay runs.
#[derive(Debug, Clone, Copy)]
pub struct Hud {
    pub root: WidgetId,
    /// The health bar's fill, as wide as the health left.
    pub fill: WidgetId,
    /// "current / max".
    pub health: WidgetId,
    /// "Gems n / 3".
    pub gems: WidgetId,
    /// The bound pause and mute keys.
    pub prompt: WidgetId,
}

/// The health bar's size, in UI units.
const BAR: [f32; 2] = [120.0, 10.0];

/// Builds the HUD at the top left, pointer-transparent.
pub fn build(tree: &mut UiTree) -> Hud {
    let style = LayoutStyle::column()
        .with_gap(theme::GAP)
        .with_padding(theme::GAP)
        .anchored(Anchor::TopStart, [theme::MARGIN; 2].into());
    let root = tree.add_root(UiLayer::Game, style);
    tree.set_hit_testable(root, false);
    let row = widgets::row(tree, root);
    let track = tree.panel(
        row,
        LayoutStyle::row().with_size(Sizing::Px(BAR[0]), Sizing::Px(BAR[1])),
    );
    let fill = tree.panel(
        track,
        LayoutStyle::default().with_size(Sizing::Percent(100.0), Sizing::Fill),
    );
    let accent = tree.theme().tokens.colors.accent;
    widgets::fill(tree, fill, accent);
    let health = tree.label(row, "");
    let gems = tree.label(root, "");
    let prompt = widgets::prompt_row(
        tree,
        root,
        "{action:state_pause} Pause   {action:toggle_mute} Mute",
    );
    Hud {
        root,
        fill,
        health,
        gems,
        prompt,
    }
}

/// Writes the run's health and gems into the HUD: part of `sample_frame`.
pub fn update(world: &mut World) {
    let (Some(hud), Some(run)) = (
        world.get_resource::<Hud>().copied(),
        world.get_resource::<SampleRun>(),
    ) else {
        return;
    };
    let (current, max) = world
        .get::<Health>(run.player)
        .map_or((0.0, 1.0), |health| (health.current(), health.max()));
    let counted = world
        .get_resource::<Counters>()
        .map_or(0, |counters| counters.get("gems"));
    let health = format!("{current} / {max}");
    let gems = format!("Gems {counted} / {}", GEM_STARTS.len());
    let tree = world.resource_mut::<UiTree>();
    tree.set_plain_text(hud.health, &health);
    tree.set_plain_text(hud.gems, &gems);
    let width = Sizing::Percent((current / max * 100.0).clamp(0.0, 100.0));
    if tree
        .layout_style(hud.fill)
        .is_some_and(|style| style.width != width)
    {
        tree.set_layout_style(
            hud.fill,
            LayoutStyle::default().with_size(width, Sizing::Fill),
        );
    }
}
