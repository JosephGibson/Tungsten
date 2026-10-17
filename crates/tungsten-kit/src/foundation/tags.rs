//! Tags: dotted names on an entity, the filters that match them, and the
//! declarations a load checks tag names against.

use serde::{Deserialize, Serialize};
use tungsten_core::{
    DataRegistry, Inspectable, MissingAsset, Plugin, Schedule, SpawnContext, World,
};

use crate::foundation::describe::{
    ComponentDescription, FieldDescription, FieldType, GRID, PLATFORMER, SHMUP, TOP_DOWN, describe,
    describe_kind, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::ensure_phases;

/// The scene component [`TagsPlugin`] registers, and the data kind.
pub(crate) const TAGS: &str = "tags";

/// Whether `name` is a tag name: `[a-z][a-z0-9_]*` segments joined by dots,
/// the rule counter and flag names follow too.
pub(crate) fn valid_tag(name: &str) -> bool {
    !name.is_empty()
        && name.split('.').all(|segment| {
            let mut chars = segment.chars();
            chars.next().is_some_and(|first| first.is_ascii_lowercase())
                && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        })
}

/// Whether `declared` is `name` or a dotted descendant of it, so declaring
/// `team.player` declares `team` too; and whether a `name` an entity
/// carries is `tag` or a descendant, so an entity tagged `team.enemy` has
/// `team`.
fn covers(declared: &str, name: &str) -> bool {
    declared == name
        || (declared.starts_with(name) && declared.as_bytes().get(name.len()) == Some(&b'.'))
}

/// The names that fail [`valid_tag`], as a message.
fn check_names(names: &[String]) -> Result<(), String> {
    match names.iter().find(|name| !valid_tag(name)) {
        Some(name) => Err(format!(
            "`{name}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
        )),
        None => Ok(()),
    }
}

/// `names` checked, sorted and deduplicated.
fn normalize(mut names: Vec<String>) -> Result<Vec<String>, String> {
    check_names(&names)?;
    names.sort();
    names.dedup();
    Ok(names)
}

/// An entity's tags: names such as `team.player` or `flying`, `[a-z][a-z0-9_]*`
/// segments joined by dots, kept sorted and deduplicated. A dotted name
/// carries its ancestors: an entity tagged `team.enemy` [`has`](Self::has)
/// `team`. Items filter on tags through a [`TagFilter`], so an item names no
/// game type.
///
/// Scene name `tags`: `"tags": ["team.player", "flying"]`, which the load
/// checks against the declared tags (see [`TagsPlugin`]). It is data: it
/// serializes as the array of names.
///
/// # Examples
///
/// ```
/// use tungsten_kit::Tags;
///
/// let mut tags = Tags::new(["team.enemy", "flying"]);
/// assert!(tags.has("team.enemy") && tags.has("team") && tags.has("flying"));
/// assert!(!tags.has("team.player") && !tags.has("fly"));
/// assert_eq!(tags.iter().collect::<Vec<_>>(), ["flying", "team.enemy"]);
///
/// assert!(tags.insert("boss"));
/// assert!(!tags.insert("boss"), "already there");
/// assert!(tags.remove("flying"));
/// assert_eq!(serde_json::to_value(&tags).unwrap(), serde_json::json!(["boss", "team.enemy"]));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<String>", into = "Vec<String>")]
pub struct Tags {
    names: Vec<String>,
}

impl TryFrom<Vec<String>> for Tags {
    type Error = String;

    fn try_from(names: Vec<String>) -> Result<Self, Self::Error> {
        Ok(Self {
            names: normalize(names)?,
        })
    }
}

impl From<Tags> for Vec<String> {
    fn from(tags: Tags) -> Self {
        tags.names
    }
}

impl Tags {
    /// Tags of `names`, sorted and deduplicated.
    ///
    /// # Panics
    ///
    /// When a name is not a tag name.
    #[must_use]
    #[track_caller]
    pub fn new(names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let names: Vec<String> = names.into_iter().map(Into::into).collect();
        Self::try_from(names).unwrap_or_else(|error| panic!("Tags::new: {error}"))
    }

    /// Whether the entity carries `tag`, exactly or as a dotted ancestor of
    /// one of its names: `has("team")` holds for `team.enemy`.
    #[must_use]
    pub fn has(&self, tag: &str) -> bool {
        self.names.iter().any(|name| covers(name, tag))
    }

    /// The names, sorted.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }

    /// Adds `tag`; whether it was new.
    ///
    /// # Panics
    ///
    /// When `tag` is not a tag name.
    #[track_caller]
    pub fn insert(&mut self, tag: impl Into<String>) -> bool {
        let tag = tag.into();
        assert!(
            valid_tag(&tag),
            "Tags::insert: `{tag}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
        );
        match self.names.binary_search(&tag) {
            Ok(_) => false,
            Err(at) => {
                self.names.insert(at, tag);
                true
            }
        }
    }

    /// Removes the exact name `tag`; whether it was there.
    pub fn remove(&mut self, tag: &str) -> bool {
        match self.names.binary_search_by(|name| name.as_str().cmp(tag)) {
            Ok(at) => {
                self.names.remove(at);
                true
            }
            Err(_) => false,
        }
    }

    /// How many names it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether it holds no name.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

/// A match over an entity's [`Tags`]: every name of `all`, at least one of
/// `any` and none of `none`, each read as [`Tags::has`] reads it. An empty
/// filter matches everything, and an entity without `Tags` matches a filter
/// with no `all` and no `any`.
///
/// Data form `{ "all": [..], "any": [..], "none": [..] }`, each optional,
/// as a [`Zone`](crate::Zone)'s `filter` is written.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{TagFilter, Tags};
///
/// let enemies = TagFilter::new().all(["team.enemy"]).none(["boss"]);
/// let grunt = Tags::new(["team.enemy"]);
/// let boss = Tags::new(["team.enemy", "boss"]);
/// assert!(enemies.matches(Some(&grunt)));
/// assert!(!enemies.matches(Some(&boss)));
/// assert!(!enemies.matches(None), "`all` needs tags");
///
/// let everything = TagFilter::new();
/// assert!(everything.is_everything() && everything.matches(None));
///
/// let parsed: TagFilter = serde_json::from_str(r#"{ "any": ["flying", "boss"] }"#).unwrap();
/// assert_eq!(parsed, TagFilter::new().any(["boss", "flying"]));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "TagFilterFields", into = "TagFilterFields")]
pub struct TagFilter {
    all: Vec<String>,
    any: Vec<String>,
    none: Vec<String>,
}

/// A filter as data writes it.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct TagFilterFields {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    all: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    any: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    none: Vec<String>,
}

impl TryFrom<TagFilterFields> for TagFilter {
    type Error = String;

    fn try_from(fields: TagFilterFields) -> Result<Self, Self::Error> {
        Ok(Self {
            all: normalize(fields.all).map_err(|e| format!("all: {e}"))?,
            any: normalize(fields.any).map_err(|e| format!("any: {e}"))?,
            none: normalize(fields.none).map_err(|e| format!("none: {e}"))?,
        })
    }
}

impl From<TagFilter> for TagFilterFields {
    fn from(filter: TagFilter) -> Self {
        Self {
            all: filter.all,
            any: filter.any,
            none: filter.none,
        }
    }
}

impl TagFilter {
    /// A filter that matches everything.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The filter, also needing every one of `tags`.
    ///
    /// # Panics
    ///
    /// When a name is not a tag name.
    #[must_use]
    #[track_caller]
    pub fn all(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.all = Self::extend(self.all, tags, "all");
        self
    }

    /// The filter, also needing at least one of `tags`.
    ///
    /// # Panics
    ///
    /// When a name is not a tag name.
    #[must_use]
    #[track_caller]
    pub fn any(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.any = Self::extend(self.any, tags, "any");
        self
    }

    /// The filter, also refusing every one of `tags`.
    ///
    /// # Panics
    ///
    /// When a name is not a tag name.
    #[must_use]
    #[track_caller]
    pub fn none(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.none = Self::extend(self.none, tags, "none");
        self
    }

    #[track_caller]
    fn extend(
        mut names: Vec<String>,
        tags: impl IntoIterator<Item = impl Into<String>>,
        method: &str,
    ) -> Vec<String> {
        names.extend(tags.into_iter().map(Into::into));
        normalize(names).unwrap_or_else(|error| panic!("TagFilter::{method}: {error}"))
    }

    /// Whether an entity with `tags` passes.
    #[must_use]
    pub fn matches(&self, tags: Option<&Tags>) -> bool {
        let has = |tag: &String| tags.is_some_and(|tags| tags.has(tag));
        self.all.iter().all(has)
            && (self.any.is_empty() || self.any.iter().any(has))
            && !self.none.iter().any(has)
    }

    /// Whether the filter names no tag, so everything passes.
    #[must_use]
    pub fn is_everything(&self) -> bool {
        self.all.is_empty() && self.any.is_empty() && self.none.is_empty()
    }
}

/// A filter as a scene component writes it, its names checked but kept in
/// the written order, so a load's error names the field as written; a
/// [`TagFilter`] comes from it by `From`.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(try_from = "TagFilterFields")]
pub(crate) struct TagFilterDef {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    all: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    any: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    none: Vec<String>,
}

impl TryFrom<TagFilterFields> for TagFilterDef {
    type Error = String;

    fn try_from(fields: TagFilterFields) -> Result<Self, Self::Error> {
        check_names(&fields.all).map_err(|e| format!("all: {e}"))?;
        check_names(&fields.any).map_err(|e| format!("any: {e}"))?;
        check_names(&fields.none).map_err(|e| format!("none: {e}"))?;
        Ok(Self {
            all: fields.all,
            any: fields.any,
            none: fields.none,
        })
    }
}

impl From<TagFilterDef> for TagFilter {
    fn from(def: TagFilterDef) -> Self {
        Self::new().all(def.all).any(def.any).none(def.none)
    }
}

impl TagFilterDef {
    /// Every name the filter uses, with the field that names it as
    /// written (`all.0`), for a load's check.
    pub(crate) fn names(&self) -> impl Iterator<Item = (String, &str)> {
        [("all", &self.all), ("any", &self.any), ("none", &self.none)]
            .into_iter()
            .flat_map(|(field, names)| {
                names
                    .iter()
                    .enumerate()
                    .map(move |(i, name)| (format!("{field}.{i}"), name.as_str()))
            })
    }
}

/// The tags declared at build, a World resource.
#[derive(Debug, Default)]
pub(crate) struct TagDeclarations(Vec<String>);

/// A `tags` data entry: a JSON array of names, each declaring itself and
/// its dotted ancestors.
#[derive(Deserialize)]
#[serde(transparent)]
struct TagList(Tags);

/// `components.tags` as a scene writes it: the names checked, in the
/// written order, so a load's error names the index as written.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "Vec<String>")]
struct TagsDef(Vec<String>);

impl TryFrom<Vec<String>> for TagsDef {
    type Error = String;

    fn try_from(names: Vec<String>) -> Result<Self, Self::Error> {
        check_names(&names)?;
        Ok(Self(names))
    }
}

/// Whether `name` is declared: by a [`TagsPlugin`]'s build-time names or by
/// an entry of the `tags` data kind, either of which declares its dotted
/// ancestors too; else the [`MissingAsset`] a load reports, of kind `tag`,
/// at `field`. A kit item's filter check calls it.
pub(crate) fn check_declared(world: &World, field: &str, name: &str) -> Result<(), MissingAsset> {
    let by_build = world
        .get_resource::<TagDeclarations>()
        .is_some_and(|declared| declared.0.iter().any(|declared| covers(declared, name)));
    if by_build {
        return Ok(());
    }
    let by_data = world.get_resource::<DataRegistry>().is_some_and(|data| {
        data.ids(TAGS).any(|id| {
            data.get::<TagList>(id)
                .is_some_and(|list| list.0.iter().any(|declared| covers(declared, name)))
        })
    });
    if by_data {
        Ok(())
    } else {
        Err(MissingAsset::new(field, "tag", name))
    }
}

/// Registers the `tags` scene component and the `tags` data kind, and
/// declares the tags a load may name. A tag is declared by
/// [`with_tags`](Self::with_tags) at build or by an entry of the `tags`
/// kind, a JSON array of names under the manifest's `data.tags`; a declared
/// name declares its dotted ancestors. A scene component or a filter that
/// names an undeclared tag fails the load, naming the field and the tag, so
/// a misspelt tag never matches nothing in silence.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use tungsten_core::{ComponentRegistry, Plugin, Schedule, World};
/// use tungsten_kit::TagsPlugin;
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// TagsPlugin::new().with_tags(["team.player"]).build(&mut schedule, &mut world);
/// let components = world.resource::<ComponentRegistry>();
/// assert!(components.check_in("tags", &json!(["team.player"]), &world).is_ok());
/// assert!(components.check_in("tags", &json!(["team"]), &world).is_ok(), "an ancestor");
/// let error = components.check_in("tags", &json!(["team.playr"]), &world).unwrap_err();
/// assert!(error.to_string().contains("team.playr"));
/// ```
#[derive(Debug, Default, Clone)]
pub struct TagsPlugin {
    declared: Vec<String>,
}

impl TagsPlugin {
    /// A plugin declaring nothing at build; the `tags` data kind declares
    /// the rest.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The plugin, declaring `names` at build.
    ///
    /// # Panics
    ///
    /// When a name is not a tag name.
    #[must_use]
    #[track_caller]
    pub fn with_tags(mut self, names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.declared.extend(names.into_iter().map(Into::into));
        self.declared = normalize(std::mem::take(&mut self.declared))
            .unwrap_or_else(|error| panic!("TagsPlugin::with_tags: {error}"));
        self
    }
}

impl Plugin for TagsPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Tags>(world, "Tags");
        describe_kind(
            world,
            TAGS,
            "A JSON array of tag names; each declares itself and its dotted ancestors.",
        );
        describe(
            world,
            ComponentDescription::new(TAGS, "The entity's tags: dotted names the items filter on.")
                .field(FieldDescription::new(
                    "names",
                    FieldType::List(Box::new(FieldType::Tag)),
                    "the value itself: the names, each declared by `tags` data or a `TagsPlugin`",
                ))
                .genre(PLATFORMER)
                .genre(TOP_DOWN)
                .genre(SHMUP)
                .genre(GRID)
                .sample(r#"["team.player"]"#)
                .roundtrip(round_trip::<TagsDef>),
        );
        if let Some(declared) = world.get_resource_mut::<TagDeclarations>() {
            declared.0.extend(self.declared.iter().cloned());
        } else {
            world.insert_resource(TagDeclarations(self.declared.clone()));
        }
        world.register_data_kind::<TagList>(TAGS);
        world.register_component_with_assets(
            TAGS,
            |def: &TagsDef, world: &World| {
                for (i, name) in def.0.iter().enumerate() {
                    check_declared(world, &i.to_string(), name)?;
                }
                Ok(())
            },
            |def: TagsDef, _: &SpawnContext<'_>| (Tags::new(def.0),),
        );
    }
}

impl TagFilter {
    /// The filter in a line: `everything`, or its lists.
    pub(crate) fn summary(&self) -> String {
        if self.is_everything() {
            return "everything".to_string();
        }
        [("all", &self.all), ("any", &self.any), ("none", &self.none)]
            .into_iter()
            .filter(|(_, names)| !names.is_empty())
            .map(|(field, names)| format!("{field}: {}", names.join(", ")))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

impl Inspectable for Tags {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![("tags", self.names.join(", "))]
    }
}

#[cfg(test)]
#[path = "../tests/tags.rs"]
mod tests;
