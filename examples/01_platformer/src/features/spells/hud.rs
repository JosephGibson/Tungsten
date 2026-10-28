//! The HUD, in the shell's theme, built as the template's sample builds its
//! bar: at the top left the health bar over the mana bar, each a dark
//! track whose fill clips its own liquid, scrolled by the game clock, with
//! a bright edge where it stands, beside its "current / max" in mono; the
//! pact's shield a lighter liquid over health's from the left, as wide as
//! its share of the maximum, with its "+shield" after the readout; at a
//! third or less, health's liquid pulses and its track's border beats in
//! the danger colour; under them, from the first take, the tomes held as
//! tiles in the order taken. At the bottom centre the spell bar: each slot its
//! icon tile, its live key on a corner plate, its cooldown's sweep and its
//! charges, the selected slot ringed in the focus colour and one the player
//! cannot pay for greyed, a white flash as a long cooldown ends and a red
//! one on a press refused as not ready; the last the summon's selected type,
//! its name over it a moment after a turn. Then, set apart past a divider,
//! the gun: its tile, its rounds as cartridges and their count, and while a
//! reload runs "reloading", the tile greyed under the reload's sweep and
//! every cartridge spent. Above the lane where the shell's toasts rise. Two
//! roots gameplay owns, drawn and frozen under a pause, which the pointer
//! passes through; and a third, hidden under a pause, the panel over a
//! book in reach: its name, what it gives, what it costs and the key that
//! takes it.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use glam::Vec2;
use tungsten::RootCover;
use tungsten::core::text::{TextAlign, TextWrap};
use tungsten::core::ui::{
    Align, Anchor, Edges, Justify, LayoutStyle, Sizing, StyleOverrides, UiLayer, UiTree, UiVertex,
    Visibility, WidgetId,
};
use tungsten::core::{
    ActionMap, AssetRegistry, Binding, CameraState, DataRegistry, Entity, ScaleFactor,
    SpriteAssetId, Time, World,
};
use tungsten::kit::{Abilities, Ability, Health, Shield, Stats};

use super::components::{Spell, Spellbook};
use super::{SLOT_ACTIONS, SLOT_COUNT, SLOTS, mana, slot_of, summon_type};
use crate::features::gun::components::Reload;
use crate::features::gun::{self, MAGAZINE, RELOAD_SECS};
use crate::features::level::systems::drawn_time;
use crate::features::run::Run;
use crate::features::tomes::{self, BOOK_CENTRE, TomeKind};
use crate::shell::{self, theme, widgets};

/// A bar: its bordered track, the fill as wide as the share left, which
/// clips the liquid's strip, a low bar's pulse and the bright edge where
/// the liquid stands, and its "current / max".
#[derive(Debug, Clone, Copy)]
pub struct Meter {
    pub track: WidgetId,
    /// The dark well under the liquid, the bar's full width.
    pub well: WidgetId,
    pub fill: WidgetId,
    /// The liquid's tiles side by side, scrolled left.
    pub strip: WidgetId,
    pub pulse: WidgetId,
    pub edge: WidgetId,
    pub text: WidgetId,
}

/// One slot of the spell bar.
#[derive(Debug, Clone, Copy)]
pub struct Slot {
    /// The focus colour's outline round the selected slot.
    pub ring: WidgetId,
    /// The icon tile's box.
    pub tile: WidgetId,
    /// The icon, the sweep and the flash over it; none in an empty slot.
    pub icon: Option<WidgetId>,
    pub sweep: Option<WidgetId>,
    pub flash: Option<WidgetId>,
    /// Its action's key, on a plate in the tile's top-left corner.
    pub key: WidgetId,
    /// One pip a charge, for a spell with charges.
    pub pips: Option<[WidgetId; PIPS]>,
}

/// The gun, set apart from the spells: its tile with the reload's sweep,
/// its rounds as cartridges and their count under them.
#[derive(Debug, Clone, Copy)]
pub struct Magazine {
    pub tile: WidgetId,
    pub icon: WidgetId,
    pub sweep: WidgetId,
    /// The cartridges' row.
    pub row: WidgetId,
    /// "left / most", or "reloading" while a reload runs.
    pub text: WidgetId,
    /// A cartridge a round, live or spent.
    pub rounds: [WidgetId; ROUNDS],
}

/// The pact's shield on the health bar: a layer in its well from the left,
/// as wide as the shield's share of the maximum, which clips the shield's
/// liquid, with its own edge, and its "+shield" after the health's
/// readout; both collapsed while it holds none.
#[derive(Debug, Clone, Copy)]
pub struct ShieldBar {
    pub fill: WidgetId,
    pub strip: WidgetId,
    pub edge: WidgetId,
    pub text: WidgetId,
}

/// The tomes held, under the bars: a tile a tome, in the order taken, the
/// row collapsed until the first.
#[derive(Debug, Clone, Copy)]
pub struct Strip {
    pub row: WidgetId,
    pub tiles: [WidgetId; 3],
}

/// The panel over a book in reach: its root, the book's name, what it
/// gives and what it costs, over the key that takes it.
#[derive(Debug, Clone, Copy)]
pub struct Panel {
    pub root: WidgetId,
    pub name: WidgetId,
    pub gives: WidgetId,
    pub costs: WidgetId,
}

/// The HUD's sprites: the gun's icon, the bars' and the shield's liquids,
/// the cartridges, live and spent, and the tomes' tiles in
/// [`TomeKind::ALL`]'s order.
#[derive(Debug, Clone, Copy)]
pub struct Art {
    pub gun: SpriteAssetId,
    pub health: SpriteAssetId,
    pub mana: SpriteAssetId,
    pub shield: SpriteAssetId,
    pub live: SpriteAssetId,
    pub spent: SpriteAssetId,
    pub tomes: [SpriteAssetId; 3],
}

/// The HUD's widgets and what it shows over time: a `World` resource while
/// gameplay runs.
#[derive(Debug, Clone, Copy)]
pub struct Hud {
    /// The top left's root, with the health and mana bars.
    pub meters: WidgetId,
    /// The bottom centre's root, with the spell bar.
    pub bar: WidgetId,
    pub health: Meter,
    pub shield: ShieldBar,
    pub mana: Meter,
    pub strip: Strip,
    pub slots: [Slot; SLOT_COUNT],
    /// The summon's name over its slot, and its plate.
    pub name_plate: WidgetId,
    pub name: WidgetId,
    pub magazine: Magazine,
    pub panel: Panel,
    pub art: Art,
    /// The UI's scale the HUD was sized for: an art pixel is a whole
    /// number of screen pixels at it, and the bars sit on whole pixels.
    pub scale: f32,
    /// Game seconds each slot's ready flash and refused flash still show,
    /// and the summon's name.
    pub ready: [f32; SLOT_COUNT],
    pub refused: [f32; SLOT_COUNT],
    pub named: f32,
    /// Each slot's cooldown share as last shown, so its end is seen.
    pub cooling: [f32; SLOT_COUNT],
}

/// The bars' liquid, in its texels, and the frame and gap round it; the
/// room their "current / max" takes in mono, so the numbers never shift,
/// in UI units, and its size and line; the readouts never wrap.
const METER: [f32; 2] = [168.0, 10.0];
const TRACK_EDGE: f32 = 2.0;
const READOUT: f32 = 84.0;
const READOUT_TEXT: [f32; 2] = [14.0, 18.0];
/// The liquids' textures, in their texels, how many a strip lays side by
/// side, and how fast the game clock scrolls each, in texels a second;
/// the mana's starts on its own phase.
const LIQUID: [f32; 2] = [128.0, 10.0];
const LIQUID_TILES: usize = 3;
const HEALTH_LIQUID: &str = "ex10_hud_liquid_health";
const MANA_LIQUID: &str = "ex10_hud_liquid_mana";
const HEALTH_FLOW: f32 = 14.0;
const MANA_FLOW: f32 = 9.0;
const MANA_PHASE: f32 = 40.0;
/// The pact's shield: its liquid, its flow, slower than health's, its
/// edge, its readout's colour and the room "+50" takes in mono.
const SHIELD_LIQUID: &str = "ex10_hud_liquid_shield";
const SHIELD_FLOW: f32 = 6.0;
const SHIELD_EDGE: [u8; 4] = [200, 240, 255, 255];
const SHIELD_TEXT: [u8; 4] = [150, 215, 255, 255];
const SHIELD_READOUT: f32 = 36.0;
/// The bright edge where each liquid stands, and the well under it.
const HEALTH_EDGE: [u8; 4] = [255, 150, 140, 255];
const MANA_EDGE: [u8; 4] = [152, 192, 255, 255];
const WELL: [u8; 4] = [2, 2, 6, 255];
/// The alpha the theme's background takes behind the bars and their
/// track.
const METERS_ALPHA: u8 = 170;
const TRACK_ALPHA: u8 = 235;
/// A low bar's beat, a second's, and the white its liquid brightens by at
/// its peak.
const PULSE_HZ: f32 = 2.0;
const PULSE: f32 = 31.0;
/// The room under the spell bar for one toast: its padding and its line.
const TOAST_LANE: f32 = 2.0 * theme::GAP + 20.0;
/// The icon tiles' art, in its pixels, and the share of it inside their
/// bevel, which a sweep and a flash cover.
const ICON: f32 = 36.0;
const INSIDE: f32 = (ICON - 4.0) / ICON * 100.0;
/// The focus ring's width round the selected slot.
const BORDER: f32 = 2.0;
/// The key's plate: its text's size and line, its padding and the alpha
/// the theme's background takes behind it.
const KEY_TEXT: [f32; 2] = [9.5, 12.0];
const KEY_PAD: f32 = 2.0;
const PLATE_ALPHA: u8 = 225;
/// The most charges a slot shows, and a pip's side.
pub const PIPS: usize = 3;
const PIP: f32 = 4.0;
/// A slot's flash: its seconds and its peak alpha, white as a cooldown of
/// at least [`FLASH_COOLDOWN`] seconds ends (I14), the danger colour on a
/// press refused as not ready (I1).
const FLASH_SECS: f32 = 0.25;
const FLASH_COOLDOWN: f32 = 0.5;
const READY_ALPHA: f32 = 120.0;
const REFUSED_ALPHA: f32 = 110.0;
/// The summon's name over its slot after a turn: its seconds, the last of
/// them fading, its plate's height, padding and background alpha.
const NAMED_SECS: f32 = 1.2;
const NAMED_FADE: f32 = 0.3;
const NAME_PLATE: f32 = 20.0;
const NAME_PAD: f32 = 6.0;
const NAME_ALPHA: u8 = 200;
/// The room that sets the gun apart from the spells, and its divider's
/// alpha in the theme's border.
const APART: f32 = 26.0;
const DIVIDER_ALPHA: u8 = 120;
/// The most rounds the magazine shows, the gun's icon, the cartridges'
/// art and the texels between them, and the room their count takes in
/// mono, so "reloading" never shifts the bar, with its size and line.
pub const ROUNDS: usize = MAGAZINE as usize;
const GUN_ICON: &str = "ex10_icon_gun";
const LIVE_ROUND: &str = "ex10_hud_bullet";
const SPENT_ROUND: &str = "ex10_hud_bullet_spent";
const BULLET: [f32; 2] = [5.0, 11.0];
const BULLET_GAP: f32 = 2.0;
const MAGAZINE_READOUT: f32 = 72.0;
const COUNT_TEXT: [f32; 2] = [12.0, 16.0];
/// The veil over the part of a cooldown still to run.
const VEIL: [u8; 4] = [11, 12, 16, 170];
/// What an icon the player cannot pay for is multiplied by.
const GREYED: [u8; 4] = [120, 120, 128, 150];
const WHITE: [u8; 4] = [255; 4];
const CLEAR: [u8; 4] = [0; 4];
/// The book's panel: its width, the name's size and line, the alpha the
/// theme's background takes behind it, its border's width and the room
/// between it and the book's top.
const PANEL_WIDTH: f32 = 280.0;
const PANEL_NAME: [f32; 2] = [17.0, 22.0];
const PANEL_ALPHA: u8 = 230;
const PANEL_BORDER: f32 = 1.0;
const PANEL_LIFT: f32 = 10.0;
/// What the panel's last line says, the key from the player's bindings.
const TAKE_PROMPT: &str = "{action:interact} Take";

/// Builds the HUD for the run gameplay starts: its roots owned by gameplay,
/// shown and frozen under a pause.
pub fn start(world: &mut World) {
    let assets = world.resource_mut::<AssetRegistry>();
    let icons = SLOTS.map(|slot| slot.map(|spell| (spell, assets.intern_sprite(spell.icon()))));
    let art = Art {
        gun: assets.intern_sprite(GUN_ICON),
        health: assets.intern_sprite(HEALTH_LIQUID),
        mana: assets.intern_sprite(MANA_LIQUID),
        shield: assets.intern_sprite(SHIELD_LIQUID),
        live: assets.intern_sprite(LIVE_ROUND),
        spent: assets.intern_sprite(SPENT_ROUND),
        tomes: TomeKind::ALL.map(|kind| assets.intern_sprite(kind.icon())),
    };
    let scale = ui_scale(world);
    let tree = world.resource_mut::<UiTree>();
    let hud = build(tree, icons, art, scale);
    theme::slide_in(tree, hud.meters);
    theme::slide_in(tree, hud.bar);
    let stack = shell::stack(world);
    stack.own_root(hud.meters, RootCover::Show);
    stack.own_root(hud.bar, RootCover::Show);
    stack.own_root(hud.panel.root, RootCover::Hide);
    world.insert_resource(hud);
    update(world);
}

/// The UI's scale: screen pixels a UI unit.
fn ui_scale(world: &World) -> f32 {
    world
        .get_resource::<ScaleFactor>()
        .map_or(1.0, ScaleFactor::get)
}

/// An art pixel's size in UI units at `scale`: the largest whole number of
/// screen pixels within it, at least one.
fn texel(scale: f32) -> f32 {
    scale.floor().max(1.0) / scale
}

/// `units` UI units at `scale`, to whole screen pixels.
fn snap(units: f32, scale: f32) -> f32 {
    (units * scale).round() / scale
}

/// The bars' root at `scale`: its corner, padding and gap on whole screen
/// pixels, so the liquids' tiles meet on whole pixels and no seam shows.
fn meters_style(scale: f32) -> LayoutStyle {
    let gap = snap(theme::GAP, scale);
    LayoutStyle::column()
        .with_gap(gap)
        .with_padding(gap)
        .anchored(Anchor::TopStart, Vec2::splat(snap(theme::MARGIN, scale)))
}

/// Notes a press of the selected spell refused as not ready: its slot
/// flashes in the danger colour (`refuse_cast`).
pub fn refused(world: &mut World) {
    let slot = world
        .get_resource::<Run>()
        .and_then(|run| world.get::<Spellbook>(run.player))
        .map(|book| book.slot);
    if let (Some(slot), Some(hud)) = (slot, world.get_resource_mut::<Hud>())
        && let Some(left) = hud.refused.get_mut(slot)
    {
        *left = FLASH_SECS;
    }
}

/// Notes a turn of the summon's type: its name shows over its slot.
pub fn summon_turned(world: &mut World) {
    if let Some(hud) = world.get_resource_mut::<Hud>() {
        hud.named = NAMED_SECS;
    }
}

/// `style` sized `width` by `height` UI units.
fn sized(style: LayoutStyle, width: f32, height: f32) -> LayoutStyle {
    style.with_size(Sizing::Px(width), Sizing::Px(height))
}

/// Resizes `id` to `width` by `height` UI units, its style kept.
fn resize(tree: &mut UiTree, id: WidgetId, width: f32, height: f32) {
    let style = tree.layout_style(id).cloned().unwrap_or_default();
    tree.set_layout_style(id, sized(style, width, height));
}

/// A colour of the theme's at `alpha`.
const fn faded([r, g, b, _]: [u8; 4], alpha: u8) -> [u8; 4] {
    [r, g, b, alpha]
}

/// A label for numbers in mono at `size`, never wrapping.
fn readout(tree: &mut UiTree, parent: WidgetId, [size, line]: [f32; 2], room: f32) -> WidgetId {
    let text = tree.label(parent, "");
    let mut style = tree.layout_style(text).cloned().unwrap_or_default();
    style.min_width = Some(room);
    tree.set_layout_style(text, style);
    widgets::mono(tree, text);
    widgets::restyle(tree, text, |text| {
        (text.size, text.line_height) = (Some(size), Some(line));
        text.wrap = Some(TextWrap::None);
    });
    text
}

fn build(
    tree: &mut UiTree,
    icons: [Option<(Spell, SpriteAssetId)>; SLOT_COUNT],
    art: Art,
    scale: f32,
) -> Hud {
    let colors = tree.theme().tokens.colors;
    let meters = tree.add_root(UiLayer::Game, meters_style(scale));
    tree.set_hit_testable(meters, false);
    widgets::fill(tree, meters, faded(colors.background, METERS_ALPHA));
    let health = meter(tree, meters, art.health, HEALTH_EDGE);
    let shield = shield_bar(tree, &health, art.shield);
    let mana = meter(tree, meters, art.mana, MANA_EDGE);
    let strip = strip(tree, meters, art);
    let style = LayoutStyle::row().with_gap(theme::GAP).anchored(
        Anchor::BottomCenter,
        Vec2::new(0.0, theme::MARGIN + TOAST_LANE + theme::GAP),
    );
    let bar = tree.add_root(UiLayer::Game, style);
    tree.set_hit_testable(bar, false);
    widgets::fill(tree, bar, CLEAR);
    let slots: Vec<Slot> = icons.into_iter().map(|art| slot(tree, bar, art)).collect();
    let summon = slot_of(Spell::Summon).unwrap_or(SLOT_COUNT - 1);
    let (name_plate, name) = name_plate(tree, slots[summon].ring);
    divider(tree, bar);
    let magazine = magazine(tree, bar, art);
    let panel = book_panel(tree);
    let hud = Hud {
        meters,
        bar,
        health,
        shield,
        mana,
        strip,
        slots: slots.try_into().expect("a tile a slot"),
        name_plate,
        name,
        magazine,
        panel,
        art,
        scale,
        ready: [0.0; SLOT_COUNT],
        refused: [0.0; SLOT_COUNT],
        named: 0.0,
        cooling: [0.0; SLOT_COUNT],
    };
    size(tree, &hud, scale);
    hud
}

/// Sizes everything drawn in art pixels to `scale`: the tiles, the bars and
/// the cartridges; and puts the bars on whole pixels.
fn size(tree: &mut UiTree, hud: &Hud, scale: f32) {
    let texel = texel(scale);
    tree.set_layout_style(hud.meters, meters_style(scale));
    let tile = ICON * texel;
    for id in hud
        .slots
        .iter()
        .map(|slot| slot.tile)
        .chain([hud.magazine.tile])
        .chain(hud.strip.tiles)
    {
        resize(tree, id, tile, tile);
    }
    let [width, height] = METER;
    let edge = TRACK_EDGE * texel;
    for meter in [hud.health, hud.mana] {
        let track = LayoutStyle::row().with_padding(edge);
        let (outer, tall) = (width * texel + 2.0 * edge, height * texel + 2.0 * edge);
        tree.set_layout_style(meter.track, sized(track, outer, tall));
        resize(tree, meter.well, width * texel, height * texel);
        let strip = LIQUID[0] * LIQUID_TILES as f32;
        resize(tree, meter.strip, strip * texel, height * texel);
        for tile in tree.children(meter.strip).to_vec() {
            resize(tree, tile, LIQUID[0] * texel, LIQUID[1] * texel);
        }
        resize(tree, meter.edge, texel, height * texel);
    }
    let strip = LIQUID[0] * LIQUID_TILES as f32;
    resize(tree, hud.shield.strip, strip * texel, height * texel);
    for tile in tree.children(hud.shield.strip).to_vec() {
        resize(tree, tile, LIQUID[0] * texel, LIQUID[1] * texel);
    }
    resize(tree, hud.shield.edge, texel, height * texel);
    for round in hud.magazine.rounds {
        resize(tree, round, BULLET[0] * texel, BULLET[1] * texel);
    }
    let row = LayoutStyle::row().with_gap(BULLET_GAP * texel);
    tree.set_layout_style(hud.magazine.row, row);
}

/// A bar's widgets, `liquid` its texture and `edge` the colour where it
/// stands, then its readout; [`size`] gives them their sizes.
fn meter(tree: &mut UiTree, parent: WidgetId, liquid: SpriteAssetId, edge: [u8; 4]) -> Meter {
    let row = widgets::row(tree, parent);
    let track = tree.panel(row, LayoutStyle::row());
    let well = tree.panel(track, LayoutStyle::row());
    widgets::fill(tree, well, WELL);
    let fill = tree.panel(
        well,
        LayoutStyle::default().with_size(Sizing::Percent(100.0), Sizing::Fill),
    );
    widgets::fill(tree, fill, CLEAR);
    tree.set_clip_children(fill, true);
    let strip = tree.panel(
        fill,
        LayoutStyle::row().anchored(Anchor::TopStart, Vec2::ZERO),
    );
    widgets::fill(tree, strip, CLEAR);
    for _ in 0..LIQUID_TILES {
        tree.image(strip, liquid, Vec2::from(LIQUID), LayoutStyle::default());
    }
    let whole = LayoutStyle::default()
        .with_size(Sizing::Percent(100.0), Sizing::Percent(100.0))
        .anchored(Anchor::TopStart, Vec2::ZERO);
    let pulse = tree.panel(fill, whole);
    widgets::fill(tree, pulse, CLEAR);
    let stand = tree.panel(
        fill,
        LayoutStyle::default().anchored(Anchor::TopEnd, Vec2::ZERO),
    );
    widgets::fill(tree, stand, edge);
    let text = readout(tree, row, READOUT_TEXT, READOUT);
    Meter {
        track,
        well,
        fill,
        strip,
        pulse,
        edge: stand,
        text,
    }
}

/// The shield's layer in `health`'s well, over its fill, and its readout
/// after the health's, collapsed; [`size`] gives them their sizes.
fn shield_bar(tree: &mut UiTree, health: &Meter, liquid: SpriteAssetId) -> ShieldBar {
    let style = LayoutStyle::default()
        .with_size(Sizing::Percent(0.0), Sizing::Percent(100.0))
        .anchored(Anchor::TopStart, Vec2::ZERO);
    let fill = tree.panel(health.well, style);
    widgets::fill(tree, fill, CLEAR);
    tree.set_clip_children(fill, true);
    let strip = tree.panel(
        fill,
        LayoutStyle::row().anchored(Anchor::TopStart, Vec2::ZERO),
    );
    widgets::fill(tree, strip, CLEAR);
    for _ in 0..LIQUID_TILES {
        tree.image(strip, liquid, Vec2::from(LIQUID), LayoutStyle::default());
    }
    let edge = tree.panel(
        fill,
        LayoutStyle::default().anchored(Anchor::TopEnd, Vec2::ZERO),
    );
    widgets::fill(tree, edge, SHIELD_EDGE);
    let row = tree.parent(health.text).expect("the health bar's row");
    let text = readout(tree, row, READOUT_TEXT, SHIELD_READOUT);
    widgets::restyle(tree, text, |text| text.color = Some(SHIELD_TEXT));
    widgets::show(tree, fill, false);
    widgets::show(tree, text, false);
    ShieldBar {
        fill,
        strip,
        edge,
        text,
    }
}

/// The tomes' row under the bars, collapsed, its tiles in the icon
/// family; [`size`] sizes them as the spell bar's.
fn strip(tree: &mut UiTree, parent: WidgetId, art: Art) -> Strip {
    let row = tree.panel(parent, LayoutStyle::row().with_gap(theme::GAP / 2.0));
    widgets::fill(tree, row, CLEAR);
    let tiles = art
        .tomes
        .map(|icon| tree.image(row, icon, Vec2::splat(ICON), LayoutStyle::default()));
    widgets::show(tree, row, false);
    Strip { row, tiles }
}

/// The book's panel, a root of its own placed by [`panel`], hidden.
fn book_panel(tree: &mut UiTree) -> Panel {
    let colors = tree.theme().tokens.colors;
    let style = LayoutStyle::column()
        .with_gap(theme::GAP / 2.0)
        .with_padding(theme::GAP)
        .with_size(Sizing::Px(PANEL_WIDTH), Sizing::Auto)
        .anchored(Anchor::TopStart, Vec2::ZERO);
    let root = tree.add_root(UiLayer::Game, style);
    tree.set_hit_testable(root, false);
    let background = faded(colors.background, PANEL_ALPHA);
    edged(tree, root, colors.border, background, PANEL_BORDER);
    let name = widgets::display(tree, root, "", PANEL_NAME);
    widgets::restyle(tree, name, |text| text.align = None);
    let gives = tree.label(root, "");
    let costs = tree.label(root, "");
    widgets::danger(tree, costs);
    widgets::prompt_row(tree, root, TAKE_PROMPT);
    tree.set_visibility(root, Visibility::Hidden);
    Panel {
        root,
        name,
        gives,
        costs,
    }
}

/// Shows the panel for `book`, its kind and its centre, over the book and
/// kept on screen, or hides it with none: what `tomes_panel` sets after
/// the camera's update.
pub fn panel(world: &mut World, book: Option<(TomeKind, Vec2)>) {
    let Some(panel) = world.get_resource::<Hud>().map(|hud| hud.panel) else {
        return;
    };
    let Some((kind, centre)) = book else {
        world
            .resource_mut::<UiTree>()
            .set_visibility(panel.root, Visibility::Hidden);
        return;
    };
    let scale = ui_scale(world);
    let top = centre - Vec2::new(0.0, BOOK_CENTRE.y);
    let at = world.resource::<CameraState>().world_to_screen(top) / scale;
    let tree = world.resource_mut::<UiTree>();
    for (id, text) in [
        (panel.name, kind.name()),
        (panel.gives, kind.gives()),
        (panel.costs, kind.costs()),
    ] {
        if widgets::text_of(tree, id) != text {
            tree.set_plain_text(id, text);
        }
    }
    // The last layout's size, over the book's top and inside the view.
    let size = tree
        .rect(panel.root)
        .map_or(Vec2::new(PANEL_WIDTH, 0.0), |rect| rect.size());
    let view = tree.layer_viewport(UiLayer::Game).unwrap_or(size);
    let margin = snap(theme::MARGIN, scale);
    let most = (view - size - Vec2::splat(margin)).max(Vec2::splat(margin));
    let corner =
        Vec2::new(at.x - size.x * 0.5, at.y - size.y - PANEL_LIFT).clamp(Vec2::splat(margin), most);
    let corner = Vec2::new(snap(corner.x, scale), snap(corner.y, scale));
    let style = tree.layout_style(panel.root).cloned().unwrap_or_default();
    tree.set_layout_style(panel.root, style.anchored(Anchor::TopStart, corner));
    tree.set_visibility(panel.root, Visibility::Visible);
}

/// Edges of `color`, `width` wide, round a `background` box.
fn edged(tree: &mut UiTree, id: WidgetId, color: [u8; 4], background: [u8; 4], width: f32) {
    let style = StyleOverrides {
        background: Some(background),
        border: Some(color),
        border_width: Some(width),
        ..StyleOverrides::default()
    };
    tree.set_overrides(id, style);
}

/// A slot: the ring, inside it the icon tile with its sweep, its flash, its
/// charges' pips and its key on a corner plate; an empty slot keeps its
/// place, its tile as large as a full one's.
fn slot(tree: &mut UiTree, bar: WidgetId, art: Option<(Spell, SpriteAssetId)>) -> Slot {
    let ring = tree.panel(bar, LayoutStyle::row().with_padding(BORDER));
    edged(tree, ring, CLEAR, CLEAR, BORDER);
    let tile = tree.panel(ring, LayoutStyle::column());
    widgets::fill(tree, tile, CLEAR);
    let colors = tree.theme().tokens.colors;
    let whole = LayoutStyle::default().with_size(Sizing::Percent(100.0), Sizing::Percent(100.0));
    let inside = LayoutStyle::default()
        .with_size(Sizing::Percent(INSIDE), Sizing::Percent(INSIDE))
        .anchored(Anchor::Center, Vec2::ZERO);
    let (icon, sweep, flash, pips) = match art {
        None => (None, None, None, None),
        Some((spell, icon)) => {
            let image = tree.image(tile, icon, Vec2::splat(ICON), whole);
            // A mesh's own box takes the panel's style: clear, so the icon
            // shows.
            let sweep = tree.mesh(tile, inside.clone());
            widgets::fill(tree, sweep, CLEAR);
            let flash = tree.panel(tile, inside);
            widgets::fill(tree, flash, CLEAR);
            let pips = (spell == Spell::IronBlock).then(|| {
                let style = LayoutStyle::row()
                    .with_gap(PIP / 2.0)
                    .anchored(Anchor::BottomCenter, Vec2::new(0.0, BORDER));
                let row = tree.panel(tile, style);
                widgets::fill(tree, row, CLEAR);
                [(); PIPS].map(|()| {
                    let pip = tree.panel(row, sized(LayoutStyle::default(), PIP, PIP));
                    widgets::fill(tree, pip, colors.text);
                    pip
                })
            });
            (Some(image), Some(sweep), Some(flash), pips)
        }
    };
    let mut style = LayoutStyle::row().anchored(Anchor::TopStart, Vec2::ZERO);
    style.padding = Edges::symmetric(KEY_PAD, 0.0);
    let plate = tree.panel(tile, style);
    widgets::fill(tree, plate, faded(colors.background, PLATE_ALPHA));
    let key = tree.label(plate, "");
    widgets::restyle(tree, key, |text| {
        (text.size, text.line_height) = (Some(KEY_TEXT[0]), Some(KEY_TEXT[1]));
        text.color = Some(colors.text);
        text.wrap = Some(TextWrap::None);
    });
    Slot {
        ring,
        tile,
        icon,
        sweep,
        flash,
        key,
        pips,
    }
}

/// The summon's name over its slot's `ring`, on a dark plate, hidden.
fn name_plate(tree: &mut UiTree, ring: WidgetId) -> (WidgetId, WidgetId) {
    let mut style =
        LayoutStyle::row().anchored(Anchor::TopCenter, Vec2::new(0.0, -(NAME_PLATE + BORDER)));
    style.height = Sizing::Px(NAME_PLATE);
    style.padding = Edges::symmetric(NAME_PAD, 0.0);
    style.align_items = Align::Center;
    let plate = tree.panel(ring, style);
    let colors = tree.theme().tokens.colors;
    widgets::fill(tree, plate, faded(colors.background, NAME_ALPHA));
    let name = tree.label(plate, "");
    widgets::restyle(tree, name, |text| {
        [text.size, text.line_height] = theme::SMALL.map(Some);
        text.color = Some(colors.text);
        text.wrap = Some(TextWrap::None);
    });
    tree.set_opacity(plate, 0.0);
    (plate, name)
}

/// The room that sets the gun apart from the spells, a thin divider in its
/// middle in the theme's border colour.
fn divider(tree: &mut UiTree, bar: WidgetId) {
    let mut style =
        LayoutStyle::row().with_size(Sizing::Px(APART - 2.0 * theme::GAP), Sizing::Auto);
    style.align_items = Align::Center;
    style.justify_content = Justify::Center;
    let room = tree.panel(bar, style);
    widgets::fill(tree, room, CLEAR);
    let line = tree.panel(
        room,
        LayoutStyle::default().with_size(Sizing::Px(1.0), Sizing::Percent(70.0)),
    );
    let border = tree.theme().tokens.colors.border;
    widgets::fill(tree, line, faded(border, DIVIDER_ALPHA));
}

/// The gun's block: its tile with the reload's sweep, no key and never
/// ringed, then its cartridges in a row with their count under them.
fn magazine(tree: &mut UiTree, bar: WidgetId, art: Art) -> Magazine {
    let ring = tree.panel(bar, LayoutStyle::row().with_padding(BORDER));
    widgets::fill(tree, ring, CLEAR);
    let tile = tree.panel(ring, LayoutStyle::column());
    widgets::fill(tree, tile, CLEAR);
    let whole = LayoutStyle::default().with_size(Sizing::Percent(100.0), Sizing::Percent(100.0));
    let icon = tree.image(tile, art.gun, Vec2::splat(ICON), whole);
    let sweep = tree.mesh(
        tile,
        LayoutStyle::default()
            .with_size(Sizing::Percent(INSIDE), Sizing::Percent(INSIDE))
            .anchored(Anchor::Center, Vec2::ZERO),
    );
    widgets::fill(tree, sweep, CLEAR);
    let mut style = LayoutStyle::column().with_gap(theme::GAP / 2.0);
    style.align_items = Align::Center;
    style.justify_content = Justify::Center;
    let column = tree.panel(bar, style);
    widgets::fill(tree, column, CLEAR);
    let row = tree.panel(column, LayoutStyle::row());
    widgets::fill(tree, row, CLEAR);
    let rounds = [(); ROUNDS]
        .map(|()| tree.image(row, art.live, Vec2::from(BULLET), LayoutStyle::default()));
    let text = readout(tree, column, COUNT_TEXT, MAGAZINE_READOUT);
    widgets::restyle(tree, text, |text| {
        text.align = Some(TextAlign::Center);
    });
    Magazine {
        tile,
        icon,
        sweep,
        row,
        text,
        rounds,
    }
}

/// What a slot shows of its spell: whether the player can pay for a use,
/// the share of its cooldown still to run and the cooldown's length, and
/// its charges left of its most.
struct Shown {
    payable: bool,
    cooling: f32,
    cooldown: f32,
    charges: Option<(u32, u32)>,
}

fn shown(world: &World, player: Entity, spell: Spell, mana: f32) -> Option<Shown> {
    let ability = world
        .get_resource::<DataRegistry>()?
        .get::<Ability>(spell.ability())?;
    let abilities = world.get::<Abilities>(player)?;
    let name = spell.ability();
    let cooldown = ability.cooldown();
    let cooling = if cooldown > 0.0 {
        abilities.cooldown_left(name).unwrap_or(0.0) / cooldown
    } else {
        0.0
    };
    let charges = ability
        .charges()
        .map(|(most, _, _)| (abilities.charges(name).unwrap_or(most), most));
    let payable = ability
        .cost_for(world.get::<Stats>(player))
        .is_none_or(|(_, cost)| mana >= cost)
        && charges.is_none_or(|(left, _)| left > 0);
    Some(Shown {
        payable,
        cooling,
        cooldown,
        charges,
    })
}

/// Whether `player` can pay for a use of `spell` now, its cost scaled by
/// the player's `spell_cost` and a charge left: what greys a slot.
#[must_use]
pub fn payable(world: &World, player: Entity, spell: Spell) -> bool {
    shown(world, player, spell, mana(world, player)).is_some_and(|shown| shown.payable)
}

/// The key a slot shows for `action`: its first key or mouse binding, else
/// its first, [`short`]ened; none for an action with no binding.
fn key_of(actions: Option<&ActionMap>, action: &str) -> String {
    let bindings = actions.map_or(&[][..], |actions| actions.bindings(action));
    bindings
        .iter()
        .find(|binding| matches!(binding, Binding::Key { .. } | Binding::Mouse { .. }))
        .or_else(|| bindings.first())
        .map_or_else(String::new, |binding| short(&binding.label()))
}

/// A binding's label in three characters at most (Q5): as written when it
/// fits; a mouse button as `LMB`, `RMB`, `MMB` or `M<n>`; `Spc` for the
/// space bar; a side and a key as the side's initial and the key's first
/// two letters (`LSh`); another two-word label as its first word's first
/// two letters and its second's initial (`WhU`); else its first three.
#[must_use]
pub fn short(label: &str) -> String {
    if label.chars().count() <= 3 {
        return label.to_string();
    }
    match label {
        "Left Mouse" => return "LMB".into(),
        "Right Mouse" => return "RMB".into(),
        "Middle Mouse" => return "MMB".into(),
        "Space" => return "Spc".into(),
        _ => {}
    }
    if let Some(button) = label.strip_prefix("Mouse ") {
        return format!("M{button}");
    }
    let first = |word: &str, n: usize| word.chars().take(n).collect::<String>();
    match label.split(' ').collect::<Vec<_>>()[..] {
        [side @ ("Left" | "Right"), key] => format!("{}{}", first(side, 1), first(key, 2)),
        [one, two] => format!("{}{}", first(one, 2), first(two, 1)),
        _ => first(label, 3),
    }
}

/// `text` with its first letter capitalised: a summon type's name.
fn titled(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// `spells_hud`, in `Update`: writes the run's health, mana, spells and gun
/// into the HUD, and runs its flashes and the summon's name by the game
/// clock.
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
    let shield = world.get::<Shield>(player).map_or(0.0, Shield::current);
    let held = mana(world, player);
    let most = world
        .get::<Stats>(player)
        .and_then(|stats| stats.get("max_mana"))
        .unwrap_or(1.0);
    let selected = world.get::<Spellbook>(player).map(|book| book.slot);
    let states = SLOTS.map(|slot| slot.and_then(|spell| shown(world, player, spell, held)));
    let keys = SLOT_ACTIONS.map(|action| key_of(world.get_resource::<ActionMap>(), action));
    let mut taken = [None; 3];
    for (slot, kind) in taken.iter_mut().zip(tomes::held(world, player)) {
        *slot = Some(*kind);
    }
    let rounds = gun::rounds(world, player);
    let reload = world.get::<Reload>(player).map(|reload| reload.left);
    let summoned = summon_type(world);
    let summoned_icon = world
        .resource_mut::<AssetRegistry>()
        .intern_sprite(summoned.icon);
    let scale = ui_scale(world);
    let texel = texel(scale);
    let time = drawn_time(world);
    let dt = world.resource::<Time>().delta();
    let tree = world.resource_mut::<UiTree>();
    let colors = tree.theme().tokens.colors;
    if scale != hud.scale {
        size(tree, &hud, scale);
        hud.scale = scale;
    }
    let low = health.0 * 3.0 <= health.1;
    let beat = 0.5 + 0.5 * (time * TAU * PULSE_HZ).sin();
    let health_flow = time * HEALTH_FLOW;
    fill(tree, hud.health, health, shown_health, health_flow, texel);
    shield_fill(
        tree,
        hud.shield,
        shield / health.1.max(f32::EPSILON),
        shield,
        time,
        texel,
    );
    let border = if low && beat > 0.5 {
        colors.danger
    } else {
        colors.border
    };
    // The tree keeps equal overrides as they were, so these mark nothing
    // while nothing changes.
    let track = faded(colors.background, TRACK_ALPHA);
    edged(tree, hud.health.track, border, track, texel);
    let glow = if low {
        faded(WHITE, (PULSE * beat).round() as u8)
    } else {
        CLEAR
    };
    widgets::fill(tree, hud.health.pulse, glow);
    let held = held.floor();
    let mana_flow = time * MANA_FLOW + MANA_PHASE;
    fill(tree, hud.mana, (held, most), held, mana_flow, texel);
    edged(tree, hud.mana.track, colors.border, track, texel);
    for (index, ((slot, state), key)) in hud.slots.iter().zip(states).zip(keys).enumerate() {
        let ring = if selected == Some(index) {
            colors.focus
        } else {
            CLEAR
        };
        edged(tree, slot.ring, ring, CLEAR, BORDER);
        if widgets::text_of(tree, slot.key) != key {
            tree.set_plain_text(slot.key, &key);
        }
        hud.ready[index] = (hud.ready[index] - dt).max(0.0);
        hud.refused[index] = (hud.refused[index] - dt).max(0.0);
        let (Some(icon), Some(sweep), Some(flash)) = (slot.icon, slot.sweep, slot.flash) else {
            continue;
        };
        if SLOTS[index] == Some(Spell::Summon) && tree.image_sprite(icon) != Some(summoned_icon) {
            tree.set_image(icon, summoned_icon, Vec2::splat(ICON));
        }
        let state = state.unwrap_or(Shown {
            payable: false,
            cooling: 0.0,
            cooldown: 0.0,
            charges: None,
        });
        if hud.cooling[index] > 0.0 && state.cooling <= 0.0 && state.cooldown >= FLASH_COOLDOWN {
            hud.ready[index] = FLASH_SECS;
        }
        hud.cooling[index] = state.cooling;
        tree.set_image_tint(icon, if state.payable { WHITE } else { GREYED });
        tree.set_mesh(sweep, &veil(state.cooling));
        let lit = if hud.refused[index] > 0.0 {
            faded(colors.danger, alpha(REFUSED_ALPHA, hud.refused[index]))
        } else if hud.ready[index] > 0.0 {
            faded(WHITE, alpha(READY_ALPHA, hud.ready[index]))
        } else {
            CLEAR
        };
        widgets::fill(tree, flash, lit);
        if let (Some(pips), Some((left, most))) = (slot.pips, state.charges) {
            for (i, pip) in pips.into_iter().enumerate() {
                let color = if i < left as usize {
                    colors.text
                } else {
                    colors.disabled_text
                };
                widgets::show(tree, pip, i < most as usize);
                widgets::fill(tree, pip, color);
            }
        }
    }
    widgets::show(tree, hud.strip.row, taken[0].is_some());
    for (tile, kind) in hud.strip.tiles.into_iter().zip(taken) {
        widgets::show(tree, tile, kind.is_some());
        if let Some(kind) = kind {
            let art = hud.art.tomes[kind.index()];
            if tree.image_sprite(tile) != Some(art) {
                tree.set_image(tile, art, Vec2::splat(ICON));
            }
        }
    }
    hud.named = (hud.named - dt).max(0.0);
    let name = titled(summoned.name);
    if widgets::text_of(tree, hud.name) != name {
        tree.set_plain_text(hud.name, &name);
    }
    tree.set_opacity(hud.name_plate, (hud.named / NAMED_FADE).min(1.0));
    if let Some((left, most)) = rounds {
        let readout = if reload.is_some() {
            "reloading".to_string()
        } else {
            format!("{left} / {most}")
        };
        if widgets::text_of(tree, hud.magazine.text) != readout {
            tree.set_plain_text(hud.magazine.text, &readout);
        }
        let shade = if reload.is_some() {
            colors.text_muted
        } else {
            colors.text
        };
        widgets::restyle(tree, hud.magazine.text, |text| text.color = Some(shade));
        let tint = if reload.is_some() { GREYED } else { WHITE };
        tree.set_image_tint(hud.magazine.icon, tint);
        let sweep = reload.map_or(0.0, |left| left / RELOAD_SECS);
        tree.set_mesh(hud.magazine.sweep, &veil(sweep));
        for (i, round) in hud.magazine.rounds.into_iter().enumerate() {
            widgets::show(tree, round, i < most as usize);
            let art = if i < left as usize && reload.is_none() {
                hud.art.live
            } else {
                hud.art.spent
            };
            if tree.image_sprite(round) != Some(art) {
                tree.set_image(round, art, Vec2::from(BULLET));
            }
        }
    }
    world.insert_resource(hud);
}

/// A flash's alpha from its `peak`, fading over [`FLASH_SECS`] with `left`
/// of them to go.
fn alpha(peak: f32, left: f32) -> u8 {
    (peak * (left / FLASH_SECS).clamp(0.0, 1.0)).round() as u8
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

/// Fills the shield's layer to `share` of the bar, capped at its width,
/// scrolls its liquid by the game clock `time`, and writes `+shield`,
/// rounded up, after the health's readout; with no shield, both collapse.
fn shield_fill(tree: &mut UiTree, bar: ShieldBar, share: f32, shield: f32, time: f32, texel: f32) {
    let held = shield.ceil();
    widgets::show(tree, bar.fill, held > 0.0);
    widgets::show(tree, bar.text, held > 0.0);
    if held <= 0.0 {
        return;
    }
    let text = format!("+{held}");
    if widgets::text_of(tree, bar.text) != text {
        tree.set_plain_text(bar.text, &text);
    }
    let width = Sizing::Percent((share * 100.0).clamp(0.0, 100.0));
    if tree
        .layout_style(bar.fill)
        .is_some_and(|style| style.width != width)
    {
        let style = LayoutStyle::default()
            .with_size(width, Sizing::Percent(100.0))
            .anchored(Anchor::TopStart, Vec2::ZERO);
        tree.set_layout_style(bar.fill, style);
    }
    let scrolled = (time * SHIELD_FLOW).rem_euclid(LIQUID[0]) * texel;
    tree.set_translation(bar.strip, Vec2::new(-scrolled, 0.0));
}

/// Writes `shown / max` into `meter`, fills it to `current / max` and
/// scrolls its liquid `flow` texels left, a texel `texel` UI units.
fn fill(
    tree: &mut UiTree,
    meter: Meter,
    (current, max): (f32, f32),
    shown: f32,
    flow: f32,
    texel: f32,
) {
    let text = format!("{shown} / {max}");
    if widgets::text_of(tree, meter.text) != text {
        tree.set_plain_text(meter.text, &text);
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
    let scrolled = flow.rem_euclid(LIQUID[0]) * texel;
    tree.set_translation(meter.strip, Vec2::new(-scrolled, 0.0));
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
