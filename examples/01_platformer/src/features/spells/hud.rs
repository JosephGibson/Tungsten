//! The HUD, in the shell's theme, built as the template's sample builds its
//! bar: at the top left the health bar over the mana bar, each with its
//! "current / max" in mono, the health in the accent (the danger colour at
//! a third or less) and the mana in indigo; at the bottom centre the spell
//! bar, each slot its icon inside its element's border, its key, its cost,
//! its cooldown's sweep and its charges, the selected slot outlined in the
//! accent and one the player cannot pay for greyed, slot 0 the summon's
//! selected type's icon, and beside it the gun's magazine, its icon, its rounds and a pip a round,
//! greyed and "reloading" at none; above the lane where the shell's toasts
//! rise. Two roots gameplay owns, drawn and frozen under a pause, which the
//! pointer passes through.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use glam::Vec2;
use tungsten::RootCover;
use tungsten::core::text::TextWrap;
use tungsten::core::ui::{
    Align, Anchor, LayoutStyle, Placement, Sizing, StyleOverrides, UiLayer, UiTree, UiVertex,
    WidgetId,
};
use tungsten::core::{AssetRegistry, DataRegistry, Entity, ScaleFactor, World};
use tungsten::kit::{Abilities, Ability, Health, Stats};

use super::components::{School, Spell, Spellbook};
use super::{SLOT_KEYS, SLOTS, STEPS_PER_SECOND, mana, summon_type};
use crate::features::gun;
use crate::features::run::Run;
use crate::shell::{self, theme, widgets};

/// A bar and its "current / max".
#[derive(Debug, Clone, Copy)]
pub struct Meter {
    /// The bar's fill, as wide as the share left.
    pub fill: WidgetId,
    pub text: WidgetId,
}

/// One slot of the spell bar.
#[derive(Debug, Clone, Copy)]
pub struct Slot {
    /// The accent's outline round the selected slot.
    pub ring: WidgetId,
    /// The icon and the sweep over it; none in an empty slot.
    pub icon: Option<WidgetId>,
    pub sweep: Option<WidgetId>,
    /// One pip a charge, for a spell with charges.
    pub pips: Option<[WidgetId; PIPS]>,
}

/// The gun's magazine beside the bar: its icon, its rounds and its pips.
#[derive(Debug, Clone, Copy)]
pub struct Magazine {
    pub icon: WidgetId,
    /// "left / most", or "reloading" at none.
    pub text: WidgetId,
    pub pips: [WidgetId; ROUNDS],
}

/// The HUD's widgets: a `World` resource while gameplay runs.
#[derive(Debug, Clone, Copy)]
pub struct Hud {
    /// The top left's root, with the health and mana bars.
    pub meters: WidgetId,
    /// The bottom centre's root, with the spell bar.
    pub bar: WidgetId,
    pub health: Meter,
    pub mana: Meter,
    pub slots: [Slot; 10],
    pub magazine: Magazine,
    /// The icons' size in UI units: a whole number of screen pixels for
    /// each of their own.
    pub icon_size: f32,
}

/// The bars' size, and the room their "current / max" takes, so the
/// numbers never shift, in UI units; the readouts never wrap.
const METER: [f32; 2] = [160.0, 10.0];
const READOUT: f32 = 96.0;
/// The room under the spell bar for one toast: its padding and its line.
const TOAST_LANE: f32 = 2.0 * theme::GAP + 20.0;
/// The icons' art, in its pixels.
const ICON: f32 = 32.0;
/// A border's width.
const BORDER: f32 = 2.0;
/// The most charges a slot shows, and a pip's side.
pub const PIPS: usize = 3;
const PIP: f32 = 4.0;
/// The most rounds the magazine shows, the gun's icon, and the room its
/// readout takes, so "reloading" never shifts the bar.
pub const ROUNDS: usize = 12;
const GUN_ICON: &str = "ex10_icon_gun";
const MAGAZINE_READOUT: f32 = 72.0;
/// The veil over the part of a cooldown still to run.
const VEIL: [u8; 4] = [11, 12, 16, 170];
/// What an icon the player cannot pay for is multiplied by.
const GREYED: [u8; 4] = [120, 120, 128, 150];
const WHITE: [u8; 4] = [255; 4];
const CLEAR: [u8; 4] = [0; 4];

/// Builds the HUD for the run gameplay starts: its roots owned by gameplay,
/// shown and frozen under a pause.
pub fn start(world: &mut World) {
    let icons = SLOTS.map(|slot| {
        slot.map(|spell| {
            let icon = world
                .resource_mut::<AssetRegistry>()
                .intern_sprite(spell.icon());
            (spell, icon, cost_label(world, spell))
        })
    });
    let gun_icon = world
        .resource_mut::<AssetRegistry>()
        .intern_sprite(GUN_ICON);
    let size = icon_size(world);
    let tree = world.resource_mut::<UiTree>();
    let hud = build(tree, icons, gun_icon, size);
    theme::slide_in(tree, hud.meters);
    theme::slide_in(tree, hud.bar);
    let stack = shell::stack(world);
    stack.own_root(hud.meters, RootCover::Show);
    stack.own_root(hud.bar, RootCover::Show);
    world.insert_resource(hud);
    update(world);
}

/// The size an icon shows at: the largest whole number of screen pixels
/// for each of its own within the UI's scale, at least one.
fn icon_size(world: &World) -> f32 {
    let scale = world
        .get_resource::<ScaleFactor>()
        .map_or(1.0, ScaleFactor::get);
    ICON * scale.floor().max(1.0) / scale
}

/// The cost the bar shows under `spell`: a press's mana, or a held spell's
/// mana a second.
fn cost_label(world: &World, spell: Spell) -> String {
    let cost = world
        .get_resource::<DataRegistry>()
        .and_then(|data| data.get::<Ability>(spell.ability()))
        .and_then(Ability::cost)
        .map_or(0.0, |(_, amount)| amount);
    if spell.held() {
        format!("{}/s", (cost * STEPS_PER_SECOND).round())
    } else {
        format!("{cost}")
    }
}

type SlotArt = Option<(Spell, tungsten::core::SpriteAssetId, String)>;

fn build(
    tree: &mut UiTree,
    icons: [SlotArt; 10],
    gun_icon: tungsten::core::SpriteAssetId,
    size: f32,
) -> Hud {
    let style = LayoutStyle::column()
        .with_gap(theme::GAP)
        .with_padding(theme::GAP)
        .anchored(Anchor::TopStart, [theme::MARGIN; 2].into());
    let meters = tree.add_root(UiLayer::Game, style);
    tree.set_hit_testable(meters, false);
    let accent = tree.theme().tokens.colors.accent;
    let health = meter(tree, meters, accent);
    let mana = meter(tree, meters, School::Mana.color());
    let style = LayoutStyle::row().with_gap(theme::GAP).anchored(
        Anchor::BottomCenter,
        Vec2::new(0.0, theme::MARGIN + TOAST_LANE + theme::GAP),
    );
    let bar = tree.add_root(UiLayer::Game, style);
    tree.set_hit_testable(bar, false);
    widgets::fill(tree, bar, CLEAR);
    let mut slots = Vec::with_capacity(SLOTS.len());
    for (key, art) in SLOT_KEYS.iter().zip(icons) {
        slots.push(slot(tree, bar, key, art, size));
    }
    let magazine = magazine(tree, bar, gun_icon, size);
    Hud {
        meters,
        bar,
        health,
        mana,
        slots: slots.try_into().expect("ten slots"),
        magazine,
        icon_size: size,
    }
}

/// The magazine: a slot's ring and frame round the gun's icon, then its
/// readout and its row of pips.
fn magazine(
    tree: &mut UiTree,
    bar: WidgetId,
    icon: tungsten::core::SpriteAssetId,
    size: f32,
) -> Magazine {
    let mut style = LayoutStyle::column();
    style.align_items = Align::Center;
    let column = tree.panel(bar, style);
    widgets::fill(tree, column, CLEAR);
    let ring = tree.panel(column, LayoutStyle::row().with_padding(BORDER / 2.0));
    edged(tree, ring, CLEAR, CLEAR);
    let frame = tree.panel(ring, LayoutStyle::column().with_padding(BORDER));
    let colors = tree.theme().tokens.colors;
    edged(tree, frame, School::Physical.color(), colors.surface);
    let square = LayoutStyle::default().with_size(Sizing::Px(size), Sizing::Px(size));
    let image = tree.image(frame, icon, Vec2::splat(ICON), square);
    let text = widgets::small(tree, column, "");
    widgets::mono(tree, text);
    let mut style = tree.layout_style(text).cloned().unwrap_or_default();
    style.min_width = Some(MAGAZINE_READOUT);
    tree.set_layout_style(text, style);
    let row = tree.panel(column, LayoutStyle::row().with_gap(PIP / 2.0));
    widgets::fill(tree, row, CLEAR);
    let pips = [(); ROUNDS].map(|()| {
        let pip = tree.panel(
            row,
            LayoutStyle::default().with_size(Sizing::Px(PIP), Sizing::Px(PIP)),
        );
        widgets::fill(tree, pip, colors.text);
        pip
    });
    Magazine {
        icon: image,
        text,
        pips,
    }
}

/// A bar `color` fills, and its text.
fn meter(tree: &mut UiTree, parent: WidgetId, color: [u8; 4]) -> Meter {
    let row = widgets::row(tree, parent);
    let track = tree.panel(
        row,
        LayoutStyle::row().with_size(Sizing::Px(METER[0]), Sizing::Px(METER[1])),
    );
    let fill = tree.panel(
        track,
        LayoutStyle::default().with_size(Sizing::Percent(100.0), Sizing::Fill),
    );
    widgets::fill(tree, fill, color);
    let text = tree.label(row, "");
    let mut style = tree.layout_style(text).cloned().unwrap_or_default();
    style.min_width = Some(READOUT);
    tree.set_layout_style(text, style);
    widgets::mono(tree, text);
    widgets::restyle(tree, text, |text| text.wrap = Some(TextWrap::None));
    Meter { fill, text }
}

/// Edges of `color`, `width` wide, round a clear or `background` box.
fn edged(tree: &mut UiTree, id: WidgetId, color: [u8; 4], background: [u8; 4]) {
    let style = StyleOverrides {
        background: Some(background),
        border: Some(color),
        border_width: Some(BORDER),
        ..StyleOverrides::default()
    };
    tree.set_overrides(id, style);
}

/// A slot: the ring, inside it the frame in the element's border with the
/// icon, its sweep and its pips, and under it the key and the cost.
fn slot(tree: &mut UiTree, bar: WidgetId, key: &str, art: SlotArt, size: f32) -> Slot {
    // Centred, so the ring keeps to its frame over a wider pair of labels.
    let mut style = LayoutStyle::column();
    style.align_items = Align::Center;
    let column = tree.panel(bar, style);
    widgets::fill(tree, column, CLEAR);
    let ring = tree.panel(column, LayoutStyle::row().with_padding(BORDER / 2.0));
    edged(tree, ring, CLEAR, CLEAR);
    let frame = tree.panel(ring, LayoutStyle::column().with_padding(BORDER));
    let colors = tree.theme().tokens.colors;
    let border = art
        .as_ref()
        .map_or(colors.border, |(spell, ..)| spell.school().color());
    edged(tree, frame, border, colors.surface);
    let square = || LayoutStyle::default().with_size(Sizing::Px(size), Sizing::Px(size));
    let labels = widgets::row(tree, column);
    widgets::small(tree, labels, key);
    let Some((spell, icon, cost)) = art else {
        // An empty slot keeps its place, its box as large as a full one's.
        tree.panel(frame, square());
        return Slot {
            ring,
            icon: None,
            sweep: None,
            pips: None,
        };
    };
    let image = tree.image(frame, icon, Vec2::splat(ICON), square());
    let mut over = square();
    over.placement = Placement::Anchored {
        anchor: Anchor::Center,
        offset: Vec2::ZERO,
    };
    // A mesh's own box takes the panel's style: clear, so the icon shows.
    let sweep = tree.mesh(frame, over);
    widgets::fill(tree, sweep, CLEAR);
    let price = widgets::small(tree, labels, &cost);
    widgets::restyle(tree, price, |text| text.color = Some(School::Mana.color()));
    let pips = (spell == Spell::IronBlock).then(|| {
        let mut style = LayoutStyle::row().with_gap(PIP / 2.0);
        style.placement = Placement::Anchored {
            anchor: Anchor::BottomCenter,
            offset: Vec2::new(0.0, BORDER),
        };
        let row = tree.panel(frame, style);
        widgets::fill(tree, row, CLEAR);
        [(); PIPS].map(|()| {
            let pip = tree.panel(
                row,
                LayoutStyle::default().with_size(Sizing::Px(PIP), Sizing::Px(PIP)),
            );
            widgets::fill(tree, pip, colors.text);
            pip
        })
    });
    Slot {
        ring,
        icon: Some(image),
        sweep: Some(sweep),
        pips,
    }
}

/// What a slot shows of its spell: whether the player can pay for a use,
/// the share of its cooldown still to run, and its charges left of its
/// most.
struct Shown {
    payable: bool,
    cooling: f32,
    charges: Option<(u32, u32)>,
}

fn shown(world: &World, player: Entity, spell: Spell, mana: f32) -> Option<Shown> {
    let ability = world
        .get_resource::<DataRegistry>()?
        .get::<Ability>(spell.ability())?;
    let abilities = world.get::<Abilities>(player)?;
    let name = spell.ability();
    let cooling = if ability.cooldown() > 0.0 {
        abilities.cooldown_left(name).unwrap_or(0.0) / ability.cooldown()
    } else {
        0.0
    };
    let charges = ability
        .charges()
        .map(|(most, _, _)| (abilities.charges(name).unwrap_or(most), most));
    let payable = ability.cost().is_none_or(|(_, cost)| mana >= cost)
        && charges.is_none_or(|(left, _)| left > 0);
    Some(Shown {
        payable,
        cooling,
        charges,
    })
}

/// `spells_hud`, in `Update`: writes the run's health, mana and spells
/// into the HUD.
pub fn update(world: &mut World) {
    let (Some(mut hud), Some(player)) = (
        world.get_resource::<Hud>().copied(),
        world.get_resource::<Run>().map(|run| run.player),
    ) else {
        return;
    };
    let health = world
        .get::<Health>(player)
        .map_or((0.0, 1.0), |health| (health.current(), health.max()));
    let shown_health = whole_health(world.get::<Health>(player));
    let held = mana(world, player);
    let most = world
        .get::<Stats>(player)
        .and_then(|stats| stats.get("max_mana"))
        .unwrap_or(1.0);
    let selected = world.get::<Spellbook>(player).map(|book| book.slot);
    let states = SLOTS.map(|slot| slot.and_then(|spell| shown(world, player, spell, held)));
    let rounds = gun::rounds(world, player);
    let summoned = summon_type(world).icon;
    let summoned = world
        .resource_mut::<AssetRegistry>()
        .intern_sprite(summoned);
    let size = icon_size(world);
    let tree = world.resource_mut::<UiTree>();
    let colors = tree.theme().tokens.colors;
    let low = health.0 * 3.0 <= health.1;
    fill(
        tree,
        hud.health,
        health,
        shown_health,
        [colors.accent, colors.danger][usize::from(low)],
    );
    let held = held.floor();
    fill(tree, hud.mana, (held, most), held, School::Mana.color());
    let resized = size != hud.icon_size;
    for (index, (slot, state)) in hud.slots.iter().zip(states).enumerate() {
        let ring = if selected == Some(index) {
            colors.accent
        } else {
            CLEAR
        };
        if tree.overrides(slot.ring).and_then(|style| style.border) != Some(ring) {
            edged(tree, slot.ring, ring, CLEAR);
        }
        if let (Some(icon), Some(sweep)) = (slot.icon, slot.sweep) {
            if SLOTS[index] == Some(Spell::Summon) && tree.image_sprite(icon) != Some(summoned) {
                tree.set_image(icon, summoned, Vec2::splat(ICON));
            }
            if resized {
                for id in [icon, sweep] {
                    let style =
                        LayoutStyle::default().with_size(Sizing::Px(size), Sizing::Px(size));
                    let mut kept = tree.layout_style(id).cloned().unwrap_or_default();
                    (kept.width, kept.height) = (style.width, style.height);
                    tree.set_layout_style(id, kept);
                }
            }
            let state = state.unwrap_or(Shown {
                payable: false,
                cooling: 0.0,
                charges: None,
            });
            tree.set_image_tint(icon, if state.payable { WHITE } else { GREYED });
            tree.set_mesh(sweep, &veil(state.cooling));
            if let (Some(pips), Some((left, most))) = (slot.pips, state.charges) {
                for (i, pip) in pips.into_iter().enumerate() {
                    let color = if i < left as usize {
                        colors.text
                    } else {
                        colors.disabled_text
                    };
                    widgets::show(tree, pip, i < most as usize);
                    if tree.overrides(pip).and_then(|style| style.background) != Some(color) {
                        widgets::fill(tree, pip, color);
                    }
                }
            }
        }
    }
    if resized {
        let style = LayoutStyle::default().with_size(Sizing::Px(size), Sizing::Px(size));
        let mut kept = tree
            .layout_style(hud.magazine.icon)
            .cloned()
            .unwrap_or_default();
        (kept.width, kept.height) = (style.width, style.height);
        tree.set_layout_style(hud.magazine.icon, kept);
    }
    if let Some((left, most)) = rounds {
        let readout = if left == 0 {
            "reloading".to_string()
        } else {
            format!("{left} / {most}")
        };
        tree.set_plain_text(hud.magazine.text, &readout);
        let tint = if left == 0 { GREYED } else { WHITE };
        tree.set_image_tint(hud.magazine.icon, tint);
        for (i, pip) in hud.magazine.pips.into_iter().enumerate() {
            let color = if i < left as usize {
                colors.text
            } else {
                colors.disabled_text
            };
            widgets::show(tree, pip, i < most as usize);
            if tree.overrides(pip).and_then(|style| style.background) != Some(color) {
                widgets::fill(tree, pip, color);
            }
        }
    }
    if resized {
        hud.icon_size = size;
        world.insert_resource(hud);
    }
}

/// Health as the HUD and a save's summary read it: whole, rounded up while
/// alive, so a scratch never reads as dead, and 0 once dead.
#[must_use]
pub fn whole_health(health: Option<&Health>) -> f32 {
    health.map_or(0.0, |health| {
        if health.is_dead() {
            0.0
        } else {
            health.current().ceil()
        }
    })
}

/// Writes `shown / max` into `meter` and fills it to `current / max` in
/// `color`.
fn fill(tree: &mut UiTree, meter: Meter, (current, max): (f32, f32), shown: f32, color: [u8; 4]) {
    tree.set_plain_text(meter.text, &format!("{shown} / {max}"));
    if tree
        .overrides(meter.fill)
        .and_then(|style| style.background)
        != Some(color)
    {
        widgets::fill(tree, meter.fill, color);
    }
    let width = Sizing::Percent((current / max.max(f32::EPSILON) * 100.0).clamp(0.0, 100.0));
    if tree
        .layout_style(meter.fill)
        .is_some_and(|style| style.width != width)
    {
        tree.set_layout_style(
            meter.fill,
            LayoutStyle::default().with_size(width, Sizing::Fill),
        );
    }
}

/// The veil over the part of a cooldown still to run: the last `left` of a
/// turn clockwise from twelve o'clock, filling its box to the edges.
#[must_use]
pub fn veil(left: f32) -> Vec<UiVertex> {
    let left = left.clamp(0.0, 1.0);
    if left <= 0.0 {
        return Vec::new();
    }
    let end = 1.5 * PI;
    let start = end - left * TAU;
    let mut angles: Vec<f32> = (0..=24)
        .map(|i| start + (end - start) * i as f32 / 24.0)
        .collect();
    // The box's corners, so the veil fills them.
    angles.extend(
        (0..4)
            .map(|k| -0.25 * PI + k as f32 * FRAC_PI_2)
            .filter(|&angle| angle > start && angle < end),
    );
    angles.sort_by(f32::total_cmp);
    let edge = |angle: f32| {
        let along = Vec2::new(angle.cos(), angle.sin());
        Vec2::splat(0.5) + along * (0.5 / along.x.abs().max(along.y.abs()))
    };
    let centre = UiVertex::new(Vec2::splat(0.5), VEIL);
    angles
        .windows(2)
        .flat_map(|pair| {
            [
                centre,
                UiVertex::new(edge(pair[0]), VEIL),
                UiVertex::new(edge(pair[1]), VEIL),
            ]
        })
        .collect()
}
