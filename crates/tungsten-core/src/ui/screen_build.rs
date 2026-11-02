//! Screens built from files (redo 7.20): [`build`] makes a new root from a
//! `ui_screens` file through [`widgets`]' builders, in file order, and
//! records the live screen in [`UiScreens`]; [`refresh`] writes the game's
//! bindings ([`UiBindings`]) into it compare-on-write, so an equal value
//! marks nothing, rebuilds a repeat's rows only when they change and leaves
//! the control the router has captured alone; [`action`], [`revision`] and
//! [`node`] answer the screen's owner. A binding is a plain `fn` a plugin
//! registers by name in its `build`; none takes arguments, so a screen's
//! state that a binding shows lives in a resource its state owns.

use std::collections::{BTreeMap, HashMap, HashSet};

use glam::Vec2;

use crate::ecs::World;

use super::event::UiValue;
use super::id::{UiKey, WidgetId};
use super::node::UiLayer;
use super::route::UiRouter;
use super::screen_file::{
    AlignName, CornerName, DirectionName, EdgesFile, JustifyName, Kind, LayerName, LayoutFile,
    Length, LimitFile, Node, ScreenAction, ScreenNames, SizingFile, Spacing, UiScreenRegistry,
};
use super::style::{Align, Anchor, Direction, Edges, Justify, LayoutStyle, Placement, Sizing};
use super::theme_file::{Color, OverridesFile, resolve};
use super::tree::UiTree;
use super::widgets::{self, Meter};
use super::{SliderRange, Visibility};

/// A row a provider gives a `repeat`: its key, which keys the row's node
/// `<repeat key>/<row key>`, and its fields by name, which the row's
/// `field` nodes show.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UiRow {
    /// The row's key: a name without a `/`, unique among the rows.
    pub key: String,
    /// The row's fields by name.
    pub fields: BTreeMap<String, String>,
}

impl UiRow {
    /// A row keyed `key` with no fields.
    #[must_use]
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            fields: BTreeMap::new(),
        }
    }

    /// This row with `field` set to `value`.
    #[must_use]
    pub fn with(mut self, field: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.insert(field.into(), value.into());
        self
    }
}

/// What an `{"action": …}` gets: the control's key in the file (a repeat
/// row's node: the repeat's), the row it sits in, and a toggle's or a
/// slider's value.
#[derive(Debug, Clone, PartialEq)]
pub struct UiAction {
    /// The control's key in the file; `None` when it has none.
    pub key: Option<String>,
    /// The key of the repeat row the control sits in.
    pub row: Option<String>,
    /// A toggle's or a slider's value; `None` for a button or a choice row.
    pub value: Option<UiValue>,
}

/// A text binding: a text node's, a button's, a choice row's value or a
/// slider row's readout.
pub type TextBinding = fn(&World) -> String;
/// A value binding: a toggle's state, a slider's value or a tab rail's
/// selected tab.
pub type ValueBinding = fn(&World) -> UiValue;
/// A meter's fill, 0 to 1.
pub type FillBinding = fn(&World) -> f32;
/// A visible or enabled binding.
pub type FlagBinding = fn(&World) -> bool;
/// A repeat's rows.
pub type RowProvider = fn(&World) -> Vec<UiRow>;
/// A mount: fills the node it is called with.
pub type MountFn = fn(&mut World, WidgetId);
/// An action a control's `on` names.
pub type ActionFn = fn(&mut World, UiAction);

/// The names a game gives its screens' bindings, row providers, mounts and
/// actions: a `World` resource ([`UiBindings::of`] inserts it) a plugin
/// registers into from its `build`. A second registration of a name
/// replaces the first.
#[derive(Debug, Clone, Default)]
pub struct UiBindings {
    text: HashMap<&'static str, TextBinding>,
    value: HashMap<&'static str, ValueBinding>,
    fill: HashMap<&'static str, FillBinding>,
    visible: HashMap<&'static str, FlagBinding>,
    enabled: HashMap<&'static str, FlagBinding>,
    rows: HashMap<&'static str, RowProvider>,
    mounts: HashMap<&'static str, MountFn>,
    actions: HashMap<&'static str, ActionFn>,
}

impl UiBindings {
    /// The world's bindings, inserted empty when it has none.
    pub fn of(world: &mut World) -> &mut Self {
        if !world.has_resource::<Self>() {
            world.insert_resource(Self::default());
        }
        world.resource_mut::<Self>()
    }

    /// Registers text binding `name`.
    pub fn text(&mut self, name: &'static str, read: TextBinding) -> &mut Self {
        self.text.insert(name, read);
        self
    }

    /// Registers value binding `name`: a toggle's [`UiValue::Bool`], a
    /// slider's [`UiValue::Number`] or a tab rail's selected tab's index as
    /// a number.
    pub fn value(&mut self, name: &'static str, read: ValueBinding) -> &mut Self {
        self.value.insert(name, read);
        self
    }

    /// Registers fill binding `name`, a meter's share.
    pub fn fill(&mut self, name: &'static str, read: FillBinding) -> &mut Self {
        self.fill.insert(name, read);
        self
    }

    /// Registers visible binding `name`: a node shown, or taken out of the
    /// layout.
    pub fn visible(&mut self, name: &'static str, read: FlagBinding) -> &mut Self {
        self.visible.insert(name, read);
        self
    }

    /// Registers enabled binding `name`.
    pub fn enabled(&mut self, name: &'static str, read: FlagBinding) -> &mut Self {
        self.enabled.insert(name, read);
        self
    }

    /// Registers row provider `name`, a repeat's rows.
    pub fn rows(&mut self, name: &'static str, read: RowProvider) -> &mut Self {
        self.rows.insert(name, read);
        self
    }

    /// Registers mount `name`, called with its node after each build.
    pub fn mount(&mut self, name: &'static str, fill: MountFn) -> &mut Self {
        self.mounts.insert(name, fill);
        self
    }

    /// Registers action `name`.
    pub fn action(&mut self, name: &'static str, run: ActionFn) -> &mut Self {
        self.actions.insert(name, run);
        self
    }

    /// The action registered as `name`.
    #[must_use]
    pub fn action_named(&self, name: &str) -> Option<ActionFn> {
        self.actions.get(name).copied()
    }

    /// The registered names by kind, in a [`ScreenNames`] whose styles,
    /// sizes and screens are left empty.
    #[must_use]
    pub fn names(&self) -> ScreenNames {
        fn names<T>(map: &HashMap<&'static str, T>) -> std::collections::BTreeSet<String> {
            map.keys().map(|name| (*name).to_owned()).collect()
        }
        ScreenNames {
            text: names(&self.text),
            value: names(&self.value),
            fill: names(&self.fill),
            visible: names(&self.visible),
            enabled: names(&self.enabled),
            rows: names(&self.rows),
            mounts: names(&self.mounts),
            actions: names(&self.actions),
            ..ScreenNames::default()
        }
    }
}

/// A control's action in a built screen, from its `on`: what to do, the key
/// and row its [`UiAction`] carries, and whether it runs at the control's
/// commit, as a toggle's and a slider's do, or at its activation, as a
/// button's and a choice row's do.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyedAction {
    /// What the file's `on` says.
    pub action: ScreenAction,
    /// The control's key in the file: a repeat row's node takes the
    /// repeat's.
    pub key: Option<String>,
    /// The key of the repeat row the control sits in.
    pub row: Option<String>,
    /// Whether it runs at the control's commit rather than its activation.
    pub on_commit: bool,
    /// Whether a value binding shows the control's value; a setting's
    /// control without one shows the setting.
    pub value_bound: bool,
}

/// A screen [`build`] made: its root, and where the file's children went,
/// a `screen`'s card or a bare root itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltScreen {
    /// The root.
    pub root: WidgetId,
    /// A `screen`'s card; a bare root itself.
    pub card: WidgetId,
}

/// The live screens built from files, a `World` resource [`build`] inserts:
/// each one's ID, root, revision, bound nodes, repeats and keyed actions.
/// Entries whose root is gone are pruned at the next build.
#[derive(Debug, Default)]
pub struct UiScreens {
    live: Vec<Live>,
    warned: HashSet<String>,
}

impl UiScreens {
    /// The live screens' roots and the IDs they were built from, in build
    /// order.
    pub fn roots(&self) -> impl Iterator<Item = (WidgetId, &str)> {
        self.live.iter().map(|live| (live.root, live.id.as_str()))
    }

    /// Logs `message` once per `once`.
    fn warn(&mut self, once: String, message: &str) {
        if self.warned.insert(once) {
            log::warn!("{message}");
        }
    }
}

#[derive(Debug)]
struct Live {
    id: String,
    root: WidgetId,
    revision: u32,
    part: Part,
    repeats: Vec<Repeat>,
}

/// What a build records of a file's body or of a repeat's rows.
#[derive(Debug, Default)]
struct Part {
    bound: Vec<Bound>,
    actions: Vec<(WidgetId, KeyedAction)>,
}

#[derive(Debug)]
struct Repeat {
    container: WidgetId,
    key: String,
    rows_of: Option<RowProvider>,
    template: Node,
    /// The rows last built; `None` before the first refresh.
    rows: Option<Vec<UiRow>>,
    part: Part,
}

/// A bound node and its binding.
#[derive(Debug, Clone, Copy)]
enum Bound {
    Text(WidgetId, TextBinding),
    Toggle(WidgetId, ValueBinding),
    Slider(WidgetId, ValueBinding),
    Tabs(WidgetId, ValueBinding),
    Fill(Meter, [u8; 4], FillBinding),
    Visible(WidgetId, FlagBinding),
    Enabled(WidgetId, FlagBinding),
}

/// Builds the `ui_screens` file `id` as a new root, in file order, records
/// it in [`UiScreens`], calls each mount with its node, then [`refresh`]es
/// it. `None` for an ID not loaded, warning once. A name the world's
/// [`UiBindings`] lacks warns once and its binding, mount or action is
/// skipped; `tungsten check` refuses it.
pub fn build(world: &mut World, id: &str) -> Option<BuiltScreen> {
    let file = world
        .get_resource::<UiScreenRegistry>()
        .and_then(|registry| registry.get(id))
        .cloned();
    let mut screens = world.remove_resource::<UiScreens>().unwrap_or_default();
    let Some(file) = file else {
        screens.warn(
            format!("screen {id}"),
            &format!("no ui screen '{id}' is loaded"),
        );
        world.insert_resource(screens);
        return None;
    };
    let bindings = world
        .get_resource::<UiBindings>()
        .cloned()
        .unwrap_or_default();
    let tree = world.resource_mut::<UiTree>();
    screens.live.retain(|live| tree.contains(live.root));
    let mut builder = Builder::new(&bindings, id);
    let built = builder.root(tree, &file.root);
    let Builder {
        part,
        repeats,
        mounts,
        warnings,
        ..
    } = builder;
    for (once, message) in warnings {
        screens.warn(once, &message);
    }
    screens.live.push(Live {
        id: id.to_owned(),
        root: built.root,
        revision: 0,
        part,
        repeats,
    });
    world.insert_resource(screens);
    for (mount, node) in mounts {
        mount(world, node);
    }
    refresh(world, built.root);
    Some(built)
}

/// Writes every binding of the screen built at `root` from the world,
/// compare-on-write, so equal values mark nothing; first rebuilds each
/// repeat whose rows' keys or fields changed. It never writes the value of
/// the control the router has captured, from its press to its release or
/// cancel, so a drag keeps its draft. A root no screen was built at does
/// nothing.
pub fn refresh(world: &mut World, root: WidgetId) {
    let changed: Vec<(usize, Vec<UiRow>)> = {
        let shared: &World = world;
        let Some(live) = live(shared, root) else {
            return;
        };
        live.repeats
            .iter()
            .enumerate()
            .filter_map(|(at, repeat)| {
                let rows = (repeat.rows_of?)(shared);
                (repeat.rows.as_ref() != Some(&rows)).then_some((at, rows))
            })
            .collect()
    };
    for (at, rows) in changed {
        rebuild_rows(world, root, at, rows);
    }
    let captured = world
        .get_resource::<UiRouter>()
        .and_then(UiRouter::captured);
    let mut at = 0;
    while let Some(bound) = live(world, root).and_then(|live| live.bound(at)) {
        at += 1;
        write(world, bound, captured);
    }
}

/// The live screen built at `root`.
fn live(world: &World, root: WidgetId) -> Option<&Live> {
    let screens = world.get_resource::<UiScreens>()?;
    screens.live.iter().find(|live| live.root == root)
}

impl Live {
    /// The `at`th bound node: the body's, then each repeat's rows'.
    fn bound(&self, at: usize) -> Option<Bound> {
        let mut at = at;
        for part in
            std::iter::once(&self.part).chain(self.repeats.iter().map(|repeat| &repeat.part))
        {
            if let Some(bound) = part.bound.get(at) {
                return Some(*bound);
            }
            at -= part.bound.len();
        }
        None
    }

    fn actions(&self) -> impl Iterator<Item = &(WidgetId, KeyedAction)> {
        std::iter::once(&self.part)
            .chain(self.repeats.iter().map(|repeat| &repeat.part))
            .flat_map(|part| &part.actions)
    }
}

/// Reads `bound`'s binding from the world and writes it to its node.
fn write(world: &mut World, bound: Bound, captured: Option<WidgetId>) {
    match bound {
        Bound::Text(id, read) => {
            let text = read(world);
            world.resource_mut::<UiTree>().set_plain_text(id, &text);
        }
        Bound::Toggle(id, read) => {
            if let (Some(on), false) = (read(world).as_bool(), captured == Some(id)) {
                world.resource_mut::<UiTree>().set_toggled(id, on);
            }
        }
        Bound::Slider(id, read) => {
            if let (Some(value), false) = (read(world).as_number(), captured == Some(id)) {
                world.resource_mut::<UiTree>().set_slider_value(id, value);
            }
        }
        Bound::Tabs(rail, read) => {
            if let Some(selected) = read(world).as_number() {
                select(world.resource_mut::<UiTree>(), rail, selected);
            }
        }
        Bound::Fill(meter, color, read) => {
            let share = read(world);
            meter.set(world.resource_mut::<UiTree>(), share, color);
        }
        Bound::Visible(id, read) => {
            let shown = read(world);
            widgets::show(world.resource_mut::<UiTree>(), id, shown);
        }
        Bound::Enabled(id, read) => {
            let enabled = read(world);
            world.resource_mut::<UiTree>().set_enabled(id, enabled);
        }
    }
}

/// Shows the bar of the tab rail's tab `selected` and hides the others',
/// as [`widgets::select_tab`] does; a selection off the rail hides them
/// all.
fn select(tree: &mut UiTree, rail: WidgetId, selected: f32) {
    for at in 0..tree.children(rail).len() {
        let tab = tree.children(rail)[at];
        if let Some(&bar) = tree.children(tab).last() {
            let shown =
                [Visibility::Hidden, Visibility::Visible][usize::from(at as f32 == selected)];
            tree.set_visibility(bar, shown);
        }
    }
}

/// Rebuilds repeat `at` of the screen at `root` with `rows`: its old rows
/// removed, a node per row built from its template, keyed by the row; a
/// row whose key repeats or holds a `/` warns once and builds unkeyed and
/// actionless.
fn rebuild_rows(world: &mut World, root: WidgetId, at: usize, rows: Vec<UiRow>) {
    let Some((id, container, key, template)) = live(world, root).and_then(|live| {
        let repeat = live.repeats.get(at)?;
        Some((
            live.id.clone(),
            repeat.container,
            repeat.key.clone(),
            repeat.template.clone(),
        ))
    }) else {
        return;
    };
    let bindings = world
        .get_resource::<UiBindings>()
        .cloned()
        .unwrap_or_default();
    let tree = world.resource_mut::<UiTree>();
    for child in tree.children(container).to_vec() {
        tree.remove(child);
    }
    let mut builder = Builder::new(&bindings, &id);
    let mut seen = HashSet::new();
    for row in &rows {
        let keyed = !row.key.is_empty() && !row.key.contains('/') && seen.insert(row.key.as_str());
        if !keyed {
            builder.warnings.push((
                format!("row {id} {key} {}", row.key),
                format!(
                    "ui screen '{id}': repeat '{key}' has row key '{}' twice or with a '/'; that row builds unkeyed and actionless",
                    row.key
                ),
            ));
        }
        builder.row = Some(RowScope {
            prefix: format!("{key}/{}", row.key),
            repeat: key.clone(),
            row: row.clone(),
            keyed,
        });
        let node = builder.node(tree, container, &template, true);
        if keyed {
            tree.set_key(node, &format!("{key}/{}", row.key));
        }
    }
    let Builder {
        part,
        mounts,
        warnings,
        ..
    } = builder;
    let screens = world.resource_mut::<UiScreens>();
    for (once, message) in warnings {
        screens.warn(once, &message);
    }
    if let Some(repeat) = screens
        .live
        .iter_mut()
        .find(|live| live.root == root)
        .and_then(|live| live.repeats.get_mut(at))
    {
        repeat.rows = Some(rows);
        repeat.part = part;
    }
    for (mount, node) in mounts {
        mount(world, node);
    }
}

/// The action of `widget`, a control of the screen built at `root`, as its
/// file's `on` gives it; `None` for a control with none.
#[must_use]
pub fn action(world: &World, root: WidgetId, widget: WidgetId) -> Option<KeyedAction> {
    let live = live(world, root)?;
    live.actions()
        .find(|(id, _)| *id == widget)
        .map(|(_, action)| action.clone())
}

/// Every control of the screen built at `root` with an action, in build
/// order, the repeats' rows after the body; empty for a root no screen was
/// built at.
#[must_use]
pub fn actions(world: &World, root: WidgetId) -> Vec<(WidgetId, KeyedAction)> {
    live(world, root).map_or_else(Vec::new, |live| live.actions().cloned().collect())
}

/// The revision of the screen built at `root`: 0 when built, raised by
/// each reload of its file. A node read by key is read again after a
/// revision rises, since a reload rebuilds the nodes under new IDs.
#[must_use]
pub fn revision(world: &World, root: WidgetId) -> Option<u32> {
    live(world, root).map(|live| live.revision)
}

/// The first node under `root` keyed `key`, in tree order.
#[must_use]
pub fn node(world: &World, root: WidgetId, key: &str) -> Option<WidgetId> {
    world
        .get_resource::<UiTree>()?
        .find_key(root, UiKey::new(key))
}

/// The repeat row a build is inside: its key prefix, its repeat's key, the
/// row, and whether it builds keyed.
struct RowScope {
    prefix: String,
    repeat: String,
    row: UiRow,
    keyed: bool,
}

/// Builds a file's nodes, recording what refreshes and actions need.
struct Builder<'a> {
    bindings: &'a UiBindings,
    id: &'a str,
    part: Part,
    repeats: Vec<Repeat>,
    mounts: Vec<(MountFn, WidgetId)>,
    warnings: Vec<(String, String)>,
    row: Option<RowScope>,
}

impl<'a> Builder<'a> {
    fn new(bindings: &'a UiBindings, id: &'a str) -> Self {
        Self {
            bindings,
            id,
            part: Part::default(),
            repeats: Vec::new(),
            mounts: Vec::new(),
            warnings: Vec::new(),
            row: None,
        }
    }

    /// The root node: a `screen`'s scrim and card, or a bare root.
    fn root(&mut self, tree: &mut UiTree, node: &Node) -> BuiltScreen {
        let (root, card) = if node.kind == Kind::Screen {
            let width = node.width.as_ref().map_or(0.0, |width| length(tree, width));
            let (root, card) = widgets::screen(tree, width);
            if let Some(file) = &node.card {
                if let Some(layout) = &file.layout {
                    apply_layout(tree, card, layout);
                }
                if let Some(overrides) = &file.overrides {
                    apply_overrides(tree, card, overrides);
                }
            }
            (root, card)
        } else {
            let layer = match node.layer {
                Some(LayerName::Game) => UiLayer::Game,
                _ => UiLayer::GamePopup,
            };
            let root = tree.add_root(layer, LayoutStyle::default());
            if node.modal {
                tree.set_modal(root, true);
            }
            (root, root)
        };
        self.common(tree, root, node, false);
        for child in &node.children {
            self.node(tree, card, child, false);
        }
        for child in &node.over {
            self.node(tree, root, child, false);
        }
        BuiltScreen { root, card }
    }

    /// Builds `node` under `parent` and its children; `row_node` when it is
    /// a repeat row's own node.
    fn node(
        &mut self,
        tree: &mut UiTree,
        parent: WidgetId,
        node: &Node,
        row_node: bool,
    ) -> WidgetId {
        let text = self.text(node);
        let caption = node.caption.as_deref().unwrap_or_default();
        let id = match node.kind {
            Kind::Row => widgets::row(tree, parent),
            Kind::Column => widgets::column(tree, parent),
            Kind::Panel => tree.panel(parent, LayoutStyle::default()),
            Kind::Label => tree.label(parent, text.as_str()),
            Kind::Heading => widgets::heading(tree, parent, &text, node.centred),
            Kind::Display => {
                let style = node.style.as_deref().unwrap_or_default();
                widgets::display(tree, parent, &text, style)
            }
            Kind::Note => widgets::note(tree, parent, &text),
            Kind::Small => widgets::small(tree, parent, &text),
            Kind::Prompt => {
                let template = node.template.as_deref().unwrap_or_default();
                widgets::prompt_row(tree, parent, template)
            }
            Kind::Button => widgets::button(tree, parent, &text),
            Kind::ChoiceRow => widgets::choice_row(tree, parent, caption, &text),
            Kind::ToggleRow => widgets::toggle_row(tree, parent, caption, false),
            Kind::SliderRow => {
                let range = node.range.map_or(SliderRange::new(0.0, 1.0), |range| {
                    SliderRange::new(range.min, range.max).with_step(range.step)
                });
                let (slider, readout) = widgets::slider_row(tree, parent, caption, range);
                let bindings = self.bindings;
                let readout_of = node.readout.as_deref();
                if let Some(read) =
                    readout_of.and_then(|name| self.find(name, "text binding", &bindings.text))
                {
                    self.part.bound.push(Bound::Text(readout, read));
                }
                slider
            }
            Kind::TabRail => {
                let captions: Vec<&str> =
                    node.tabs.iter().map(|tab| tab.caption.as_str()).collect();
                let (tabs, _) = widgets::tab_rail(tree, parent, &captions);
                for (tab, file) in tabs.iter().zip(&node.tabs) {
                    self.key(tree, *tab, &file.key);
                }
                tabs.first()
                    .and_then(|tab| tree.parent(*tab))
                    .unwrap_or(parent)
            }
            Kind::Meter => {
                let size =
                    |side: &Option<Length>| side.as_ref().map_or(0.0, |each| length(tree, each));
                let (width, height) = (size(&node.width), size(&node.height));
                let accent = tree.theme().tokens.colors.accent;
                let color = node
                    .color
                    .as_ref()
                    .map_or(accent, |color| color_of(tree, color));
                let meter = widgets::meter(tree, parent, width, height);
                let bindings = self.bindings;
                let fill_of = node.bind.fill.as_deref();
                if let Some(read) =
                    fill_of.and_then(|name| self.find(name, "fill binding", &bindings.fill))
                {
                    self.part.bound.push(Bound::Fill(meter, color, read));
                }
                meter.track
            }
            Kind::Group | Kind::Mount | Kind::Repeat | Kind::Screen | Kind::Root => {
                widgets::group(tree, parent, LayoutStyle::default(), Align::Stretch)
            }
        };
        self.common(tree, id, node, row_node);
        for child in &node.children {
            self.node(tree, id, child, false);
        }
        match node.kind {
            Kind::Mount => {
                let name = node.mount.as_deref().unwrap_or_default();
                let bindings = self.bindings;
                if let Some(mount) = self.find(name, "mount", &bindings.mounts) {
                    self.mounts.push((mount, id));
                }
            }
            Kind::Repeat => {
                let name = node.rows.as_deref().unwrap_or_default();
                let bindings = self.bindings;
                let rows_of = self.find(name, "row provider", &bindings.rows);
                if let Some(template) = &node.row {
                    self.repeats.push(Repeat {
                        container: id,
                        key: node.key.clone().unwrap_or_default(),
                        rows_of,
                        template: (**template).clone(),
                        rows: None,
                        part: Part::default(),
                    });
                }
            }
            _ => {}
        }
        id
    }

    /// The text a node is built with: its literal, its row's field, or
    /// empty until its binding's first refresh.
    fn text(&mut self, node: &Node) -> String {
        if let Some(text) = &node.text {
            return text.clone();
        }
        let (Some(field), Some(scope)) = (&node.field, &self.row) else {
            return String::new();
        };
        if let Some(value) = scope.row.fields.get(field) {
            return value.clone();
        }
        let (id, key) = (self.id, &scope.row.key);
        self.warnings.push((
            format!("field {id} {field}"),
            format!("ui screen '{id}': row '{key}' has no field '{field}'; it shows empty"),
        ));
        String::new()
    }

    /// What every kind takes: a key, a layout, overrides, a text style, the
    /// mono and danger looks, the bindings and an action.
    fn common(&mut self, tree: &mut UiTree, id: WidgetId, node: &Node, row_node: bool) {
        if let Some(key) = node.key.as_deref().filter(|_| !row_node) {
            self.key(tree, id, key);
        }
        if let Some(layout) = &node.layout {
            apply_layout(tree, id, layout);
        }
        if let Some(overrides) = &node.overrides {
            apply_overrides(tree, id, overrides);
        }
        if let Some(style) = node.style.as_deref().filter(|_| node.kind != Kind::Display) {
            tree.set_text_style(id, Some(style));
        }
        if node.mono {
            widgets::mono(tree, id);
        }
        if node.danger {
            widgets::danger(tree, id);
        }
        let (bind, bindings) = (&node.bind, self.bindings);
        if let Some(read) = bind
            .text
            .as_deref()
            .and_then(|name| self.find(name, "text binding", &bindings.text))
        {
            self.part.bound.push(Bound::Text(id, read));
        }
        if let Some(read) = bind
            .value
            .as_deref()
            .and_then(|name| self.find(name, "value binding", &bindings.value))
        {
            self.part.bound.push(match node.kind {
                Kind::ToggleRow => Bound::Toggle(id, read),
                Kind::SliderRow => Bound::Slider(id, read),
                _ => Bound::Tabs(id, read),
            });
        }
        if let Some(read) = bind
            .visible
            .as_deref()
            .and_then(|name| self.find(name, "visible binding", &bindings.visible))
        {
            self.part.bound.push(Bound::Visible(id, read));
        }
        if let Some(read) = bind
            .enabled
            .as_deref()
            .and_then(|name| self.find(name, "enabled binding", &bindings.enabled))
        {
            self.part.bound.push(Bound::Enabled(id, read));
        }
        if let Some(action) = &node.on {
            let (key, row) = match &self.row {
                Some(scope) if !scope.keyed => return,
                Some(scope) if row_node => {
                    (Some(scope.repeat.clone()), Some(scope.row.key.clone()))
                }
                Some(scope) => (node.key.clone(), Some(scope.row.key.clone())),
                None => (node.key.clone(), None),
            };
            let on_commit = matches!(node.kind, Kind::ToggleRow | Kind::SliderRow);
            self.part.actions.push((
                id,
                KeyedAction {
                    action: action.clone(),
                    key,
                    row,
                    on_commit,
                    value_bound: node.bind.value.is_some(),
                },
            ));
        }
    }

    /// Keys `id` by `key`, qualified inside a repeat's row, and not at all
    /// in an unkeyed row.
    fn key(&self, tree: &mut UiTree, id: WidgetId, key: &str) {
        match &self.row {
            Some(scope) if scope.keyed => tree.set_key(id, &format!("{}/{key}", scope.prefix)),
            Some(_) => {}
            None => tree.set_key(id, key),
        }
    }

    /// The binding `name` of `kind` in `map`; a name not there warns once.
    fn find<T: Copy>(
        &mut self,
        name: &str,
        kind: &str,
        map: &HashMap<&'static str, T>,
    ) -> Option<T> {
        let found = map.get(name).copied();
        if found.is_none() {
            let id = self.id;
            self.warnings.push((
                format!("{kind} {id} {name}"),
                format!("ui screen '{id}': no {kind} '{name}' is registered; it is skipped"),
            ));
        }
        found
    }
}

/// A length in UI units: its number, the theme's size, or a spacing token.
fn length(tree: &UiTree, length: &Length) -> f32 {
    let spacing = tree.theme().tokens.spacing;
    match length {
        Length::Units(units) => *units,
        Length::Size(name) => widgets::size(tree, name),
        Length::Spacing(Spacing::Xs) => spacing.xs,
        Length::Spacing(Spacing::Sm) => spacing.sm,
        Length::Spacing(Spacing::Md) => spacing.md,
        Length::Spacing(Spacing::Lg) => spacing.lg,
        Length::Spacing(Spacing::Xl) => spacing.xl,
    }
}

/// A colour: its literal, or its token's in the tree's theme.
fn color_of(tree: &UiTree, color: &Color) -> [u8; 4] {
    match color {
        Color::Rgba(rgba) => *rgba,
        Color::Token(name) => tree.theme().tokens.colors.named(name).unwrap_or_default(),
    }
}

/// Sets `layout`'s fields over `id`'s layout, the others kept.
fn apply_layout(tree: &mut UiTree, id: WidgetId, layout: &LayoutFile) {
    let Some(mut style) = tree.layout_style(id).cloned() else {
        return;
    };
    let sizing = |sizing: &SizingFile| match sizing {
        SizingFile::Auto => Sizing::Auto,
        SizingFile::Fill => Sizing::Fill,
        SizingFile::Percent(percent) => Sizing::Percent(*percent),
        SizingFile::Length(each) => Sizing::Px(length(tree, each)),
    };
    let limit = |limit: &LimitFile| match limit {
        LimitFile::None => None,
        LimitFile::Length(each) => Some(length(tree, each)),
    };
    if let Some(direction) = layout.direction {
        style.direction = match direction {
            DirectionName::Row => Direction::Row,
            DirectionName::Column => Direction::Column,
        };
    }
    for (field, slot) in [
        (&layout.width, &mut style.width),
        (&layout.height, &mut style.height),
    ] {
        if let Some(each) = field {
            *slot = sizing(each);
        }
    }
    for (field, slot) in [
        (&layout.min_width, &mut style.min_width),
        (&layout.min_height, &mut style.min_height),
        (&layout.max_width, &mut style.max_width),
        (&layout.max_height, &mut style.max_height),
    ] {
        if let Some(each) = field {
            *slot = limit(each);
        }
    }
    for (field, slot) in [
        (&layout.padding, &mut style.padding),
        (&layout.margin, &mut style.margin),
    ] {
        if let Some(each) = field {
            *slot = edges(tree, each, *slot);
        }
    }
    if let Some(gap) = &layout.gap {
        style.gap = length(tree, gap);
    }
    if let Some(align) = layout.align_items {
        style.align_items = align_of(align);
    }
    if let Some(align) = layout.align_self {
        style.align_self = Some(align_of(align));
    }
    if let Some(justify) = layout.justify_content {
        style.justify_content = match justify {
            JustifyName::Start => Justify::Start,
            JustifyName::Center => Justify::Center,
            JustifyName::End => Justify::End,
            JustifyName::SpaceBetween => Justify::SpaceBetween,
            JustifyName::SpaceAround => Justify::SpaceAround,
            JustifyName::SpaceEvenly => Justify::SpaceEvenly,
        };
    }
    if let Some(anchor) = &layout.anchor {
        let offset = anchor.offset.as_ref().map_or(Vec2::ZERO, |[x, y]| {
            Vec2::new(length(tree, x), length(tree, y))
        });
        let anchor = match anchor.corner {
            CornerName::TopStart => Anchor::TopStart,
            CornerName::TopCenter => Anchor::TopCenter,
            CornerName::TopEnd => Anchor::TopEnd,
            CornerName::CenterStart => Anchor::CenterStart,
            CornerName::Center => Anchor::Center,
            CornerName::CenterEnd => Anchor::CenterEnd,
            CornerName::BottomStart => Anchor::BottomStart,
            CornerName::BottomCenter => Anchor::BottomCenter,
            CornerName::BottomEnd => Anchor::BottomEnd,
        };
        style.placement = Placement::Anchored { anchor, offset };
    }
    tree.set_layout_style(id, style);
}

fn align_of(align: AlignName) -> Align {
    match align {
        AlignName::Start => Align::Start,
        AlignName::Center => Align::Center,
        AlignName::End => Align::End,
        AlignName::Stretch => Align::Stretch,
    }
}

/// `file`'s edges over `base`: one length for every side, or the sides it
/// names, `x` and `y` first.
fn edges(tree: &UiTree, file: &EdgesFile, base: Edges) -> Edges {
    let sides = match file {
        EdgesFile::All(each) => return Edges::all(length(tree, each)),
        EdgesFile::Sides(sides) => sides,
    };
    let value = |side: &Option<Length>| side.as_ref().map(|each| length(tree, each));
    let mut edges = base;
    if let Some(x) = value(&sides.x) {
        (edges.start, edges.end) = (x, x);
    }
    if let Some(y) = value(&sides.y) {
        (edges.top, edges.bottom) = (y, y);
    }
    for (side, slot) in [
        (&sides.top, &mut edges.top),
        (&sides.right, &mut edges.end),
        (&sides.bottom, &mut edges.bottom),
        (&sides.left, &mut edges.start),
    ] {
        if let Some(each) = value(side) {
            *slot = each;
        }
    }
    edges
}

/// Sets `file`'s overrides over `id`'s own, the others kept, its colour
/// tokens resolved against the tree's theme.
fn apply_overrides(tree: &mut UiTree, id: WidgetId, file: &OverridesFile) {
    let tokens = tree.theme().tokens.colors;
    let mut file = file.clone();
    // Token names were checked when the file was read, so these resolve.
    let _ = resolve(&mut file.background, &tokens, &String::new);
    let _ = resolve(&mut file.border, &tokens, &String::new);
    let _ = file.text.resolve(&tokens, &|_| String::new());
    tree.edit_overrides(id, |own| file.apply_overrides(own));
}

#[cfg(test)]
#[path = "../tests/ui/screen_build.rs"]
mod tests;
