//! Damage types: the `damage_types` data kind a game declares its types
//! in, the World's table of their bits, the kinds a hit part carries, and
//! the penetration and conversion a hit or its source gives.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tungsten_core::{DataRegistry, Inspectable, MissingAsset, SpawnContext, World};

use crate::damage::ailments::{AILMENT_RUNG, AilmentRung};
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, SHMUP, TOP_DOWN, describe,
    describe_kind, describe_shape, round_trip,
};
use crate::foundation::fraction;
use crate::foundation::inspect::register_rows;
use crate::foundation::tags::{Tags, check_declared, valid_tag};
use crate::present::labels::LabelStyle;

/// The data kind a game declares its damage types in.
pub(crate) const DAMAGE_TYPES: &str = "damage_types";
/// The scene components the health plugin registers for a hit's source.
const PENETRATION: &str = "penetration";
const CONVERSION: &str = "conversion";
/// The shape of a `damage_types` entry's definitions, and of a conversion
/// rule.
const DAMAGE_TYPE: &str = "damage_type";
pub(crate) const CONVERSION_RULE: &str = "conversion_rule";

/// The most damage types a World holds: a bit each in [`DamageKinds`].
const MAX_TYPES: usize = 64;

/// The kind a capacity failure reports.
const FULL: &str = "damage type (a World holds 64)";

/// Whether the tag `declared` lies under `name`: `elemental.fire` under
/// `elemental`.
fn under(declared: &str, name: &str) -> bool {
    declared.len() > name.len()
        && declared.starts_with(name)
        && declared.as_bytes()[name.len()] == b'.'
}

/// `name` checked as a tag name, for a data form's error.
fn tag_name(field: &str, name: &str) -> Result<(), String> {
    if valid_tag(name) {
        Ok(())
    } else {
        Err(format!(
            "{field}`{name}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
        ))
    }
}

/// One damage type's definition in a `damage_types` entry, every field
/// optional and inherited down the dotted tree: a type's value is its own
/// entry's, else its nearest declared ancestor's that sets it.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DamageTypeDef {
    /// How a `damaged_part` cue's style-less `label` floats a part of the
    /// type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label_style: Option<LabelStyle>,
    /// The ladder a part of the type climbs, from the lowest rung up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ailments: Option<Vec<AilmentRung>>,
}

/// A `damage_types` entry: definitions by damage type, in name order, each
/// type a tag name, which the load checks is declared.
#[derive(Debug, Deserialize)]
#[serde(try_from = "BTreeMap<String, DamageTypeDef>")]
pub(crate) struct DamageTypes(Vec<(String, DamageTypeDef)>);

impl TryFrom<BTreeMap<String, DamageTypeDef>> for DamageTypes {
    type Error = String;

    fn try_from(types: BTreeMap<String, DamageTypeDef>) -> Result<Self, Self::Error> {
        for name in types.keys() {
            tag_name("", name)?;
        }
        Ok(Self(types.into_iter().collect()))
    }
}

impl DamageTypes {
    /// The types it defines, by name.
    fn names(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(name, _)| name.as_str())
    }

    /// `name`'s definition, when it defines `name`.
    fn get(&self, name: &str) -> Option<&DamageTypeDef> {
        self.0
            .binary_search_by(|(known, _)| known.as_str().cmp(name))
            .ok()
            .map(|at| &self.0[at].1)
    }

    fn defines(&self, name: &str) -> bool {
        self.get(name).is_some()
    }
}

/// The `damage_types` entries the World holds, in ID order, with their IDs.
fn entries(world: &World) -> impl Iterator<Item = (&str, &DamageTypes)> {
    world
        .get_resource::<DataRegistry>()
        .into_iter()
        .flat_map(|data| {
            data.ids(DAMAGE_TYPES)
                .filter_map(move |id| data.get::<DamageTypes>(id).map(|types| (id, types)))
        })
}

/// The `damage_types` kind's check at a load's commit, every staged entry
/// in place: each type a declared tag and defined by no other entry, each
/// ladder rung's effect and tags, and room in the World's 64 bits for every
/// type the table lacks; when the types the table lacks outnumber the bits
/// left, every checked entry that holds one fails, naming its first.
fn check_types(types: &DamageTypes, world: &World) -> Result<(), MissingAsset> {
    for name in types.names() {
        check_declared(world, name, name)?;
    }
    for (name, definition) in &types.0 {
        for (i, rung) in definition.ailments.iter().flatten().enumerate() {
            rung.check(world, &format!("{name}.ailments.{i}"))?;
        }
    }
    for name in types.names() {
        let twice =
            entries(world).find(|(_, other)| !std::ptr::eq(*other, types) && other.defines(name));
        if let Some((other, _)) = twice {
            return Err(MissingAsset::new(
                name,
                "damage type defined by one entry",
                format!("{name}, also defined by {other}"),
            ));
        }
    }
    let table = world.get_resource::<DamageTypeTable>();
    let known = |name: &str| table.is_some_and(|table| table.bit(name).is_some());
    let mut new: Vec<&str> = entries(world)
        .flat_map(|(_, other)| other.names())
        .filter(|name| !known(name))
        .collect();
    new.sort_unstable();
    new.dedup();
    let left = MAX_TYPES - table.map_or(0, DamageTypeTable::len);
    match types.names().find(|name| !known(name)) {
        Some(name) if new.len() > left => Err(MissingAsset::new(name, FULL, name)),
        _ => Ok(()),
    }
}

/// Whether `name` is a declared damage type, a key of a `damage_types`
/// entry, or with `ancestor` a dotted ancestor of one; else the
/// [`MissingAsset`] a load reports, of kind `damage type`, at `field`.
pub(crate) fn check_type(
    world: &World,
    field: &str,
    name: &str,
    ancestor: bool,
) -> Result<(), MissingAsset> {
    let found = entries(world).any(|(_, types)| {
        types
            .names()
            .any(|declared| declared == name || (ancestor && under(declared, name)))
    });
    if found {
        Ok(())
    } else {
        Err(MissingAsset::new(field, "damage type", name))
    }
}

/// The World's damage types, a crate-private resource: names in bit order
/// and their bits sorted by name. Its first merge takes every entry's types,
/// sorted, as bits 0 onward; a later merge appends the types it lacks,
/// sorted among themselves; a type a reload removed keeps its bit, so a mask
/// queued before the reload keeps its meaning.
#[derive(Debug, Default)]
pub(crate) struct DamageTypeTable {
    /// By bit.
    names: Vec<String>,
    /// Each name's bit, sorted by name.
    index: Vec<u8>,
    /// The types past 64 that no check saw, each warned about once, sorted.
    overflow: Vec<String>,
}

impl DamageTypeTable {
    /// How many bits are taken.
    pub(crate) fn len(&self) -> usize {
        self.names.len()
    }

    /// The kinds of every type named `name` or under it: a rule's `kind`
    /// filter reads a name as a tag filter reads a tag.
    pub(crate) fn mask_of(&self, name: &str) -> DamageKinds {
        let mut bits = 0_u64;
        for (bit, known) in self.names.iter().enumerate() {
            if known == name || under(known, name) {
                bits |= 1 << bit;
            }
        }
        DamageKinds { bits }
    }

    /// `name`'s bit, when it has one.
    fn bit(&self, name: &str) -> Option<u8> {
        self.index
            .binary_search_by(|&bit| self.names[usize::from(bit)].as_str().cmp(name))
            .ok()
            .map(|at| self.index[at])
    }

    /// Whether a merge has seen `name`: it has a bit, or was warned about.
    fn knows(&self, name: &str) -> bool {
        self.bit(name).is_some()
            || self
                .overflow
                .binary_search_by(|known| known.as_str().cmp(name))
                .is_ok()
    }

    /// Gives `name` the next bit, or past 64 warns once.
    fn append(&mut self, name: String) {
        if self.names.len() == MAX_TYPES {
            log::warn!(
                "damage types: `{name}` gets no bit; a World holds {MAX_TYPES} damage types, so its parts carry no kind of it"
            );
            if let Err(at) = self.overflow.binary_search(&name) {
                self.overflow.insert(at, name);
            }
            return;
        }
        let bit = self.names.len() as u8;
        let at = self
            .index
            .partition_point(|&known| self.names[usize::from(known)] < name);
        self.index.insert(at, bit);
        self.names.push(name);
    }

    /// The kinds a part of `tags` carries: the bit of every type its types
    /// are or lie under, found by searching each type's dotted prefixes, so
    /// the cost follows the part's tags, not the table.
    pub(crate) fn mask(&self, tags: &Tags) -> DamageKinds {
        let mut bits = 0_u64;
        for tag in tags.iter() {
            let mut prefix = tag;
            loop {
                if let Some(bit) = self.bit(prefix) {
                    bits |= 1 << bit;
                }
                match prefix.rfind('.') {
                    Some(dot) => prefix = &prefix[..dot],
                    None => break,
                }
            }
        }
        DamageKinds { bits }
    }
}

/// Appends to the World's table each damage type the `damage_types`
/// entries hold that it lacks, sorted among themselves, inserting the table
/// when the World has none: every path that builds or reads a mask runs it
/// first, once per run or per immediate call.
pub(crate) fn merge_types(world: &mut World) {
    if !world.has_resource::<DamageTypeTable>() {
        world.insert_resource(DamageTypeTable::default());
    }
    let table = world.resource::<DamageTypeTable>();
    let mut new: Vec<String> = entries(world)
        .flat_map(|(_, types)| types.names())
        .filter(|name| !table.knows(name))
        .map(str::to_owned)
        .collect();
    if new.is_empty() {
        return;
    }
    new.sort_unstable();
    new.dedup();
    let table = world.resource_mut::<DamageTypeTable>();
    for name in new {
        table.append(name);
    }
}

/// The damage types a hit part carries, as bits of its World's type table:
/// the kinds on [`Damaged`](crate::Damaged), the hit's parts' together, and
/// on [`DamagedPart`](crate::DamagedPart). A part carries every declared
/// type its types are or lie under: a part of `elemental.fire` carries
/// `elemental` and `elemental.fire` when the `damage_types` entries declare
/// both. A part without types, or whose types no entry declares, carries
/// [`NONE`](Self::NONE). Bits mean nothing outside their World, so the
/// kinds are read through it.
///
/// A World's bits follow its types' sorted names at their first read and
/// stay for its life: a type a reload adds takes the next bit, and a type
/// it removes keeps its own, so a kind queued before the reload means what
/// it meant. A World holds 64 types; a load past them fails.
///
/// # Examples
///
/// ```
/// use tungsten_core::{EventQueue, Plugin, Schedule, StagedLoad, World};
/// use tungsten_kit::{
///     DamageKinds, DamagePart, DamageRequest, Damaged, Health, HealthPlugin, Tags, TagsPlugin,
/// };
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// TagsPlugin::new().with_tags(["elemental.fire"]).build(&mut schedule, &mut world);
/// HealthPlugin.build(&mut schedule, &mut world);
/// // A game's damage types, loaded as a manifest's `data` loads them.
/// let dir = std::env::temp_dir().join(format!("tungsten_kinds_doc_{}", std::process::id()));
/// std::fs::create_dir_all(&dir).unwrap();
/// let file = dir.join("types.json");
/// std::fs::write(&file, r#"{ "elemental": {}, "elemental.fire": {} }"#).unwrap();
/// let mut load = StagedLoad::new();
/// load.add_data(&world, "damage_types", "game", &file).unwrap();
/// load.commit(&mut world).unwrap();
/// std::fs::remove_dir_all(&dir).unwrap();
///
/// let golem = world.spawn_with((Health::new(10.0),));
/// let fire = DamagePart::new(3.0).with_tags(Tags::new(["elemental.fire"]));
/// DamageRequest::from_parts(golem, [fire, DamagePart::new(1.0)]).apply(&mut world);
/// let hit = world.resource::<EventQueue<Damaged>>().iter().next().copied().unwrap();
/// assert_eq!(hit.kinds.names(&world), ["elemental", "elemental.fire"]);
/// assert!(hit.kinds.has(&world, "elemental") && !hit.kinds.has(&world, "elemental.cold"));
/// assert!(DamageKinds::NONE.is_empty() && !hit.kinds.is_empty());
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct DamageKinds {
    bits: u64,
}

impl DamageKinds {
    /// No kind: an untyped part's.
    pub const NONE: Self = Self { bits: 0 };

    /// Whether it carries no kind.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.bits == 0
    }

    /// Whether it carries a type named `name`, or one under it: `has(world,
    /// "elemental")` holds for a part of `elemental.fire`.
    #[must_use]
    pub fn has(self, world: &World, name: &str) -> bool {
        self.each(world)
            .any(|kind| kind == name || under(kind, name))
    }

    /// The types it carries, by name.
    #[must_use]
    pub fn names(self, world: &World) -> Vec<&str> {
        let mut names: Vec<&str> = self.each(world).collect();
        names.sort_unstable();
        names
    }

    /// The style a `damaged_part` cue's style-less `label` floats a part of
    /// these kinds in, from the current `damage_types` entries: the deepest
    /// type's `label_style`, else its nearest ancestor's; `None` for no
    /// kinds or when no type of its line sets one.
    #[must_use]
    pub fn label_style(self, world: &World) -> Option<LabelStyle> {
        label_style(world, self)
    }

    /// Both kinds' types.
    pub(crate) fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    /// Whether the two carry a type in common.
    pub(crate) fn intersects(self, other: Self) -> bool {
        self.bits & other.bits != 0
    }

    /// The name of each bit it carries, in bit order.
    fn each(self, world: &World) -> impl Iterator<Item = &str> {
        let names = world
            .get_resource::<DamageTypeTable>()
            .map_or(&[][..], |table| &table.names[..]);
        let mut bits = self.bits;
        std::iter::from_fn(move || {
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if let Some(name) = names.get(bit) {
                    return Some(name.as_str());
                }
            }
            None
        })
    }
}

/// The type a part of `kinds` takes its definition from: its deepest type,
/// the first by name at one depth; `None` for an untyped part.
pub(crate) fn definition(world: &World, kinds: DamageKinds) -> Option<&str> {
    let depth = |name: &str| name.matches('.').count();
    kinds
        .each(world)
        .min_by(|a, b| depth(b).cmp(&depth(a)).then_with(|| a.cmp(b)))
}

/// The style a `damaged_part` cue's style-less `label` floats a part of
/// `kinds` in: its definition's `label_style`, else its nearest ancestor's
/// that sets one, read from the current `damage_types` entries, so a reload
/// restyles; `None` when no type of its line sets one.
pub(crate) fn label_style(world: &World, kinds: DamageKinds) -> Option<LabelStyle> {
    let mut name = definition(world, kinds)?;
    loop {
        let style = entries(world).find_map(|(_, types)| {
            types
                .get(name)
                .and_then(|definition| definition.label_style.as_ref())
        });
        if let Some(style) = style {
            return Some(style.clone());
        }
        name = &name[..name.rfind('.')?];
    }
}

/// The ladder a part of `kinds` climbs and the type it sits on, whose
/// bucket the part fills: its definition's `ailments`, else its nearest
/// ancestor's that sets one, read from the current `damage_types` entries;
/// `None` when no type of its line sets one.
pub(crate) fn ladder(world: &World, kinds: DamageKinds) -> Option<(&str, &[AilmentRung])> {
    let mut name = definition(world, kinds)?;
    loop {
        if let Some(rungs) = ladder_of(world, name) {
            return Some((name, rungs));
        }
        name = &name[..name.rfind('.')?];
    }
}

/// The ladder the type `name`'s own definition sets, when it sets one.
pub(crate) fn ladder_of<'a>(world: &'a World, name: &str) -> Option<&'a [AilmentRung]> {
    entries(world).find_map(|(_, types)| {
        types
            .get(name)
            .and_then(|definition| definition.ailments.as_deref())
    })
}

/// Points off a hit part's resistances: for each damage type, points in
/// `[0, 1]` that each matching [`Resist`](crate::Resist) factor in `(0, 1)`
/// gains, capped at 1, so penetration never pierces an immunity and never
/// raises a factor above 1. A part's points are the matching entries of the
/// hit's own penetration and of its source's, summed and capped at 1; an
/// entry matches a part whose types carry its type, exactly or as a dotted
/// ancestor. A [`Hitbox`](crate::Hitbox)'s or a
/// [`ContactDamage`](crate::ContactDamage)'s `pen` is the hit's own; this
/// component on the entity a hit comes from, its spawn chain's origin for a
/// spawned bolt, is its source's.
///
/// Scene name `penetration`: `{ "elemental.fire": 0.5 }`, each key a
/// declared damage type or an ancestor of one, each value in `[0, 1]`. It is
/// data: it serializes as that object.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{Penetration, Tags};
///
/// let pen = Penetration::new().with_points("elemental", 0.25).with_points("elemental.fire", 0.5);
/// assert_eq!(pen.points(&Tags::new(["elemental.fire"])), 0.75, "a type and its ancestor");
/// assert_eq!(pen.points(&Tags::new(["elemental.cold"])), 0.25);
/// assert_eq!(pen.points(&Tags::new(["physical"])), 0.0);
/// let deep = pen.with_points("elemental.fire", 1.0);
/// assert_eq!(deep.points(&Tags::new(["elemental.fire"])), 1.0, "capped at 1");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PointsSave", into = "PointsSave")]
pub struct Penetration {
    /// Sorted by type.
    points: Vec<(String, f32)>,
}

/// Penetration as data writes it: points by damage type.
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct PointsSave(BTreeMap<String, f32>);

/// `tag` a tag name and `points` in `[0, 1]`.
fn check_points(tag: &str, points: f32) -> Result<(), String> {
    tag_name("", tag)?;
    if (0.0..=1.0).contains(&points) {
        Ok(())
    } else {
        Err(format!("{tag}: points must be in [0, 1] (got {points})"))
    }
}

impl TryFrom<PointsSave> for Penetration {
    type Error = String;

    fn try_from(save: PointsSave) -> Result<Self, Self::Error> {
        for (tag, &points) in &save.0 {
            check_points(tag, points)?;
        }
        Ok(Self {
            points: save.0.into_iter().collect(),
        })
    }
}

impl From<Penetration> for PointsSave {
    fn from(penetration: Penetration) -> Self {
        Self(penetration.points.into_iter().collect())
    }
}

impl Penetration {
    /// No points.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The penetration with `points` against damage of type `tag`, added or
    /// replaced.
    ///
    /// # Panics
    ///
    /// When `tag` is not a tag name, or `points` is not in `[0, 1]`.
    #[must_use]
    #[track_caller]
    pub fn with_points(mut self, tag: &str, points: f32) -> Self {
        if let Err(error) = check_points(tag, points) {
            panic!("Penetration::with_points: {error}");
        }
        match self
            .points
            .binary_search_by(|(known, _)| known.as_str().cmp(tag))
        {
            Ok(at) => self.points[at].1 = points,
            Err(at) => self.points.insert(at, (tag.to_owned(), points)),
        }
        self
    }

    /// The points a part of `tags` takes from it: the matching entries
    /// summed, capped at 1.
    #[must_use]
    pub fn points(&self, tags: &Tags) -> f32 {
        self.sum(tags).min(1.0)
    }

    /// The matching entries summed, uncapped, to add to another's.
    pub(crate) fn sum(&self, tags: &Tags) -> f32 {
        self.points
            .iter()
            .filter(|(tag, _)| tags.has(tag))
            .map(|&(_, points)| points)
            .sum()
    }
}

/// A penetration as a scene writes it, `pen` or `penetration`: points by
/// damage type, each checked, in name order.
#[derive(Default, Serialize, Deserialize)]
#[serde(try_from = "BTreeMap<String, f32>")]
pub(crate) struct PenetrationDef(BTreeMap<String, f32>);

impl TryFrom<BTreeMap<String, f32>> for PenetrationDef {
    type Error = String;

    fn try_from(points: BTreeMap<String, f32>) -> Result<Self, Self::Error> {
        for (tag, &value) in &points {
            check_points(tag, value)?;
        }
        Ok(Self(points))
    }
}

impl PenetrationDef {
    /// The load's check: each key a declared damage type or an ancestor of
    /// one, named by its field after `prefix` (`pen.`).
    pub(crate) fn check(&self, world: &World, prefix: &str) -> Result<(), MissingAsset> {
        for tag in self.0.keys() {
            check_type(world, &format!("{prefix}{tag}"), tag, true)?;
        }
        Ok(())
    }

    /// Whether it gives no points.
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<PenetrationDef> for Penetration {
    fn from(def: PenetrationDef) -> Self {
        Self {
            points: def.0.into_iter().collect(),
        }
    }
}

/// Shares of a hit part converted to other damage types before penetration
/// and resistances: each rule takes `pct` of every part whose types carry
/// its `from`, exactly or as a dotted ancestor, as a new part of its `to`
/// alone. The hit's own rules, then its source's, apply in one pass, so a
/// share is never converted again; a part's matching `pct`s past 1
/// normalize to sum to 1, and what they leave stays the part's. Parts that
/// end with equal types merge, in first-appearance order. A
/// [`Hitbox`](crate::Hitbox)'s or a [`ContactDamage`](crate::ContactDamage)'s
/// `convert` is the hit's own; this component on the entity a hit comes
/// from, its spawn chain's origin for a spawned bolt, is its source's.
///
/// Scene name `conversion`:
/// `[{ "from": "physical.slash", "to": "elemental.cold", "pct": 0.5 }]`,
/// `to` a declared damage type, `from` one or an ancestor of one, `pct` in
/// `[0, 1]`. It is data: it serializes as that list.
///
/// # Examples
///
/// ```
/// use tungsten_kit::Conversion;
///
/// let frost = Conversion::new().with_rule("physical.slash", "elemental.cold", 0.5);
/// let saved = serde_json::to_value(&frost).unwrap();
/// assert_eq!(
///     saved,
///     serde_json::json!([{ "from": "physical.slash", "to": "elemental.cold", "pct": 0.5 }])
/// );
/// assert_eq!(serde_json::from_value::<Conversion>(saved).unwrap(), frost);
/// let same = r#"[{ "from": "fire", "to": "fire", "pct": 1 }]"#;
/// assert!(serde_json::from_str::<Conversion>(same).is_err());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "ConversionDef", into = "ConversionDef")]
pub struct Conversion {
    /// In order.
    rules: Vec<ConversionRule>,
}

/// One conversion rule: its source type, its target type as the one tag a
/// share carries, and its share.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ConversionRule {
    from: String,
    to: Tags,
    pct: f32,
}

impl ConversionRule {
    /// The type a part carries for the rule to take from it.
    pub(crate) fn from(&self) -> &str {
        &self.from
    }

    /// A share's types: the rule's `to` alone.
    pub(crate) fn to(&self) -> &Tags {
        &self.to
    }

    /// Its share of a part, in `[0, 1]`.
    pub(crate) fn pct(&self) -> f32 {
        self.pct
    }

    fn to_name(&self) -> &str {
        self.to.iter().next().unwrap_or_default()
    }
}

/// `from` and `to` tag names that differ, `pct` in `[0, 1]`.
fn check_rule(from: &str, to: &str, pct: f32) -> Result<(), String> {
    tag_name("from: ", from)?;
    tag_name("to: ", to)?;
    if from == to {
        return Err(format!("to: must differ from `from` (both `{to}`)"));
    }
    if !(0.0..=1.0).contains(&pct) {
        return Err(format!("pct: must be in [0, 1] (got {pct})"));
    }
    Ok(())
}

impl Conversion {
    /// No rules.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The conversion with a rule after its others: `pct` of each part of
    /// type `from` becomes a part of type `to`.
    ///
    /// # Panics
    ///
    /// When `from` or `to` is not a tag name, they are equal, or `pct` is
    /// not in `[0, 1]`.
    #[must_use]
    #[track_caller]
    pub fn with_rule(mut self, from: &str, to: &str, pct: f32) -> Self {
        if let Err(error) = check_rule(from, to, pct) {
            panic!("Conversion::with_rule: {error}");
        }
        self.rules.push(ConversionRule {
            from: from.to_owned(),
            to: Tags::new([to]),
            pct,
        });
        self
    }

    /// Its rules, in order.
    pub(crate) fn rules(&self) -> &[ConversionRule] {
        &self.rules
    }
}

/// A conversion rule as data writes it, checked.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "ConversionRuleFields")]
pub(crate) struct ConversionRuleDef {
    from: String,
    to: String,
    pct: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversionRuleFields {
    from: String,
    to: String,
    #[serde(deserialize_with = "fraction")]
    pct: f32,
}

impl TryFrom<ConversionRuleFields> for ConversionRuleDef {
    type Error = String;

    fn try_from(fields: ConversionRuleFields) -> Result<Self, Self::Error> {
        check_rule(&fields.from, &fields.to, fields.pct)?;
        Ok(Self {
            from: fields.from,
            to: fields.to,
            pct: fields.pct,
        })
    }
}

/// A conversion as data writes it, `convert` or `conversion`: its rules in
/// order.
#[derive(Default, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct ConversionDef(Vec<ConversionRuleDef>);

impl ConversionDef {
    /// The load's check: each `to` a declared damage type and each `from`
    /// one or an ancestor of one, named by its field after `prefix`
    /// (`convert.`).
    pub(crate) fn check(&self, world: &World, prefix: &str) -> Result<(), MissingAsset> {
        for (i, rule) in self.0.iter().enumerate() {
            check_type(world, &format!("{prefix}{i}.from"), &rule.from, true)?;
            check_type(world, &format!("{prefix}{i}.to"), &rule.to, false)?;
        }
        Ok(())
    }

    /// Whether it holds no rule.
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<ConversionDef> for Conversion {
    fn from(def: ConversionDef) -> Self {
        Self {
            rules: def
                .0
                .into_iter()
                .map(|rule| ConversionRule {
                    from: rule.from,
                    to: Tags::new([rule.to]),
                    pct: rule.pct,
                })
                .collect(),
        }
    }
}

impl From<Conversion> for ConversionDef {
    fn from(conversion: Conversion) -> Self {
        Self(
            conversion
                .rules
                .iter()
                .map(|rule| ConversionRuleDef {
                    from: rule.from.clone(),
                    to: rule.to_name().to_owned(),
                    pct: rule.pct,
                })
                .collect(),
        )
    }
}

/// Registers the `damage_types` kind and its table, and the `penetration`
/// and `conversion` scene components, with their rows and descriptions:
/// [`HealthPlugin`](crate::HealthPlugin)'s part of the build for the types.
pub(crate) fn register_types(world: &mut World) {
    register_rows::<Penetration>(world, "Penetration");
    register_rows::<Conversion>(world, "Conversion");
    describe_types(world);
    if !world.has_resource::<DamageTypeTable>() {
        world.insert_resource(DamageTypeTable::default());
    }
    world.register_data_kind_with_assets(DAMAGE_TYPES, check_types);
    world.register_component_with_assets(
        PENETRATION,
        |def: &PenetrationDef, world: &World| def.check(world, ""),
        |def: PenetrationDef, _: &SpawnContext<'_>| (Penetration::from(def),),
    );
    world.register_component_with_assets(
        CONVERSION,
        |def: &ConversionDef, world: &World| def.check(world, ""),
        |def: ConversionDef, _: &SpawnContext<'_>| (Conversion::from(def),),
    );
}

/// The descriptions of `penetration`, `conversion`, their shapes and the
/// `damage_types` kind.
fn describe_types(world: &mut World) {
    describe(
        world,
        ComponentDescription::new(
            PENETRATION,
            "Points off a hit part's resistances by damage type, which every hit from the entity takes after the hit's own.",
        )
        .field(FieldDescription::new(
            "points",
            FieldType::Object("damage type → points in [0, 1]"),
            "the value itself: each a declared damage type or an ancestor of one, off each matching factor in (0, 1), capped at 1",
        ))
        .genre(TOP_DOWN)
        .genre(SHMUP)
        .sample(r#"{ "damage.fire": 0.5 }"#)
        .roundtrip(round_trip::<PenetrationDef>),
    );
    describe(
        world,
        ComponentDescription::new(
            CONVERSION,
            "Shares of a hit part converted to other damage types before penetration and resistances, after the hit's own rules.",
        )
        .field(FieldDescription::new(
            "rules",
            FieldType::List(Box::new(FieldType::Shape(CONVERSION_RULE))),
            "the value itself: the rules in order, in one pass with the hit's own, a share never converted again",
        ))
        .genre(TOP_DOWN)
        .genre(SHMUP)
        .sample(r#"[{ "from": "damage", "to": "damage.fire", "pct": 0.5 }]"#)
        .roundtrip(round_trip::<ConversionDef>),
    );
    describe_shape(
        world,
        ComponentDescription::new(
            CONVERSION_RULE,
            "A share of each part of one damage type converted to another; one part's shares past 1 normalize.",
        )
        .field(FieldDescription::new(
            "from",
            FieldType::Tag,
            "the type a part carries, exactly or as a dotted ancestor: a declared damage type or an ancestor of one",
        ))
        .field(FieldDescription::new(
            "to",
            FieldType::Tag,
            "the share's one type: a declared damage type other than `from`",
        ))
        .field(FieldDescription::new(
            "pct",
            FieldType::F32 {
                min: Bound::Inclusive(0.0),
                max: Bound::Inclusive(1.0),
            },
            "the part's share converted",
        ))
        .sample(r#"{ "from": "damage", "to": "damage.fire", "pct": 0.5 }"#)
        .roundtrip(round_trip::<ConversionRuleDef>),
    );
    describe_shape(
        world,
        ComponentDescription::new(
            DAMAGE_TYPE,
            "A game's damage type: each field inherited from its nearest declared ancestor that sets it.",
        )
        .field(
            FieldDescription::new(
                "label_style",
                FieldType::Shape("label_style"),
                "how a `damaged_part` cue's `{ \"label\": {} }` floats a part of the type",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "ailments",
                FieldType::List(Box::new(FieldType::Shape(AILMENT_RUNG))),
                "the ladder a part of the type climbs, from the lowest rung up; the feed takes the highest rung the target reaches and empties its bucket",
            )
            .default("null"),
        )
        .sample("{}")
        .roundtrip(round_trip::<DamageTypeDef>),
    );
    describe_kind(
        world,
        DAMAGE_TYPES,
        "An object of `damage_type` by damage type: each key a declared tag, defined by one entry; a World holds 64.",
    );
}

impl Inspectable for Penetration {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let points: Vec<String> = self
            .points
            .iter()
            .map(|(tag, points)| format!("{tag} {points}"))
            .collect();
        vec![("penetration", points.join(", "))]
    }
}

impl Inspectable for Conversion {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let rules: Vec<String> = self
            .rules
            .iter()
            .map(|rule| format!("{} → {} × {}", rule.from, rule.to_name(), rule.pct))
            .collect();
        vec![("conversion", rules.join(", "))]
    }
}

#[cfg(test)]
#[path = "../tests/kinds.rs"]
mod tests;
