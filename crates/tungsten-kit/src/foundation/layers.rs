//! Physics layers as a scene names them: lists whose entries are a layer's
//! number, 0 to 31, or its name in the manifests' `physics_layers`
//! ([`PhysicsLayers`], `D-229`), checked at load and resolved to bits at
//! spawn.

use serde::{Deserialize, Serialize};
use tungsten_core::physics::PhysicsLayers;
use tungsten_core::{MissingAsset, World};

use crate::foundation::describe::FieldType;

/// One entry of a layer list: a number or a name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum LayerRef {
    Number(LayerNumber),
    Name(String),
}

/// A physics layer's number, 0 to 31.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub(crate) struct LayerNumber(u32);

impl TryFrom<u32> for LayerNumber {
    type Error = String;

    fn try_from(layer: u32) -> Result<Self, String> {
        if layer < u32::BITS {
            Ok(Self(layer))
        } else {
            Err(format!("a layer must be 0 to 31 (got {layer})"))
        }
    }
}

impl From<LayerNumber> for u32 {
    fn from(layer: LayerNumber) -> Self {
        layer.0
    }
}

/// A layer list's description: numbers or names.
pub(crate) fn layer_list() -> FieldType {
    FieldType::List(Box::new(FieldType::Object(
        "a physics layer: its number, 0 to 31, or its physics_layers name",
    )))
}

/// The bits of the layers `list` names, one per layer, a name by `names`;
/// the first name `names` lacks, with its place, is the error.
fn bits(list: &[LayerRef], names: Option<&PhysicsLayers>) -> Result<u32, (usize, String)> {
    list.iter()
        .enumerate()
        .try_fold(0, |bits, (at, entry)| match entry {
            LayerRef::Number(layer) => Ok(bits | 1 << layer.0),
            LayerRef::Name(name) => names
                .and_then(|names| names.bit(name))
                .map(|bit| bits | bit)
                .ok_or_else(|| (at, name.clone())),
        })
}

/// Checks the list of field `field` against the World's layer names: the
/// first name they lack is missing at `field[i]`.
pub(crate) fn check_layers(
    field: &str,
    list: &[LayerRef],
    world: &World,
) -> Result<(), MissingAsset> {
    bits(list, world.get_resource::<PhysicsLayers>())
        .map(|_| ())
        .map_err(|(at, name)| MissingAsset::new(format!("{field}[{at}]"), "physics layer", name))
}

/// The bits of the layers `list` names, a check at load having found each
/// name; a name the World lacks adds none.
pub(crate) fn layer_bits(list: &[LayerRef], world: &World) -> u32 {
    let names = world.get_resource::<PhysicsLayers>();
    list.iter().fold(0, |bits, entry| match entry {
        LayerRef::Number(layer) => bits | 1 << layer.0,
        LayerRef::Name(name) => bits | names.and_then(|names| names.bit(name)).unwrap_or(0),
    })
}
