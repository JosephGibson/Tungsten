//! A prefab's runtime spawns as a table (`D-295`, save format 2): the spawns
//! of one area, one prefab and one set of saved components that share their
//! references and their layout, each a row of its ID and its components'
//! leaves, where a record repeats every component's name and field names.
//!
//! ```json
//! { "prefab": "ball",
//!   "columns": { "position": "[_,_]", "visibility": "{visible:_}" },
//!   "rows": [[7, 12.0, 40.5, true], [8, 16.0, 40.5, true]] }
//! ```
//!
//! A column's layout is its component's saved form as it serializes: `_` a
//! leaf, `[…]` a tuple, a fixed array or a tuple struct, `{name:…}` a struct
//! with its field names in order; a newtype is what it holds. Everything
//! else is one leaf holding the value `serde_json::to_value` gives: a
//! number, a string, a unit variant's name, an `Option`'s `null` or whole
//! value, a sequence, a map, an enum's tagged value. A component's value
//! built back from its layout and its leaves ([`Shape::build`]) is the one
//! `serde_json::to_value` gives.
//!
//! A table sits in its area's `spawns` beside the records, one element
//! ([`SpawnEntry`]); [`expand_tables`] writes a save's tables back as the
//! records they stand for.

use std::collections::BTreeMap;

use serde::de::Error as _;
use serde::ser::{self, Serialize, SerializeSeq, SerializeStruct};
use serde::{Deserialize, Deserializer, Serializer};
use serde_json::Value;

use super::id::SaveId;
use super::registry::Refs;
use super::world::SpawnRecord;

/// The fewest rows a table holds: under it a table's header costs more than
/// it saves, and a person reads records.
pub(crate) const TABLE_MIN: usize = 16;

/// One step of a component's layout as it serializes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Token {
    Leaf,
    Tuple,
    Struct,
    Field(&'static str),
    Close,
}

/// Where a component's leaves go as it serializes through [`Flat`]: the
/// layout is recorded for a table's first row and each later row is held to
/// it.
pub(crate) struct Cells<'a> {
    pub(crate) leaves: &'a mut Vec<Value>,
    pub(crate) layout: &'a mut Vec<Token>,
    /// The next token a row that is checked must give; `None` records.
    pub(crate) cursor: Option<usize>,
}

type Error = serde_json::Error;
type Json = serde_json::value::Serializer;
/// `serde_json`'s serializer to a [`Value`].
const JSON: Json = serde_json::value::Serializer;

impl Cells<'_> {
    #[inline]
    fn token(&mut self, token: Token) -> Result<(), Error> {
        let Some(cursor) = &mut self.cursor else {
            self.layout.push(token);
            return Ok(());
        };
        if self.layout.get(*cursor) == Some(&token) {
            *cursor += 1;
            Ok(())
        } else {
            Err(ser::Error::custom("the row does not fit its table"))
        }
    }

    #[inline]
    fn leaf(&mut self, value: Value) -> Result<(), Error> {
        self.token(Token::Leaf)?;
        self.leaves.push(value);
        Ok(())
    }

    /// Whether a checked row gave the whole layout.
    pub(crate) fn fits(&self) -> bool {
        self.cursor.is_none_or(|cursor| cursor == self.layout.len())
    }
}

/// The serializer a component's saved form runs through into leaves.
pub(crate) struct Flat<'c, 'a>(pub(crate) &'c mut Cells<'a>);

/// A value `serde_json` builds whole, then one leaf.
pub(crate) struct Whole<'c, 'a, S> {
    inner: S,
    cells: &'c mut Cells<'a>,
}

macro_rules! leaf {
    ($($name:ident($ty:ty)),*) => {$(
        #[inline]
        fn $name(self, v: $ty) -> Result<(), Error> {
            let value = JSON.$name(v)?;
            self.0.leaf(value)
        }
    )*};
}

impl<'c, 'a> Serializer for Flat<'c, 'a> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Whole<'c, 'a, <Json as Serializer>::SerializeSeq>;
    type SerializeTuple = Opened<'c, 'a>;
    type SerializeTupleStruct = Opened<'c, 'a>;
    type SerializeTupleVariant = Whole<'c, 'a, <Json as Serializer>::SerializeTupleVariant>;
    type SerializeMap = Whole<'c, 'a, <Json as Serializer>::SerializeMap>;
    type SerializeStruct = Opened<'c, 'a>;
    type SerializeStructVariant = Whole<'c, 'a, <Json as Serializer>::SerializeStructVariant>;

    leaf!(
        serialize_bool(bool),
        serialize_i8(i8),
        serialize_i16(i16),
        serialize_i32(i32),
        serialize_i64(i64),
        serialize_i128(i128),
        serialize_u8(u8),
        serialize_u16(u16),
        serialize_u32(u32),
        serialize_u64(u64),
        serialize_u128(u128),
        serialize_f32(f32),
        serialize_f64(f64),
        serialize_char(char),
        serialize_str(&str),
        serialize_bytes(&[u8])
    );

    fn serialize_none(self) -> Result<(), Error> {
        self.0.leaf(Value::Null)
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<(), Error> {
        let value = value.serialize(JSON)?;
        self.0.leaf(value)
    }

    fn serialize_unit(self) -> Result<(), Error> {
        self.0.leaf(Value::Null)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), Error> {
        self.0.leaf(Value::Null)
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
    ) -> Result<(), Error> {
        let value = JSON.serialize_unit_variant(name, index, variant)?;
        self.0.leaf(value)
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        let value = JSON.serialize_newtype_variant(name, index, variant, value)?;
        self.0.leaf(value)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Error> {
        Ok(Whole {
            inner: JSON.serialize_seq(len)?,
            cells: self.0,
        })
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Error> {
        self.0.token(Token::Tuple)?;
        Ok(Opened(self.0))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        self.0.token(Token::Tuple)?;
        Ok(Opened(self.0))
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        Ok(Whole {
            inner: JSON.serialize_tuple_variant(name, index, variant, len)?,
            cells: self.0,
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Ok(Whole {
            inner: JSON.serialize_map(len)?,
            cells: self.0,
        })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Error> {
        self.0.token(Token::Struct)?;
        Ok(Opened(self.0))
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        Ok(Whole {
            inner: JSON.serialize_struct_variant(name, index, variant, len)?,
            cells: self.0,
        })
    }
}

/// A struct or tuple opened in place.
pub(crate) struct Opened<'c, 'a>(&'c mut Cells<'a>);

impl ser::SerializeTuple for Opened<'_, '_> {
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        value.serialize(Flat(self.0))
    }

    #[inline]
    fn end(self) -> Result<(), Error> {
        self.0.token(Token::Close)
    }
}

impl ser::SerializeTupleStruct for Opened<'_, '_> {
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        value.serialize(Flat(self.0))
    }

    #[inline]
    fn end(self) -> Result<(), Error> {
        self.0.token(Token::Close)
    }
}

impl SerializeStruct for Opened<'_, '_> {
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.0.token(Token::Field(key))?;
        value.serialize(Flat(self.0))
    }

    #[inline]
    fn end(self) -> Result<(), Error> {
        self.0.token(Token::Close)
    }
}

impl<S: SerializeSeq<Ok = Value, Error = Error>> SerializeSeq for Whole<'_, '_, S> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        self.inner.serialize_element(value)
    }

    fn end(self) -> Result<(), Error> {
        let value = self.inner.end()?;
        self.cells.leaf(value)
    }
}

impl<S: ser::SerializeTupleVariant<Ok = Value, Error = Error>> ser::SerializeTupleVariant
    for Whole<'_, '_, S>
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        self.inner.serialize_field(value)
    }

    fn end(self) -> Result<(), Error> {
        let value = self.inner.end()?;
        self.cells.leaf(value)
    }
}

impl<S: ser::SerializeMap<Ok = Value, Error = Error>> ser::SerializeMap for Whole<'_, '_, S> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Error> {
        self.inner.serialize_key(key)
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        self.inner.serialize_value(value)
    }

    fn end(self) -> Result<(), Error> {
        let value = self.inner.end()?;
        self.cells.leaf(value)
    }
}

impl<S: ser::SerializeStructVariant<Ok = Value, Error = Error>> ser::SerializeStructVariant
    for Whole<'_, '_, S>
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.inner.serialize_field(key, value)
    }

    fn end(self) -> Result<(), Error> {
        let value = self.inner.end()?;
        self.cells.leaf(value)
    }
}

/// A column's layout: what its component's saved form opens into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Shape {
    Leaf,
    Tuple(Vec<Shape>),
    /// Each field's name and shape, in serialization order.
    Struct(Vec<(String, Shape)>),
}

/// Whether a layout's text can hold `name` as a field: letters, digits and
/// `_`, at least one.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

impl Shape {
    /// The shape `tokens` describe, one component's from its first token to
    /// its last. `None` for a field whose name the layout's text cannot
    /// hold, or for tokens that are no whole shape.
    pub(crate) fn from_tokens(tokens: &[Token]) -> Option<Self> {
        let mut rest = tokens.iter();
        let shape = Self::next(&mut rest)?;
        rest.next().is_none().then_some(shape)
    }

    fn next(tokens: &mut std::slice::Iter<'_, Token>) -> Option<Self> {
        match tokens.next()? {
            Token::Leaf => Some(Self::Leaf),
            Token::Tuple => {
                let mut items = Vec::new();
                loop {
                    if tokens.as_slice().first() == Some(&Token::Close) {
                        tokens.next();
                        return Some(Self::Tuple(items));
                    }
                    items.push(Self::next(tokens)?);
                }
            }
            Token::Struct => {
                let mut fields = Vec::new();
                loop {
                    match tokens.next()? {
                        Token::Close => return Some(Self::Struct(fields)),
                        Token::Field(name) if plain_name(name) => {
                            fields.push(((*name).to_owned(), Self::next(tokens)?));
                        }
                        _ => return None,
                    }
                }
            }
            Token::Field(_) | Token::Close => None,
        }
    }

    /// The leaves a value of this shape holds.
    pub(crate) fn leaves(&self) -> usize {
        match self {
            Self::Leaf => 1,
            Self::Tuple(items) => items.iter().map(Self::leaves).sum(),
            Self::Struct(fields) => fields.iter().map(|(_, shape)| shape.leaves()).sum(),
        }
    }

    /// The value of this shape over the next [`leaves`](Self::leaves) of
    /// `leaves`: what `serde_json::to_value` gives the component they came
    /// from. A leaf the iterator lacks reads `null`.
    pub(crate) fn build<'v>(&self, leaves: &mut impl Iterator<Item = &'v Value>) -> Value {
        match self {
            Self::Leaf => leaves.next().cloned().unwrap_or(Value::Null),
            Self::Tuple(items) => {
                Value::Array(items.iter().map(|item| item.build(leaves)).collect())
            }
            Self::Struct(fields) => Value::Object(
                fields
                    .iter()
                    .map(|(name, shape)| (name.clone(), shape.build(leaves)))
                    .collect(),
            ),
        }
    }

    /// The layout's text: `_`, `[…]` and `{name:…}`.
    pub(crate) fn text(&self) -> String {
        let mut text = String::new();
        self.write(&mut text);
        text
    }

    fn write(&self, text: &mut String) {
        match self {
            Self::Leaf => text.push('_'),
            Self::Tuple(items) => {
                text.push('[');
                for (at, item) in items.iter().enumerate() {
                    if at > 0 {
                        text.push(',');
                    }
                    item.write(text);
                }
                text.push(']');
            }
            Self::Struct(fields) => {
                text.push('{');
                for (at, (name, shape)) in fields.iter().enumerate() {
                    if at > 0 {
                        text.push(',');
                    }
                    text.push_str(name);
                    text.push(':');
                    shape.write(text);
                }
                text.push('}');
            }
        }
    }

    /// The shape `text` spells.
    pub(crate) fn parse(text: &str) -> Result<Self, String> {
        let mut rest = text.as_bytes();
        let shape = Self::parse_next(&mut rest)
            .ok_or_else(|| format!("`{text}` is no layout: `_`, `[…]` or `{{name:…}}`"))?;
        if rest.is_empty() {
            Ok(shape)
        } else {
            Err(format!("layout `{text}` goes on past its shape"))
        }
    }

    fn parse_next(rest: &mut &[u8]) -> Option<Self> {
        let (&first, after) = rest.split_first()?;
        *rest = after;
        match first {
            b'_' => Some(Self::Leaf),
            b'[' => {
                let mut items = Vec::new();
                loop {
                    if rest.first() == Some(&b']') {
                        *rest = &rest[1..];
                        return Some(Self::Tuple(items));
                    }
                    if !items.is_empty() {
                        Self::expect(rest, b',')?;
                    }
                    items.push(Self::parse_next(rest)?);
                }
            }
            b'{' => {
                let mut fields = Vec::new();
                loop {
                    if rest.first() == Some(&b'}') {
                        *rest = &rest[1..];
                        return Some(Self::Struct(fields));
                    }
                    if !fields.is_empty() {
                        Self::expect(rest, b',')?;
                    }
                    let end = rest.iter().position(|&byte| byte == b':')?;
                    let name = std::str::from_utf8(&rest[..end]).ok()?;
                    if !plain_name(name) {
                        return None;
                    }
                    *rest = &rest[end + 1..];
                    fields.push((name.to_owned(), Self::parse_next(rest)?));
                }
            }
            _ => None,
        }
    }

    fn expect(rest: &mut &[u8], byte: u8) -> Option<()> {
        let (&first, after) = rest.split_first()?;
        (first == byte).then(|| *rest = after)
    }
}

/// One table: the spawns of one area, one prefab and one set of saved
/// components that share their references and their layout.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SpawnTable {
    pub(crate) prefab: String,
    /// Each column's component and its layout, by component name.
    pub(crate) columns: Vec<(String, Shape)>,
    /// The references every row holds.
    pub(crate) refs: BTreeMap<String, Refs>,
    /// Each row's ID, ascending.
    pub(crate) ids: Vec<SaveId>,
    /// The rows' leaves, `width` a row, in the columns' order.
    pub(crate) cells: Vec<Value>,
    pub(crate) width: usize,
}

impl SpawnTable {
    /// Row `row`'s leaves.
    fn row(&self, row: usize) -> &[Value] {
        &self.cells[row * self.width..(row + 1) * self.width]
    }

    /// The record row `row` stands for.
    pub(crate) fn record(&self, row: usize) -> SpawnRecord {
        let mut leaves = self.row(row).iter();
        SpawnRecord {
            id: self.ids[row],
            prefab: self.prefab.clone(),
            components: self
                .columns
                .iter()
                .map(|(name, shape)| (name.clone(), shape.build(&mut leaves)))
                .collect(),
            refs: self.refs.clone(),
        }
    }

    /// Each row's record, by ID.
    pub(crate) fn records(&self) -> impl Iterator<Item = SpawnRecord> + '_ {
        (0..self.ids.len()).map(|row| self.record(row))
    }

    /// Sorts the rows by ID; a table built in ID order moves nothing.
    pub(crate) fn sort_rows(&mut self) {
        if self.ids.is_sorted() {
            return;
        }
        let mut order: Vec<usize> = (0..self.ids.len()).collect();
        order.sort_by_key(|&row| self.ids[row]);
        let mut cells = Vec::with_capacity(self.cells.len());
        for &row in &order {
            cells.extend_from_slice(self.row(row));
        }
        self.ids = order.iter().map(|&row| self.ids[row]).collect();
        self.cells = cells;
    }

    /// The order tables take after an area's records: by prefab, then by
    /// their columns.
    pub(crate) fn order(&self, other: &Self) -> std::cmp::Ordering {
        fn names(table: &SpawnTable) -> Vec<&str> {
            table
                .columns
                .iter()
                .map(|(name, _)| name.as_str())
                .collect()
        }
        (self.prefab.as_str(), names(self), self.ids.first()).cmp(&(
            other.prefab.as_str(),
            names(other),
            other.ids.first(),
        ))
    }
}

struct Row<'a>(SaveId, &'a [Value]);

impl Serialize for Row<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(1 + self.1.len()))?;
        seq.serialize_element(&self.0)?;
        for leaf in self.1 {
            seq.serialize_element(leaf)?;
        }
        seq.end()
    }
}

struct Rows<'a>(&'a SpawnTable);

impl Serialize for Rows<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let table = self.0;
        let mut seq = serializer.serialize_seq(Some(table.ids.len()))?;
        for (row, &id) in table.ids.iter().enumerate() {
            seq.serialize_element(&Row(id, table.row(row)))?;
        }
        seq.end()
    }
}

impl Serialize for SpawnTable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let columns: BTreeMap<&str, String> = self
            .columns
            .iter()
            .map(|(name, shape)| (name.as_str(), shape.text()))
            .collect();
        let fields = 3 + usize::from(!self.refs.is_empty());
        let mut table = serializer.serialize_struct("SpawnTable", fields)?;
        table.serialize_field("prefab", &self.prefab)?;
        table.serialize_field("columns", &columns)?;
        if self.refs.is_empty() {
            table.skip_field("refs")?;
        } else {
            table.serialize_field("refs", &self.refs)?;
        }
        table.serialize_field("rows", &Rows(self))?;
        table.end()
    }
}

/// A spawn element as a file holds it: a record's fields or a table's.
#[derive(Deserialize)]
struct Element {
    id: Option<SaveId>,
    prefab: Option<String>,
    components: Option<BTreeMap<String, Value>>,
    #[serde(default)]
    refs: BTreeMap<String, Refs>,
    columns: Option<BTreeMap<String, String>>,
    rows: Option<Vec<Vec<Value>>>,
}

impl Element {
    /// The table these fields spell: each layout parsed and each row held
    /// to the columns' width.
    fn table(self) -> Result<SpawnTable, String> {
        let prefab = self.prefab.ok_or("missing field `prefab`")?;
        let layouts = self.columns.ok_or("missing field `columns`")?;
        let rows = self.rows.ok_or("missing field `rows`")?;
        let mut columns = Vec::with_capacity(layouts.len());
        for (name, layout) in layouts {
            let shape =
                Shape::parse(&layout).map_err(|err| format!("table column `{name}`: {err}"))?;
            columns.push((name, shape));
        }
        let width: usize = columns.iter().map(|(_, shape)| shape.leaves()).sum();
        let mut ids = Vec::with_capacity(rows.len());
        let mut cells = Vec::with_capacity(rows.len() * width);
        for (at, row) in rows.into_iter().enumerate() {
            if row.len() != 1 + width {
                return Err(format!(
                    "table row {at} of `{prefab}` holds {} cells; its columns take an ID and {width}",
                    row.len()
                ));
            }
            let mut row = row.into_iter();
            let id = row.next().expect("a row holds its ID");
            let id = SaveId::deserialize(id)
                .map_err(|err| format!("table row {at} of `{prefab}`: its ID: {err}"))?;
            ids.push(id);
            cells.extend(row);
        }
        let mut table = SpawnTable {
            prefab,
            columns,
            refs: self.refs,
            ids,
            cells,
            width,
        };
        table.sort_rows();
        Ok(table)
    }
}

impl<'de> Deserialize<'de> for SpawnTable {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Element::deserialize(deserializer)?
            .table()
            .map_err(D::Error::custom)
    }
}

/// One element of an area's `spawns`: a spawn in full, or a table of them.
/// The table is boxed, so an element is a record's size.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SpawnEntry {
    Record(SpawnRecord),
    Table(Box<SpawnTable>),
}

impl SpawnEntry {
    /// The order a capture writes an area's spawns in: its records by ID,
    /// then its tables by prefab and columns.
    pub(crate) fn order(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match (self, other) {
            (Self::Record(a), Self::Record(b)) => a.id.cmp(&b.id),
            (Self::Record(_), Self::Table(_)) => Ordering::Less,
            (Self::Table(_), Self::Record(_)) => Ordering::Greater,
            (Self::Table(a), Self::Table(b)) => a.order(b),
        }
    }
}

impl Serialize for SpawnEntry {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Record(record) => record.serialize(serializer),
            Self::Table(table) => table.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for SpawnEntry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let element = Element::deserialize(deserializer)?;
        if element.rows.is_some() || element.columns.is_some() {
            return element
                .table()
                .map(|table| Self::Table(Box::new(table)))
                .map_err(D::Error::custom);
        }
        Ok(Self::Record(SpawnRecord {
            id: element.id.ok_or_else(|| D::Error::missing_field("id"))?,
            prefab: element
                .prefab
                .ok_or_else(|| D::Error::missing_field("prefab"))?,
            components: element
                .components
                .ok_or_else(|| D::Error::missing_field("components"))?,
            refs: element.refs,
        }))
    }
}

/// An area's spawns as records, in the order a restore applies them: an
/// area of records alone in its file's order, as before tables; one with a
/// table by ID across its records and its rows.
pub(crate) fn records_in_order(entries: &[SpawnEntry]) -> Vec<std::borrow::Cow<'_, SpawnRecord>> {
    use std::borrow::Cow;
    let mut records = Vec::with_capacity(entries.len());
    let mut any_table = false;
    for entry in entries {
        match entry {
            SpawnEntry::Record(record) => records.push(Cow::Borrowed(record)),
            SpawnEntry::Table(table) => {
                any_table = true;
                records.extend(table.records().map(Cow::Owned));
            }
        }
    }
    if any_table {
        records.sort_by_key(|record| record.id);
    }
    records
}

/// `entries` with each table written back as its records, by ID across
/// them; entries with no table come back as they are.
pub(crate) fn expanded(entries: &[SpawnEntry]) -> Vec<SpawnEntry> {
    if !entries
        .iter()
        .any(|entry| matches!(entry, SpawnEntry::Table(_)))
    {
        return entries.to_vec();
    }
    records_in_order(entries)
        .into_iter()
        .map(|record| SpawnEntry::Record(record.into_owned()))
        .collect()
}

/// Rewrites a world save's tables as the records they stand for, in place:
/// `world` is a [`WorldSave`](super::WorldSave) as JSON, and afterwards
/// each of its areas' `spawns` holds records alone, by ID where it held a
/// table, so a migration step, or a test that walks a capture, reads one
/// shape. A save with no table is left as it is.
///
/// # Errors
///
/// A line naming what is wrong: a value that is no world save, or a table
/// or a spawn beside one that does not read.
pub fn expand_tables(world: &mut Value) -> Result<(), String> {
    fn expand(area: &mut Value, name: &str) -> Result<(), String> {
        let Some(spawns) = area.get_mut("spawns") else {
            return Ok(());
        };
        let Some(list) = spawns.as_array() else {
            return Err(format!("{name}'s `spawns` is no list"));
        };
        let table = |spawn: &Value| spawn.get("rows").is_some() || spawn.get("columns").is_some();
        if !list.iter().any(table) {
            return Ok(());
        }
        let entries: Vec<SpawnEntry> = serde_json::from_value(spawns.take())
            .map_err(|err| format!("{name}'s `spawns`: {err}"))?;
        *spawns = serde_json::to_value(expanded(&entries))
            .map_err(|err| format!("{name}'s `spawns`: {err}"))?;
        Ok(())
    }
    let no_save = || "not a world save: it holds no `global` record and `areas` map".to_owned();
    let save = world.as_object_mut().ok_or_else(no_save)?;
    expand(
        save.get_mut("global").ok_or_else(no_save)?,
        "the global record",
    )?;
    let areas = save
        .get_mut("areas")
        .and_then(Value::as_object_mut)
        .ok_or_else(no_save)?;
    for (name, area) in areas {
        expand(area, &format!("area `{name}`"))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/save/table.rs"]
mod tests;
