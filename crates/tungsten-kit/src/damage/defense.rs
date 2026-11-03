//! Defences between a damage request and health: resistances by damage
//! tag with modifiers, and a shield that takes damage first and recharges
//! after a delay.

use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tungsten_core::{Inspectable, Time, World};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, SHMUP, TOP_DOWN, round_trip,
};
use crate::foundation::lines::{Line, Reach, Read};
use crate::foundation::stats::{Modifier, modified};
use crate::foundation::tags::{Tags, valid_tag};

/// [`HealthPlugin`](crate::HealthPlugin)'s system that recharges shields, in
/// the tick phase.
pub const SHIELD_TICK: &str = "shield_tick";

/// The scene components the health plugin registers for the defences.
pub(crate) const RESIST: &str = "resist";
pub(crate) const SHIELD: &str = "shield";

/// The largest resistance factor.
const MAX_FACTOR: f32 = 1000.0;

/// An entity's resistances: a factor per damage tag, which multiplies each
/// part of a [`DamageRequest`](crate::DamageRequest) whose types carry that
/// tag, exactly or as a dotted ancestor: a factor for `damage` applies to a
/// `damage.fire` part too. A base factor of 0 is immunity, whatever the
/// modifiers: the part drops before any product. The matching factors
/// multiply in `f64` and the amount they make is clamped to `f32::MAX`, so
/// every amount stays finite; a factor in `(0, 1)` first gains the part's
/// [`Penetration`](crate::Penetration) points, capped at 1.
///
/// A factor takes modifiers by source, as a stat does
/// ([`add_modifier`](Self::add_modifier)): its value is `((base + Σadd) ×
/// (1 + Σpct) × Πmul)`, replaced by the last-added override, in `f64`,
/// clamped to `[0, 1000]`, the base 1 for a tag listed by a modifier only.
/// A curse that doubles fire damage is `Modifier::Mul(2.0)` on `fire`, from
/// `curse`, which [`remove_modifiers`](Self::remove_modifiers) takes back.
///
/// A factor may have a floor ([`with_floor`](Self::with_floor)), the
/// lowest it goes once above 0, which modifiers change too
/// ([`add_floor_modifier`](Self::add_floor_modifier)); a hit's factor rises
/// to its floor before the part's penetration, so penetration works past a
/// capped resistance. A [`Line`] scoped by tag filters
/// ([`add_line`](Self::add_line)) changes a factor only in a hit whose
/// context passes it: the part's types, the hit's track, state tags and
/// side, the target's own tags for `while` and the attacker's for
/// `against`.
///
/// Scene name `resist`: `{ "damage.fire": 0.5, "damage.poison": 0 }`, each
/// key a declared tag, each base finite in `[0, 1000]`, or a factor with
/// its floor, `{ "damage.cold": { "factor": 0.5, "floor": 0.25 } }`. It is
/// data: a save writes a factor without modifiers as its base, as the scene
/// does, and one with modifiers as `{ "base": 0.5, "modifiers": [["curse", { "mul": 2 }]] }`,
/// `base` left out for a tag listed by a modifier only, with `floor`,
/// `floor_modifiers` and `lines` beside them when it has any.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{Modifier, Resist, Tags};
///
/// let resist = Resist::new().with_factor("damage", 0.5).with_factor("damage.fire", 0.5);
/// assert_eq!(resist.factor(&Tags::new(["damage.fire"])), 0.25, "the tag and its ancestor");
/// assert_eq!(resist.factor(&Tags::new(["damage.cold"])), 0.5);
/// assert_eq!(resist.factor(&Tags::new(["magic"])), 1.0, "no listed tag");
///
/// let mut cursed = resist.with_factor("damage.poison", 0.0);
/// cursed.add_modifier("damage.fire", "curse", Modifier::Mul(3.0));
/// cursed.add_modifier("damage.poison", "curse", Modifier::Add(1.0));
/// assert_eq!(cursed.factor(&Tags::new(["damage.fire"])), 0.75);
/// assert_eq!(cursed.factor(&Tags::new(["damage.poison"])), 0.0, "a base of 0 stays immune");
/// assert_eq!(cursed.remove_modifiers("curse"), 2);
/// assert_eq!(cursed.base("damage.fire"), Some(0.5));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ResistSave", into = "ResistSave")]
pub struct Resist {
    /// Sorted by tag.
    entries: Vec<Entry>,
    /// The floors and scoped lines, when it has any.
    more: Option<Box<More>>,
}

/// What only some resistances carry, boxed so the rest stay small: floors
/// and scoped lines.
#[derive(Debug, Clone, Default, PartialEq)]
struct More {
    /// Sorted by tag.
    floors: Vec<Floor>,
    /// Sorted by tag, in add order within one.
    lines: Vec<ScopedLine>,
}

impl More {
    fn is_empty(&self) -> bool {
        self.floors.is_empty() && self.lines.is_empty()
    }

    /// `tag`'s floor value; 0 without one.
    fn floor(&self, tag: &str) -> f32 {
        self.floors
            .binary_search_by(|floor| floor.tag.as_str().cmp(tag))
            .map_or(0.0, |at| self.floors[at].value)
    }

    /// The scoped lines on `tag` that `read` takes, in add order.
    fn taken<'s>(
        &'s self,
        tag: &'s str,
        read: &'s Read<'_>,
    ) -> impl Iterator<Item = &'s ScopedLine> + Clone {
        self.lines_on(tag)
            .filter(move |scoped| read.reach(&scoped.line) == Reach::Applies)
    }

    /// The scoped lines on `tag`, in add order.
    fn lines_on<'s>(&'s self, tag: &'s str) -> impl Iterator<Item = &'s ScopedLine> + Clone {
        let start = self.lines.partition_point(|line| line.tag.as_str() < tag);
        self.lines[start..]
            .iter()
            .take_while(move |line| line.tag == tag)
    }
}

/// One tag's floor: its base, its modifiers by source and the value they
/// make, in `[0, 1000]`.
#[derive(Debug, Clone, PartialEq)]
struct Floor {
    tag: String,
    /// `None` for a floor listed by a modifier only, whose base is 0.
    base: Option<f32>,
    modifiers: Vec<(String, Modifier)>,
    value: f32,
}

impl Floor {
    fn new(tag: &str, base: Option<f32>, modifiers: Vec<(String, Modifier)>) -> Self {
        let mut floor = Self {
            tag: tag.to_owned(),
            base,
            modifiers,
            value: 0.0,
        };
        floor.recompute();
        floor
    }

    fn recompute(&mut self) {
        self.value = modified(
            self.base.unwrap_or(0.0),
            self.modifiers.iter().map(|&(_, modifier)| modifier),
            0.0,
            MAX_FACTOR,
        );
    }
}

/// A scoped line on one tag's factor, from a source.
#[derive(Debug, Clone, PartialEq)]
struct ScopedLine {
    tag: String,
    source: String,
    line: Line,
}

/// One resisted tag: its base, its modifiers by source in add order, and
/// the factor they make.
#[derive(Debug, Clone, PartialEq)]
struct Entry {
    tag: String,
    /// `None` for a tag listed by a modifier only, whose base is 1.
    base: Option<f32>,
    modifiers: Vec<(String, Modifier)>,
    factor: f32,
}

impl Entry {
    fn new(tag: &str, base: Option<f32>, modifiers: Vec<(String, Modifier)>) -> Self {
        let mut entry = Self {
            tag: tag.to_owned(),
            base,
            modifiers,
            factor: 0.0,
        };
        entry.recompute();
        entry
    }

    /// A base of 0 stays 0; a base without modifiers is the factor as it
    /// is; otherwise the modifiers over the base, or over 1 without one.
    fn recompute(&mut self) {
        self.factor = match self.base {
            Some(base) if base == 0.0 || self.modifiers.is_empty() => base,
            base => modified(
                base.unwrap_or(1.0),
                self.modifiers.iter().map(|&(_, modifier)| modifier),
                0.0,
                MAX_FACTOR,
            ),
        };
    }
}

impl TryFrom<BTreeMap<String, f32>> for Resist {
    type Error = String;

    /// Resistances of these bases, without modifiers: the scene's form.
    fn try_from(factors: BTreeMap<String, f32>) -> Result<Self, Self::Error> {
        let mut entries = Vec::with_capacity(factors.len());
        for (tag, factor) in factors {
            check_factor(&tag, factor)?;
            entries.push(Entry::new(&tag, Some(factor), Vec::new()));
        }
        Ok(Self {
            entries,
            more: None,
        })
    }
}

/// `tag` a tag name and `factor` finite in `[0, 1000]`.
fn check_factor(tag: &str, factor: f32) -> Result<(), String> {
    check_tag(tag)?;
    if !(factor.is_finite() && (0.0..=MAX_FACTOR).contains(&factor)) {
        return Err(format!(
            "{tag}: the factor must be finite and in [0, {MAX_FACTOR}] (got {factor})"
        ));
    }
    Ok(())
}

/// `tag` a tag name.
fn check_tag(tag: &str) -> Result<(), String> {
    if valid_tag(tag) {
        Ok(())
    } else {
        Err(format!(
            "`{tag}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
        ))
    }
}

/// Every modifier finite.
fn check_modifiers(tag: &str, modifiers: &[(String, Modifier)]) -> Result<(), String> {
    match modifiers.iter().find(|(_, modifier)| !modifier.is_finite()) {
        Some((_, modifier)) => Err(format!(
            "{tag}: a modifier must be finite (got {modifier:?})"
        )),
        None => Ok(()),
    }
}

impl Resist {
    /// No resistances: every factor 1.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The resistances with base `factor` for damage tagged `tag`, added or
    /// replaced, its modifiers kept; 0 is immunity.
    ///
    /// # Panics
    ///
    /// When `tag` is not a tag name, or `factor` is not finite and in
    /// `[0, 1000]`.
    #[must_use]
    #[track_caller]
    pub fn with_factor(mut self, tag: &str, factor: f32) -> Self {
        if let Err(error) = check_factor(tag, factor) {
            panic!("Resist::with_factor: {error}");
        }
        match self.search(tag) {
            Ok(at) => {
                self.entries[at].base = Some(factor);
                self.entries[at].recompute();
            }
            Err(at) => self
                .entries
                .insert(at, Entry::new(tag, Some(factor), Vec::new())),
        }
        self
    }

    /// Adds `modifier` to `tag`'s factor from `source`, after its earlier
    /// ones, as [`Stats::add_modifier`](crate::Stats::add_modifier) does; a
    /// tag without a factor takes one of base 1, which goes with its last
    /// modifier. A base of 0 stays immune whatever its modifiers.
    ///
    /// # Panics
    ///
    /// When `tag` is not a tag name or the modifier's value is not finite.
    #[track_caller]
    pub fn add_modifier(&mut self, tag: &str, source: &str, modifier: Modifier) {
        if let Err(error) = check_tag(tag) {
            panic!("Resist::add_modifier: {error}");
        }
        assert!(
            modifier.is_finite(),
            "Resist::add_modifier: {tag}: the modifier must be finite (got {modifier:?})"
        );
        match self.search(tag) {
            Ok(at) => {
                let entry = &mut self.entries[at];
                entry.modifiers.push((source.to_owned(), modifier));
                entry.recompute();
            }
            Err(at) => self.entries.insert(
                at,
                Entry::new(tag, None, vec![(source.to_owned(), modifier)]),
            ),
        }
    }

    /// The resistances with a floor of `floor` for `tag`'s factor, added or
    /// replaced, its modifiers kept: the lowest the factor goes in a hit
    /// once above 0.
    ///
    /// # Panics
    ///
    /// When `tag` is not a tag name, or `floor` is not finite and in
    /// `[0, 1000]`.
    #[must_use]
    #[track_caller]
    pub fn with_floor(mut self, tag: &str, floor: f32) -> Self {
        if let Err(error) = check_factor(tag, floor) {
            panic!("Resist::with_floor: {error}");
        }
        let floors = &mut self.more.get_or_insert_with(Box::default).floors;
        match floors.binary_search_by(|entry| entry.tag.as_str().cmp(tag)) {
            Ok(at) => {
                floors[at].base = Some(floor);
                floors[at].recompute();
            }
            Err(at) => floors.insert(at, Floor::new(tag, Some(floor), Vec::new())),
        }
        self
    }

    /// Adds `modifier` to `tag`'s floor from `source`, after its earlier
    /// ones; a tag without a floor takes one of base 0.
    ///
    /// # Panics
    ///
    /// When `tag` is not a tag name or the modifier's value is not finite.
    #[track_caller]
    pub fn add_floor_modifier(&mut self, tag: &str, source: &str, modifier: Modifier) {
        if let Err(error) = check_tag(tag) {
            panic!("Resist::add_floor_modifier: {error}");
        }
        assert!(
            modifier.is_finite(),
            "Resist::add_floor_modifier: {tag}: the modifier must be finite (got {modifier:?})"
        );
        let floors = &mut self.more.get_or_insert_with(Box::default).floors;
        match floors.binary_search_by(|entry| entry.tag.as_str().cmp(tag)) {
            Ok(at) => {
                floors[at].modifiers.push((source.to_owned(), modifier));
                floors[at].recompute();
            }
            Err(at) => floors.insert(
                at,
                Floor::new(tag, None, vec![(source.to_owned(), modifier)]),
            ),
        }
    }

    /// Adds `line` to `tag`'s factor from `source`: an unscoped line as
    /// [`add_modifier`](Self::add_modifier) does, a scoped one read only in
    /// a hit whose context passes it, over the factor's base, or 1 for a
    /// tag the resistances list no factor for.
    ///
    /// # Panics
    ///
    /// When `tag` is not a tag name.
    #[track_caller]
    pub fn add_line(&mut self, tag: &str, source: &str, line: Line) {
        if let Err(error) = check_tag(tag) {
            panic!("Resist::add_line: {error}");
        }
        if !line.is_scoped() {
            self.add_modifier(tag, source, line.modifier());
            return;
        }
        let lines = &mut self.more.get_or_insert_with(Box::default).lines;
        let at = lines.partition_point(|kept| kept.tag.as_str() <= tag);
        lines.insert(
            at,
            ScopedLine {
                tag: tag.to_owned(),
                source: source.to_owned(),
                line,
            },
        );
    }

    /// Removes every modifier, floor modifier and scoped line `source`
    /// gave, on every tag, and recomputes those factors and floors,
    /// dropping a tag left with no base and no modifier; how many it
    /// removed.
    pub fn remove_modifiers(&mut self, source: &str) -> usize {
        let mut removed = 0;
        for entry in &mut self.entries {
            let before = entry.modifiers.len();
            entry.modifiers.retain(|(from, _)| from != source);
            if entry.modifiers.len() != before {
                removed += before - entry.modifiers.len();
                entry.recompute();
            }
        }
        self.entries
            .retain(|entry| entry.base.is_some() || !entry.modifiers.is_empty());
        if let Some(more) = &mut self.more {
            for floor in &mut more.floors {
                let before = floor.modifiers.len();
                floor.modifiers.retain(|(from, _)| from != source);
                if floor.modifiers.len() != before {
                    removed += before - floor.modifiers.len();
                    floor.recompute();
                }
            }
            more.floors
                .retain(|floor| floor.base.is_some() || !floor.modifiers.is_empty());
            let before = more.lines.len();
            more.lines.retain(|line| line.source != source);
            removed += before - more.lines.len();
            if more.is_empty() {
                self.more = None;
            }
        }
        removed
    }

    /// `tag`'s floor, when it lists one: the lowest its factor goes in a
    /// hit once above 0.
    #[must_use]
    pub fn floor(&self, tag: &str) -> Option<f32> {
        let more = self.more.as_ref()?;
        more.floors
            .binary_search_by(|floor| floor.tag.as_str().cmp(tag))
            .ok()
            .map(|at| more.floors[at].value)
    }

    /// The product of the factors whose tag `tags` carries in `read`'s
    /// context, as a hit with no penetration meets them: each with the
    /// scoped lines the read takes, raised to its floor, 0 when one of them
    /// is 0, clamped to `f32::MAX`.
    #[must_use]
    pub fn factor_in(&self, tags: &Tags, read: &Read<'_>) -> f32 {
        self.product_in(tags, 0.0, read)
            .map_or(0.0, |product| product.min(f64::from(f32::MAX)) as f32)
    }

    /// `tag`'s base factor, when it lists one.
    #[must_use]
    pub fn base(&self, tag: &str) -> Option<f32> {
        self.search(tag).ok().and_then(|at| self.entries[at].base)
    }

    /// The product of the factors whose tag `tags` carries, each after its
    /// modifiers and raised to its floor, 1 when none does and 0 when one
    /// of them is 0, clamped to `f32::MAX`: the factor in no context.
    #[must_use]
    pub fn factor(&self, tags: &Tags) -> f32 {
        self.factor_in(tags, &Read::default())
    }

    /// Whether a hit's context could change a factor: it has a floor or a
    /// scoped line.
    pub(crate) fn reads_context(&self) -> bool {
        self.more.is_some()
    }

    /// [`product_with`](Self::product_with) in `read`'s context: each
    /// matching factor with the scoped lines the read takes, 0 is immunity,
    /// the rest raised to their floors before `points`.
    pub(crate) fn product_in(&self, tags: &Tags, points: f32, read: &Read<'_>) -> Option<f64> {
        let Some(more) = &self.more else {
            return self.product_with(tags, points);
        };
        let raise = |tag: &str, factor: f32| -> f64 {
            let factor = f64::from(factor.max(more.floor(tag)));
            if points > 0.0 && factor < 1.0 {
                (factor + f64::from(points)).min(1.0)
            } else {
                factor
            }
        };
        let mut product = 1.0_f64;
        for entry in self.entries.iter().filter(|entry| tags.has(&entry.tag)) {
            let lines = more.taken(&entry.tag, read);
            let factor = match entry.base {
                _ if lines.clone().next().is_none() => entry.factor,
                Some(0.0) => 0.0,
                base => modified(
                    base.unwrap_or(1.0),
                    entry
                        .modifiers
                        .iter()
                        .map(|&(_, modifier)| modifier)
                        .chain(lines.map(|scoped| scoped.line.modifier())),
                    0.0,
                    MAX_FACTOR,
                ),
            };
            if factor == 0.0 {
                return None;
            }
            product *= raise(&entry.tag, factor);
        }
        // A scoped line on a tag without a factor stands over a base of 1,
        // once a tag.
        for (at, scoped) in more.lines.iter().enumerate() {
            if !tags.has(&scoped.tag)
                || (at > 0 && more.lines[at - 1].tag == scoped.tag)
                || self.search(&scoped.tag).is_ok()
            {
                continue;
            }
            let lines = more.taken(&scoped.tag, read);
            if lines.clone().next().is_none() {
                continue;
            }
            let factor = modified(
                1.0,
                lines.map(|scoped| scoped.line.modifier()),
                0.0,
                MAX_FACTOR,
            );
            if factor == 0.0 {
                return None;
            }
            product *= raise(&scoped.tag, factor);
        }
        Some(product)
    }

    /// The product of the matching factors in `f64`, each in `(0, 1)`
    /// raised by `points` and capped at 1, or `None` when one of them is 0:
    /// an immunity or a modifier's 0, found before any product.
    pub(crate) fn product_with(&self, tags: &Tags, points: f32) -> Option<f64> {
        let matching = || self.entries.iter().filter(|entry| tags.has(&entry.tag));
        if matching().any(|entry| entry.factor == 0.0) {
            return None;
        }
        Some(
            matching()
                .map(|entry| {
                    let factor = f64::from(entry.factor);
                    if points > 0.0 && entry.factor < 1.0 {
                        (factor + f64::from(points)).min(1.0)
                    } else {
                        factor
                    }
                })
                .product(),
        )
    }

    fn search(&self, tag: &str) -> Result<usize, usize> {
        self.entries
            .binary_search_by(|entry| entry.tag.as_str().cmp(tag))
    }
}

/// A resistance as a save writes it: factors by tag, each a bare base or a
/// base with its modifiers.
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct ResistSave(BTreeMap<String, SavedFactor>);

/// One saved factor: `0.5`, or `{ "base": 0.5, "modifiers": [..] }` with
/// `base` optional, never `{}`; a floor and scoped lines beside them.
struct SavedFactor {
    base: Option<f32>,
    modifiers: Vec<(String, Modifier)>,
    more: Option<Box<SavedMore>>,
}

/// A saved factor's floor and scoped lines.
#[derive(Default)]
struct SavedMore {
    floor: Option<f32>,
    floor_modifiers: Vec<(String, Modifier)>,
    lines: Vec<(String, Line)>,
}

impl Serialize for SavedFactor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct WithModifiers<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            base: Option<f32>,
            #[serde(skip_serializing_if = "<[_]>::is_empty")]
            modifiers: &'a [(String, Modifier)],
            #[serde(skip_serializing_if = "Option::is_none")]
            floor: Option<f32>,
            #[serde(skip_serializing_if = "<[_]>::is_empty")]
            floor_modifiers: &'a [(String, Modifier)],
            #[serde(skip_serializing_if = "<[_]>::is_empty")]
            lines: &'a [(String, Line)],
        }
        match (self.base, self.modifiers.is_empty(), &self.more) {
            (Some(base), true, None) => serializer.serialize_f32(base),
            (base, _, None) => {
                #[derive(Serialize)]
                struct Plain<'a> {
                    #[serde(skip_serializing_if = "Option::is_none")]
                    base: Option<f32>,
                    modifiers: &'a [(String, Modifier)],
                }
                Plain {
                    base,
                    modifiers: &self.modifiers,
                }
                .serialize(serializer)
            }
            (base, _, Some(more)) => WithModifiers {
                base,
                modifiers: &self.modifiers,
                floor: more.floor,
                floor_modifiers: &more.floor_modifiers,
                lines: &more.lines,
            }
            .serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for SavedFactor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(SavedFactorVisitor)
    }
}

/// Reads either form of a saved factor.
struct SavedFactorVisitor;

impl SavedFactorVisitor {
    fn base(value: f64) -> SavedFactor {
        SavedFactor {
            base: Some(value as f32),
            modifiers: Vec::new(),
            more: None,
        }
    }
}

impl<'de> Visitor<'de> for SavedFactorVisitor {
    type Value = SavedFactor;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a factor, or an object of `base` and `modifiers`")
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<SavedFactor, E> {
        Ok(Self::base(value))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<SavedFactor, E> {
        Ok(Self::base(value as f64))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<SavedFactor, E> {
        Ok(Self::base(value as f64))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<SavedFactor, A::Error> {
        const FIELDS: &[&str] = &["base", "modifiers", "floor", "floor_modifiers", "lines"];
        let mut base = None;
        let mut modifiers = None;
        let mut floor = None;
        let mut floor_modifiers = None;
        let mut lines = None;
        while let Some(key) = map.next_key::<String>()? {
            let duplicate = match key.as_str() {
                "base" => base.replace(map.next_value::<f32>()?).is_some(),
                "modifiers" => modifiers
                    .replace(map.next_value::<Vec<(String, Modifier)>>()?)
                    .is_some(),
                "floor" => floor.replace(map.next_value::<f32>()?).is_some(),
                "floor_modifiers" => floor_modifiers
                    .replace(map.next_value::<Vec<(String, Modifier)>>()?)
                    .is_some(),
                "lines" => lines
                    .replace(map.next_value::<Vec<(String, Line)>>()?)
                    .is_some(),
                other => return Err(de::Error::unknown_field(other, FIELDS)),
            };
            if duplicate {
                let field = FIELDS.iter().find(|field| **field == key).copied();
                return Err(de::Error::duplicate_field(field.unwrap_or("base")));
            }
        }
        let modifiers = modifiers.unwrap_or_default();
        let more = SavedMore {
            floor,
            floor_modifiers: floor_modifiers.unwrap_or_default(),
            lines: lines.unwrap_or_default(),
        };
        let more =
            (more.floor.is_some() || !more.floor_modifiers.is_empty() || !more.lines.is_empty())
                .then(|| Box::new(more));
        if base.is_none() && modifiers.is_empty() && more.is_none() {
            return Err(de::Error::custom(
                "a factor names its `base`, its `modifiers`, a floor or lines",
            ));
        }
        Ok(SavedFactor {
            base,
            modifiers,
            more,
        })
    }
}

impl TryFrom<ResistSave> for Resist {
    type Error = String;

    fn try_from(save: ResistSave) -> Result<Self, Self::Error> {
        let mut resist = Self {
            entries: Vec::with_capacity(save.0.len()),
            more: None,
        };
        for (tag, factor) in save.0 {
            match factor.base {
                Some(base) => check_factor(&tag, base)?,
                None => check_tag(&tag)?,
            }
            check_modifiers(&tag, &factor.modifiers)?;
            if factor.base.is_some() || !factor.modifiers.is_empty() {
                resist
                    .entries
                    .push(Entry::new(&tag, factor.base, factor.modifiers));
            }
            let Some(more) = factor.more else {
                continue;
            };
            if let Some(floor) = more.floor {
                check_factor(&tag, floor)?;
            }
            check_modifiers(&tag, &more.floor_modifiers)?;
            if more.floor.is_some() || !more.floor_modifiers.is_empty() {
                resist
                    .more
                    .get_or_insert_with(Box::default)
                    .floors
                    .push(Floor::new(&tag, more.floor, more.floor_modifiers));
            }
            for (source, line) in more.lines {
                if line.is_scoped() {
                    resist.add_line(&tag, &source, line);
                } else {
                    return Err(format!("{tag}: a saved line is scoped"));
                }
            }
        }
        Ok(resist)
    }
}

/// The floor and lines `saved` holds for `tag`, made when missing.
fn saved_more(saved: &mut BTreeMap<String, SavedFactor>, tag: String) -> &mut SavedMore {
    saved
        .entry(tag)
        .or_insert_with(|| SavedFactor {
            base: None,
            modifiers: Vec::new(),
            more: None,
        })
        .more
        .get_or_insert_with(Box::default)
}

impl From<Resist> for ResistSave {
    fn from(resist: Resist) -> Self {
        let mut saved: BTreeMap<String, SavedFactor> = resist
            .entries
            .into_iter()
            .map(|entry| {
                let factor = SavedFactor {
                    base: entry.base,
                    modifiers: entry.modifiers,
                    more: None,
                };
                (entry.tag, factor)
            })
            .collect();
        if let Some(more) = resist.more {
            for floor in more.floors {
                let fields = saved_more(&mut saved, floor.tag);
                fields.floor = floor.base;
                fields.floor_modifiers = floor.modifiers;
            }
            for scoped in more.lines {
                saved_more(&mut saved, scoped.tag)
                    .lines
                    .push((scoped.source, scoped.line));
            }
        }
        Self(saved)
    }
}

/// A shield that takes damage before health: a hit's amount, after the
/// resistances, comes off the shield first and only the rest off the
/// [`Health`](crate::Health). After `delay` seconds of game time without a
/// hit it recharges at `recharge` per second, up to its max;
/// [`HealthPlugin`](crate::HealthPlugin)'s [`SHIELD_TICK`] counts both on
/// the fixed step. A hit the shield takes whole is
/// [`Hit::Absorbed`](crate::Hit::Absorbed). A heal never fills a shield;
/// [`restore`](Self::restore) does.
///
/// Scene name `shield`: `{ "max": 3, "recharge": 1, "delay": 2 }`,
/// `recharge` and `delay` 0 by default, so it does not recharge. It is
/// data: a game's save can hold it.
///
/// # Examples
///
/// ```
/// use tungsten_core::{Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{Health, HealthPlugin, Hit, Shield, damage};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// HealthPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let carrier = world.spawn_with((Health::new(5.0), Shield::new(2.0).with_recharge(1.0, 0.5)));
/// assert_eq!(damage(&mut world, carrier, 1.5), Hit::Absorbed);
/// assert_eq!(damage(&mut world, carrier, 1.5), Hit::Hurt, "0.5 left, then 1 off health");
/// assert_eq!(world.get::<Health>(carrier).unwrap().current(), 4.0);
///
/// // Half a second without a hit, then a second of recharge.
/// for _ in 0..90 {
///     world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
///     schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// }
/// let shield = world.get::<Shield>(carrier).unwrap();
/// assert!((shield.current() - 1.0).abs() < 1e-4, "{}", shield.current());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ShieldFields", into = "ShieldFields")]
pub struct Shield {
    current: f32,
    max: f32,
    recharge: f32,
    delay: f32,
    /// Game seconds since its last hit.
    since_hit: f32,
}

/// A shield as a save writes it, before its values are checked.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ShieldFields {
    current: f32,
    max: f32,
    #[serde(default)]
    recharge: f32,
    #[serde(default)]
    delay: f32,
    #[serde(default)]
    since_hit: f32,
}

impl TryFrom<ShieldFields> for Shield {
    type Error = String;

    fn try_from(fields: ShieldFields) -> Result<Self, Self::Error> {
        let ShieldFields {
            current,
            max,
            recharge,
            delay,
            since_hit,
        } = fields;
        if !(max.is_finite() && max > 0.0) {
            return Err(format!("max: must be finite and > 0 (got {max})"));
        }
        if !(current.is_finite() && (0.0..=max).contains(&current)) {
            return Err(format!("current: must be within 0..={max} (got {current})"));
        }
        for (field, value) in [
            ("recharge", recharge),
            ("delay", delay),
            ("since_hit", since_hit),
        ] {
            if !(value.is_finite() && value >= 0.0) {
                return Err(format!("{field}: must be finite and >= 0 (got {value})"));
            }
        }
        Ok(Self {
            current,
            max,
            recharge,
            delay,
            since_hit,
        })
    }
}

impl From<Shield> for ShieldFields {
    fn from(shield: Shield) -> Self {
        Self {
            current: shield.current,
            max: shield.max,
            recharge: shield.recharge,
            delay: shield.delay,
            since_hit: shield.since_hit,
        }
    }
}

impl Shield {
    /// A full shield of `max`, which does not recharge.
    ///
    /// # Panics
    ///
    /// When `max` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn new(max: f32) -> Self {
        assert!(
            max.is_finite() && max > 0.0,
            "Shield::new: max must be finite and > 0 (got {max})"
        );
        Self {
            current: max,
            max,
            recharge: 0.0,
            delay: 0.0,
            since_hit: 0.0,
        }
    }

    /// The shield, recharging `per_second` once `delay` seconds of game time
    /// have passed since its last hit.
    ///
    /// # Panics
    ///
    /// When either is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_recharge(mut self, per_second: f32, delay: f32) -> Self {
        for (name, value) in [("per_second", per_second), ("delay", delay)] {
            assert!(
                value.is_finite() && value >= 0.0,
                "Shield::with_recharge: {name} must be finite and >= 0 (got {value})"
            );
        }
        self.recharge = per_second;
        self.delay = delay;
        self
    }

    /// The shield left, from 0 to [`max`](Self::max).
    #[must_use]
    pub fn current(&self) -> f32 {
        self.current
    }

    /// The full shield.
    #[must_use]
    pub fn max(&self) -> f32 {
        self.max
    }

    /// Adds `amount`, up to [`max`](Self::max): a pickup that fills the
    /// shield, which a heal does not.
    ///
    /// # Panics
    ///
    /// When `amount` is not finite and at least 0.
    #[track_caller]
    pub fn restore(&mut self, amount: f32) {
        assert!(
            amount.is_finite() && amount >= 0.0,
            "Shield::restore: amount must be finite and >= 0 (got {amount})"
        );
        self.current = (self.current + amount).min(self.max);
    }

    /// Takes up to `amount` and restarts the delay; what it took.
    pub(crate) fn take(&mut self, amount: f32) -> f32 {
        let taken = self.current.min(amount);
        self.current -= taken;
        self.since_hit = 0.0;
        taken
    }

    /// Counts `dt` of game time without a hit and recharges by the part of
    /// it past the delay, so a recharge lands the same at any frame rate.
    fn tick(&mut self, dt: f32) {
        let before = self.since_hit;
        self.since_hit = (before + dt).min(f32::MAX);
        if self.recharge <= 0.0 || self.current >= self.max {
            return;
        }
        let past = self.since_hit - self.delay.max(before);
        if past > 0.0 {
            self.current = (self.current + self.recharge * past.min(dt)).min(self.max);
        }
    }
}

/// [`SHIELD_TICK`]'s work: every shield counts the step.
pub(crate) fn shield_tick(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    if dt <= 0.0 {
        return;
    }
    for shield in world.query_mut::<&mut Shield>() {
        shield.tick(dt);
    }
}

/// `components.resist` as a scene writes it: factors by tag, in name
/// order, each a factor or a factor with its floor, checked.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "BTreeMap<String, FactorDef>")]
pub(crate) struct ResistDef(pub(crate) BTreeMap<String, FactorDef>);

impl TryFrom<BTreeMap<String, FactorDef>> for ResistDef {
    type Error = String;

    fn try_from(factors: BTreeMap<String, FactorDef>) -> Result<Self, Self::Error> {
        for (tag, factor) in &factors {
            check_factor(tag, factor.factor)?;
            if let Some(floor) = factor.floor {
                check_factor(tag, floor).map_err(|e| format!("floor: {e}"))?;
            }
        }
        Ok(Self(factors))
    }
}

/// One factor as a scene writes it: `0.5`, or `{ "factor": 0.5, "floor":
/// 0.25 }`.
pub(crate) struct FactorDef {
    factor: f32,
    floor: Option<f32>,
}

impl Serialize for FactorDef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct WithFloor {
            factor: f32,
            floor: f32,
        }
        match self.floor {
            None => serializer.serialize_f32(self.factor),
            Some(floor) => WithFloor {
                factor: self.factor,
                floor,
            }
            .serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for FactorDef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(FactorVisitor)
    }
}

/// The object form of a [`FactorDef`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FactorFields {
    factor: f32,
    floor: f32,
}

/// Reads either form of a scene's factor.
struct FactorVisitor;

impl<'de> Visitor<'de> for FactorVisitor {
    type Value = FactorDef;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a factor, or an object of `factor` and `floor`")
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<FactorDef, E> {
        Ok(FactorDef {
            factor: value as f32,
            floor: None,
        })
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<FactorDef, E> {
        self.visit_f64(value as f64)
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<FactorDef, E> {
        self.visit_f64(value as f64)
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<FactorDef, A::Error> {
        let fields = FactorFields::deserialize(de::value::MapAccessDeserializer::new(map))?;
        Ok(FactorDef {
            factor: fields.factor,
            floor: Some(fields.floor),
        })
    }
}

impl From<ResistDef> for Resist {
    fn from(def: ResistDef) -> Self {
        let mut resist = Self::default();
        for (tag, factor) in def.0 {
            resist
                .entries
                .push(Entry::new(&tag, Some(factor.factor), Vec::new()));
            if let Some(floor) = factor.floor {
                resist = resist.with_floor(&tag, floor);
            }
        }
        resist
    }
}

/// `components.shield` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ShieldDef {
    #[serde(deserialize_with = "crate::foundation::positive")]
    pub(crate) max: f32,
    #[serde(default, deserialize_with = "crate::foundation::non_negative")]
    pub(crate) recharge: f32,
    #[serde(default, deserialize_with = "crate::foundation::non_negative")]
    pub(crate) delay: f32,
}

/// The descriptions of `resist` and `shield`.
pub(crate) fn resist_description() -> ComponentDescription {
    ComponentDescription::new(
        RESIST,
        "Damage factors by damage tag: a hit part's matching factors multiply its amount, and a factor of 0 is immunity.",
    )
    .field(FieldDescription::new(
        "factors",
        FieldType::Object("damage tag → factor in [0, 1000], or { factor, floor }"),
        "the value itself: each tag declared, matching a part's type or its dotted descendants; a floor in [0, 1000] is the lowest a factor above 0 goes in a hit, before penetration; a save adds each factor's modifiers and scoped lines",
    ))
    .genre(TOP_DOWN)
    .genre(SHMUP)
    .sample(r#"{ "damage.fire": 0.5 }"#)
    .roundtrip(round_trip::<ResistDef>)
}

pub(crate) fn shield_description() -> ComponentDescription {
    ComponentDescription::new(
        SHIELD,
        "Takes damage before health, and recharges after a delay without a hit.",
    )
    .field(FieldDescription::new(
        "max",
        FieldType::F32 {
            min: Bound::Exclusive(0.0),
            max: Bound::Open,
        },
        "the full shield, which it starts at",
    ))
    .field(
        FieldDescription::new(
            "recharge",
            FieldType::F32 {
                min: Bound::Inclusive(0.0),
                max: Bound::Open,
            },
            "shield restored per second of game time once the delay has passed",
        )
        .default("0.0"),
    )
    .field(
        FieldDescription::new(
            "delay",
            FieldType::F32 {
                min: Bound::Inclusive(0.0),
                max: Bound::Open,
            },
            "game seconds without a hit before it recharges",
        )
        .default("0.0"),
    )
    .genre(SHMUP)
    .genre(TOP_DOWN)
    .sample(r#"{ "max": 3 }"#)
    .roundtrip(round_trip::<ShieldDef>)
}

impl Inspectable for Resist {
    /// Each tag's factor, with its base and modifiers when it has any.
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let factors: Vec<String> = self
            .entries
            .iter()
            .map(|entry| {
                if entry.modifiers.is_empty() {
                    return format!("{} × {}", entry.tag, entry.factor);
                }
                let base = entry
                    .base
                    .map_or_else(|| "none".to_string(), |base| base.to_string());
                let modifiers: Vec<String> = entry
                    .modifiers
                    .iter()
                    .map(|(source, modifier)| format!("{source} {modifier:?}"))
                    .collect();
                format!(
                    "{} × {} (base {base}; {})",
                    entry.tag,
                    entry.factor,
                    modifiers.join(", ")
                )
            })
            .collect();
        let mut rows = vec![("resist", factors.join(", "))];
        if let Some(more) = &self.more {
            if !more.floors.is_empty() {
                let floors: Vec<String> = more
                    .floors
                    .iter()
                    .map(|floor| format!("{} ≥ {}", floor.tag, floor.value))
                    .collect();
                rows.push(("floors", floors.join(", ")));
            }
            if !more.lines.is_empty() {
                let lines: Vec<String> = more
                    .lines
                    .iter()
                    .map(|scoped| format!("{} from {}", scoped.tag, scoped.source))
                    .collect();
                rows.push(("scoped", lines.join(", ")));
            }
        }
        rows
    }
}

impl Inspectable for Shield {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![
            ("shield", format!("{} / {}", self.current, self.max)),
            (
                "recharge",
                format!("{} /s after {:.2} s", self.recharge, self.delay),
            ),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/defense.rs"]
mod tests;
