//! The sample's HUD: a root gameplay owns, drawn and frozen under a pause
//! (`RootCover::Show`), with a health bar, a meter in the theme's danger
//! colour at a third of max health or less, and its "current / max", the
//! gems counted, both in the mono family, and a prompt row that names the
//! bound keys for the last device.

use tungsten::core::World;
use tungsten::core::ui::widgets::{self, Meter};
use tungsten::core::ui::{Anchor, LayoutStyle, UiLayer, UiTree, WidgetId};
use tungsten::kit::{Counters, Health};

use super::{GEM_STARTS, SampleRun};

/// The HUD's widgets: a `World` resource while gameplay runs.
#[derive(Debug, Clone, Copy)]
pub struct Hud {
    pub root: WidgetId,
    /// The health bar, its fill as wide as the health left.
    pub bar: Meter,
    /// "current / max".
    pub health: WidgetId,
    /// "Gems n / 3".
    pub gems: WidgetId,
    /// The bound pause and shoot keys.
    pub prompt: WidgetId,
}

/// The health bar's size, in UI units.
const BAR: [f32; 2] = [120.0, 10.0];

/// Builds the HUD at the top left, pointer-transparent.
pub fn build(tree: &mut UiTree) -> Hud {
    let spacing = tree.theme().tokens.spacing;
    let style = LayoutStyle::column()
        .with_gap(spacing.sm)
        .with_padding(spacing.sm)
        .anchored(Anchor::TopStart, [spacing.lg; 2].into());
    let root = tree.add_root(UiLayer::Game, style);
    tree.set_hit_testable(root, false);
    let row = widgets::row(tree, root);
    let bar = widgets::meter(tree, row, BAR[0], BAR[1]);
    let health = tree.label(row, "");
    let gems = tree.label(root, "");
    widgets::mono(tree, health);
    widgets::mono(tree, gems);
    let prompt = widgets::prompt_row(
        tree,
        root,
        "{action:state_pause} Pause   {action:shoot} Shoot",
    );
    Hud {
        root,
        bar,
        health,
        gems,
        prompt,
    }
}

/// `sample_frame`: writes the run's health and gems into the HUD.
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
    let colors = tree.theme().tokens.colors;
    let color = [colors.accent, colors.danger][usize::from(current * 3.0 <= max)];
    hud.bar.set(tree, current / max, color);
}
