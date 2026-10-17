//! Field descriptions: what each scene component and data kind takes, kept
//! in the [`KitDescriptions`] resource and rendered into the components
//! snapshot (`api/tungsten-kit-components.txt`), which `just api-check`
//! compares, so a changed default, a renamed field or a removed variant
//! shows as a diff (`D-107`, as the kit foundations amend it).

use std::fmt::Write as _;

use serde_json::Value;
use tungsten_core::World;

/// What a scene component takes: its summary, fields, events, genres and
/// a minimal sample, with a round-trip hook over its private DTO. Each kit
/// plugin adds one per scene component it registers, and one per shape, a
/// named object its fields refer to ([`FieldType::Shape`]); a game's own
/// plugin may add its own.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{ComponentDescription, FieldDescription, FieldType, Bound};
///
/// let description = ComponentDescription::new("torch", "A light that burns down.")
///     .field(
///         FieldDescription::new(
///             "seconds",
///             FieldType::F32 { min: Bound::Exclusive(0.0), max: Bound::Open },
///             "how long it burns",
///         )
///         .default("10.0"),
///     )
///     .genre("platformer")
///     .sample("{}");
/// assert_eq!(description.name(), "torch");
/// assert_eq!(description.fields()[0].default_json(), Some("10.0"));
/// ```
#[derive(Debug, Clone)]
pub struct ComponentDescription {
    name: &'static str,
    summary: &'static str,
    fields: Vec<FieldDescription>,
    events: Vec<&'static str>,
    genres: Vec<&'static str>,
    sample: Option<&'static str>,
    roundtrip: Option<fn(Value) -> Result<Value, String>>,
    one_of: bool,
}

impl ComponentDescription {
    /// A description of the scene component `name` with `summary`.
    #[must_use]
    pub fn new(name: &'static str, summary: &'static str) -> Self {
        Self {
            name,
            summary,
            fields: Vec::new(),
            events: Vec::new(),
            genres: Vec::new(),
            sample: None,
            roundtrip: None,
            one_of: false,
        }
    }

    /// The description, with `field` after the fields so far.
    #[must_use]
    pub fn field(mut self, field: FieldDescription) -> Self {
        self.fields.push(field);
        self
    }

    /// The description, also naming an event the component's system sends.
    #[must_use]
    pub fn event(mut self, event: &'static str) -> Self {
        self.events.push(event);
        self
    }

    /// The description, also naming a genre whose fixture uses the
    /// component.
    #[must_use]
    pub fn genre(mut self, genre: &'static str) -> Self {
        self.genres.push(genre);
        self
    }

    /// The description, with a minimal sample value as JSON: every field
    /// with no default given.
    #[must_use]
    pub fn sample(mut self, json: &'static str) -> Self {
        self.sample = Some(json);
        self
    }

    /// The description, with the hook that deserializes a value as the
    /// component's DTO and serializes it back, so the snapshot test holds
    /// the description and the DTO together.
    #[must_use]
    pub fn roundtrip(mut self, hook: fn(Value) -> Result<Value, String>) -> Self {
        self.roundtrip = Some(hook);
        self
    }

    /// The description, for a shape whose value is an object holding
    /// exactly one of its fields, such as a cue action: each field is one
    /// form, and the description tests probe each one alone.
    #[must_use]
    pub fn one_of(mut self) -> Self {
        self.one_of = true;
        self
    }

    /// Whether the value holds exactly one of its fields
    /// ([`one_of`](Self::one_of)).
    #[must_use]
    pub fn is_one_of(&self) -> bool {
        self.one_of
    }

    /// The scene name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The summary.
    #[must_use]
    pub fn summary(&self) -> &'static str {
        self.summary
    }

    /// The fields, in the order described.
    #[must_use]
    pub fn fields(&self) -> &[FieldDescription] {
        &self.fields
    }

    /// The events the component's system sends.
    #[must_use]
    pub fn events(&self) -> &[&'static str] {
        &self.events
    }

    /// The genres whose fixtures use the component.
    #[must_use]
    pub fn genres(&self) -> &[&'static str] {
        &self.genres
    }

    /// The minimal sample, as JSON.
    #[must_use]
    pub fn sample_json(&self) -> Option<&'static str> {
        self.sample
    }

    /// `value` through the round-trip hook; `None` without one.
    #[must_use]
    pub fn round_trip(&self, value: Value) -> Option<Result<Value, String>> {
        self.roundtrip.map(|hook| hook(value))
    }

    fn render(&self, out: &mut String, heading: &str) {
        let _ = writeln!(out, "{heading} {}: {}", self.name, self.summary);
        if self.one_of {
            let _ = writeln!(out, "  exactly one of its fields");
        }
        for field in &self.fields {
            let _ = write!(out, "  field {}: {}", field.name, field.ty.render());
            if let Some(default) = field.default {
                let _ = write!(out, " = {default}");
            }
            let _ = writeln!(out, " \u{2014} {}", field.doc);
        }
        if !self.events.is_empty() {
            let _ = writeln!(out, "  events: {}", self.events.join(", "));
        }
        if !self.genres.is_empty() {
            let _ = writeln!(out, "  genres: {}", self.genres.join(", "));
        }
        if let Some(sample) = self.sample {
            let _ = writeln!(out, "  sample: {sample}");
        }
    }
}

/// One field of a scene component: its name, type, doc and default.
#[derive(Debug, Clone)]
pub struct FieldDescription {
    name: &'static str,
    ty: FieldType,
    doc: &'static str,
    default: Option<&'static str>,
}

impl FieldDescription {
    /// Field `name` of type `ty`, with `doc`, required until
    /// [`default`](Self::default) gives it one.
    #[must_use]
    pub fn new(name: &'static str, ty: FieldType, doc: &'static str) -> Self {
        Self {
            name,
            ty,
            doc,
            default: None,
        }
    }

    /// The field, with `default` as its JSON value when the scene leaves
    /// it out.
    #[must_use]
    pub fn default(mut self, default: &'static str) -> Self {
        self.default = Some(default);
        self
    }

    /// The field's name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The field's type.
    #[must_use]
    pub fn ty(&self) -> &FieldType {
        &self.ty
    }

    /// The field's doc.
    #[must_use]
    pub fn doc(&self) -> &'static str {
        self.doc
    }

    /// The field's default as JSON, when it has one.
    #[must_use]
    pub fn default_json(&self) -> Option<&'static str> {
        self.default
    }
}

/// A bound of a numeric field.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bound {
    /// No bound.
    Open,
    /// The value itself is allowed.
    Inclusive(f32),
    /// The value itself is refused.
    Exclusive(f32),
}

/// A field's type, as the snapshot prints it; later milestones add kinds.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum FieldType {
    /// `true` or `false`.
    Bool,
    /// A number within its bounds.
    F32 {
        /// The least value.
        min: Bound,
        /// The greatest value.
        max: Bound,
    },
    /// A whole number within its bounds, each inclusive when given.
    U32 {
        /// The least value.
        min: Option<u32>,
        /// The greatest value.
        max: Option<u32>,
    },
    /// A signed whole number within its bounds, each inclusive when given.
    I64 {
        /// The least value.
        min: Option<i64>,
        /// The greatest value.
        max: Option<i64>,
    },
    /// Any text.
    Text,
    /// `[x, y]`.
    Vec2,
    /// `[r, g, b, a]`, linear.
    Color,
    /// A list of the inner type.
    List(Box<FieldType>),
    /// One of these names.
    Enum(&'static [&'static str]),
    /// A manifest prefab ID.
    Prefab,
    /// A declared tag name.
    Tag,
    /// A tag filter: `{ "all": [..], "any": [..], "none": [..] }`.
    Filter,
    /// An object written as this text describes it.
    Object(&'static str),
    /// An object of the shape of this name, described in
    /// [`KitDescriptions::shapes`].
    Shape(&'static str),
}

impl FieldType {
    fn render(&self) -> String {
        match self {
            FieldType::Bool => "bool".to_string(),
            FieldType::F32 { min, max } => {
                let mut text = "f32".to_string();
                let bounds: Vec<String> = [(">", min), ("<", max)]
                    .into_iter()
                    .filter_map(|(sign, bound)| match bound {
                        Bound::Open => None,
                        Bound::Inclusive(value) => Some(format!("{sign}= {value}")),
                        Bound::Exclusive(value) => Some(format!("{sign} {value}")),
                    })
                    .collect();
                if !bounds.is_empty() {
                    let _ = write!(text, " ({})", bounds.join(", "));
                }
                text
            }
            FieldType::U32 { min, max } => whole("u32", *min, *max),
            FieldType::I64 { min, max } => whole("i64", *min, *max),
            FieldType::Text => "text".to_string(),
            FieldType::Vec2 => "[x, y]".to_string(),
            FieldType::Color => "[r, g, b, a]".to_string(),
            FieldType::List(inner) => format!("list of {}", inner.render()),
            FieldType::Enum(variants) => format!("one of {}", variants.join(" | ")),
            FieldType::Prefab => "prefab ID".to_string(),
            FieldType::Tag => "tag".to_string(),
            FieldType::Filter => "tag filter".to_string(),
            FieldType::Object(text) => (*text).to_string(),
            FieldType::Shape(name) => format!("shape {name}"),
        }
    }
}

/// A whole-number type's name with its inclusive bounds.
fn whole<T: std::fmt::Display>(name: &str, min: Option<T>, max: Option<T>) -> String {
    let mut text = name.to_string();
    let bounds: Vec<String> = [(">=", min), ("<=", max)]
        .into_iter()
        .filter_map(|(sign, bound)| bound.map(|value| format!("{sign} {value}")))
        .collect();
    if !bounds.is_empty() {
        let _ = write!(text, " ({})", bounds.join(", "));
    }
    text
}

/// The descriptions the kit's plugins registered, a World resource, sorted
/// by name, with the shapes their fields refer to and the data kinds
/// beside them; [`render`](Self::render) is the components snapshot. A
/// game's plugin adds its own through [`add`](Self::add),
/// [`add_shape`](Self::add_shape) and [`add_kind`](Self::add_kind).
#[derive(Debug, Default)]
pub struct KitDescriptions {
    components: Vec<ComponentDescription>,
    shapes: Vec<ComponentDescription>,
    kinds: Vec<(&'static str, &'static str)>,
}

impl KitDescriptions {
    /// No descriptions.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `description`, in name order.
    ///
    /// # Panics
    ///
    /// When a description of that name is already there.
    #[track_caller]
    pub fn add(&mut self, description: ComponentDescription) {
        match self
            .components
            .binary_search_by(|d| d.name.cmp(description.name))
        {
            Ok(_) => panic!(
                "KitDescriptions::add: `{}` is already described",
                description.name
            ),
            Err(at) => self.components.insert(at, description),
        }
    }

    /// Adds `description` as a shape: a named object that a field refers
    /// to with [`FieldType::Shape`], such as a cue action, in name order.
    ///
    /// # Panics
    ///
    /// When a shape of that name is already there.
    #[track_caller]
    pub fn add_shape(&mut self, description: ComponentDescription) {
        match self
            .shapes
            .binary_search_by(|d| d.name.cmp(description.name))
        {
            Ok(_) => panic!(
                "KitDescriptions::add_shape: `{}` is already described",
                description.name
            ),
            Err(at) => self.shapes.insert(at, description),
        }
    }

    /// Adds the data kind `kind` with `summary`, in name order.
    ///
    /// # Panics
    ///
    /// When that kind is already there.
    #[track_caller]
    pub fn add_kind(&mut self, kind: &'static str, summary: &'static str) {
        match self.kinds.binary_search_by(|(name, _)| name.cmp(&kind)) {
            Ok(_) => panic!("KitDescriptions::add_kind: `{kind}` is already described"),
            Err(at) => self.kinds.insert(at, (kind, summary)),
        }
    }

    /// The description of the scene component `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ComponentDescription> {
        self.components
            .binary_search_by(|d| d.name.cmp(name))
            .ok()
            .map(|at| &self.components[at])
    }

    /// The descriptions, by name.
    pub fn iter(&self) -> impl Iterator<Item = &ComponentDescription> {
        self.components.iter()
    }

    /// The description of the shape `name`.
    #[must_use]
    pub fn get_shape(&self, name: &str) -> Option<&ComponentDescription> {
        self.shapes
            .binary_search_by(|d| d.name.cmp(name))
            .ok()
            .map(|at| &self.shapes[at])
    }

    /// The shapes, by name.
    pub fn shapes(&self) -> impl Iterator<Item = &ComponentDescription> {
        self.shapes.iter()
    }

    /// The data kinds and their summaries, by name.
    pub fn kinds(&self) -> impl Iterator<Item = (&'static str, &'static str)> {
        self.kinds.iter().copied()
    }

    /// The snapshot: one `component` line per description with a line per
    /// field, its events, genres and sample, then a `shape` line per shape
    /// in the same form, then a `kind` line per data kind.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        for description in &self.components {
            description.render(&mut out, "component");
        }
        for description in &self.shapes {
            description.render(&mut out, "shape");
        }
        for (kind, summary) in &self.kinds {
            let _ = writeln!(out, "kind {kind}: {summary}");
        }
        out
    }
}

/// Adds `description` to the World's [`KitDescriptions`], inserting the
/// resource first when the World has none; a name already described is
/// left as it is, so a kit plugin built twice describes once, as it
/// registers once.
pub(crate) fn describe(world: &mut World, description: ComponentDescription) {
    let descriptions = descriptions_mut(world);
    if descriptions.get(description.name()).is_none() {
        descriptions.add(description);
    }
}

/// Adds `description` to the World's [`KitDescriptions`] as a shape, once.
pub(crate) fn describe_shape(world: &mut World, description: ComponentDescription) {
    let descriptions = descriptions_mut(world);
    if descriptions.get_shape(description.name()).is_none() {
        descriptions.add_shape(description);
    }
}

/// Adds the data kind to the World's [`KitDescriptions`], once.
pub(crate) fn describe_kind(world: &mut World, kind: &'static str, summary: &'static str) {
    let descriptions = descriptions_mut(world);
    if !descriptions.kinds().any(|(name, _)| name == kind) {
        descriptions.add_kind(kind, summary);
    }
}

fn descriptions_mut(world: &mut World) -> &mut KitDescriptions {
    if !world.has_resource::<KitDescriptions>() {
        world.insert_resource(KitDescriptions::new());
    }
    world.resource_mut::<KitDescriptions>()
}

/// The round-trip hook of a DTO `D`: deserialize, serialize back.
pub(crate) fn round_trip<D: serde::de::DeserializeOwned + serde::Serialize>(
    value: Value,
) -> Result<Value, String> {
    let dto: D = serde_json::from_value(value).map_err(|e| e.to_string())?;
    serde_json::to_value(dto).map_err(|e| e.to_string())
}

/// Core's `Easing` names, as its snake-case serde writes them.
pub(crate) const EASING_NAMES: &[&str] = &[
    "linear",
    "quad_in",
    "quad_out",
    "quad_in_out",
    "cubic_in",
    "cubic_out",
    "cubic_in_out",
    "quart_in",
    "quart_out",
    "quart_in_out",
    "sine_in",
    "sine_out",
    "sine_in_out",
    "expo_in",
    "expo_out",
    "expo_in_out",
    "back_in",
    "back_out",
    "back_in_out",
    "bounce_in",
    "bounce_out",
    "bounce_in_out",
];

/// The four reference genres.
pub(crate) const PLATFORMER: &str = "platformer";
pub(crate) const TOP_DOWN: &str = "top_down";
pub(crate) const SHMUP: &str = "shmup";
pub(crate) const GRID: &str = "grid";

#[cfg(test)]
#[path = "../tests/describe.rs"]
mod tests;
