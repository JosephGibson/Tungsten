//! Areas and their tiles: a level reference's map spawned with its objects
//! under their names, its edits and damage in a save, and an area kept
//! while the World has not loaded it.

use std::collections::BTreeMap;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::id::{NameError, SaveId, SaveState, name};
use super::world::{RestoreError, RestoreReport};
use crate::assets::{EMPTY_TILE, PrefabError, TileIndex, TilemapInstance, TilemapRegistry};
use crate::ecs::{Bundle, Entity, World};

/// One map's tiles in a save: its layers' names at the capture, each
/// edited cell's layer, column, row and tile by its sprite ID (`null` for
/// none), and each damaged cell's health.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct TileRecord {
    pub(crate) layers: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) edited: Vec<(usize, u32, u32, Option<String>)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) damaged: Vec<(usize, u32, u32, f32)>,
}

/// Each of `maps` the game edited or damaged, as a save writes it.
pub(crate) fn capture_tiles<'a>(
    world: &World,
    maps: impl Iterator<Item = &'a String>,
) -> BTreeMap<String, TileRecord> {
    let mut tiles = BTreeMap::new();
    let Some(registry) = world.get_resource::<TilemapRegistry>() else {
        return tiles;
    };
    for map in maps {
        let Some(data) = registry.get(map) else {
            continue;
        };
        let sprite = |tile: TileIndex| -> Option<String> {
            usize::try_from(tile)
                .ok()
                .and_then(|index| data.tileset.get(index).cloned())
        };
        let record = TileRecord {
            layers: data.layers.iter().map(|layer| layer.name.clone()).collect(),
            edited: registry
                .edited_cells(map)
                .map(|(cell, tile)| (cell.layer, cell.col, cell.row, sprite(tile)))
                .collect(),
            damaged: registry
                .damaged_cells(map)
                .map(|(cell, health)| (cell.layer, cell.col, cell.row, health))
                .collect(),
        };
        if !record.edited.is_empty() || !record.damaged.is_empty() {
            tiles.insert(map.clone(), record);
        }
    }
    tiles
}

/// A map's checked writes: its edits, then its damage.
pub(crate) struct TilePlan {
    map: String,
    edits: Vec<(usize, u32, u32, TileIndex)>,
    damage: Vec<(usize, u32, u32, f32)>,
}

/// Checks `tiles` against the maps as the World holds them: an edit or a
/// damaged cell past its map's layers or cells, on a layer whose name
/// changed, or naming a sprite its tileset lacks is dropped with a line.
pub(crate) fn plan_tiles(
    world: &World,
    area: &str,
    tiles: &BTreeMap<String, TileRecord>,
    report: &mut RestoreReport,
) -> Vec<TilePlan> {
    let mut plans = Vec::new();
    let Some(registry) = world.get_resource::<TilemapRegistry>() else {
        if !tiles.is_empty() {
            report.dropped.push(format!(
                "area `{area}`'s tiles: the World has no tilemap registry"
            ));
        }
        return plans;
    };
    for (map, record) in tiles {
        let Some(data) = registry.get(map) else {
            report
                .dropped
                .push(format!("area `{area}`'s map `{map}`: not registered"));
            continue;
        };
        let at = |layer: usize, col: u32, row: u32| -> Result<usize, String> {
            let name = record
                .layers
                .get(layer)
                .ok_or_else(|| format!("layer {layer} was not saved"))?;
            let index = data
                .layers
                .iter()
                .position(|now| &now.name == name)
                .ok_or_else(|| format!("the map has no layer `{name}`"))?;
            if col >= data.width || row >= data.height {
                return Err(format!("the map is {} × {} cells", data.width, data.height));
            }
            Ok(index)
        };
        let mut plan = TilePlan {
            map: map.clone(),
            edits: Vec::new(),
            damage: Vec::new(),
        };
        for (layer, col, row, sprite) in &record.edited {
            let cell = format!("map `{map}`'s edit at layer {layer}, ({col}, {row})");
            let index = match at(*layer, *col, *row) {
                Ok(index) => index,
                Err(why) => {
                    report.dropped.push(format!("{cell}: {why}"));
                    continue;
                }
            };
            let tile = match sprite {
                None => Some(EMPTY_TILE),
                Some(sprite) => data
                    .tileset
                    .iter()
                    .position(|now| now == sprite)
                    .and_then(|tile| TileIndex::try_from(tile).ok()),
            };
            let Some(tile) = tile else {
                let sprite = sprite.as_deref().unwrap_or_default();
                report
                    .dropped
                    .push(format!("{cell}: the tileset has no `{sprite}`"));
                continue;
            };
            plan.edits.push((index, *col, *row, tile));
        }
        for &(layer, col, row, health) in &record.damaged {
            match at(layer, col, row) {
                Ok(index) => plan.damage.push((index, col, row, health)),
                Err(why) => report.dropped.push(format!(
                    "map `{map}`'s damage at layer {layer}, ({col}, {row}): {why}"
                )),
            }
        }
        plans.push(plan);
    }
    plans
}

/// Puts each planned map back as inserted, then writes its edits and its
/// damage.
pub(crate) fn apply_tiles(world: &mut World, plans: Vec<TilePlan>) {
    let Some(registry) = world.get_resource_mut::<TilemapRegistry>() else {
        return;
    };
    for plan in plans {
        registry.revert(&plan.map);
        for (layer, col, row, tile) in plan.edits {
            registry.set_tile(&plan.map, layer, col, row, tile);
        }
        for (layer, col, row, health) in plan.damage {
            let Some(full) = registry.health(&plan.map, layer, col, row) else {
                continue;
            };
            if health < full {
                registry.damage_tile(&plan.map, layer, col, row, full - health);
            }
        }
    }
}

/// Why [`World::spawn_area`] spawned nothing.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AreaError {
    /// The map is not registered, or one of its objects does not prepare.
    #[error(transparent)]
    Prefab(#[from] PrefabError),
    /// An object's name is held, or two of its objects share one.
    #[error("map `{map}`: {source}")]
    Name {
        /// The map.
        map: String,
        /// Why.
        source: NameError,
    },
}

impl World {
    /// Spawns area `map`, a manifest tilemap ID, at `origin`: the map put
    /// back as inserted (its edits from an earlier run gone), its instance,
    /// then its classed objects as [`spawn_map_objects`](Self::spawn_map_objects)
    /// spawns them, each named one under its name in the area
    /// ([`save::name`](crate::save::name)) and each unnamed one unsaved. The
    /// area joins the World's loaded areas with its map's tiles. Returns the
    /// instance, then the objects in order.
    ///
    /// # Errors
    ///
    /// [`AreaError::Prefab`] as `prepare_map_objects`; [`AreaError::Name`]
    /// when a live entity holds an object's name or two objects share one.
    /// Nothing changes on an error.
    ///
    /// # Panics
    ///
    /// As [`prepare_prefab`](Self::prepare_prefab).
    pub fn spawn_area(
        &mut self,
        map: &str,
        origin: impl Into<Vec2>,
    ) -> Result<Vec<Entity>, AreaError> {
        let origin = origin.into();
        let prepared = self.prepare_map_objects_named(map, origin)?;
        let name_error = |source| AreaError::Name {
            map: map.to_owned(),
            source,
        };
        let mut seen: BTreeMap<SaveId, &String> = BTreeMap::new();
        for (_, object) in prepared.iter().filter(|(_, object)| !object.is_empty()) {
            let id = SaveId::authored(object);
            if let Some(other) = seen.insert(id, object) {
                return Err(name_error(if other == object {
                    NameError::Taken {
                        name: object.clone(),
                    }
                } else {
                    NameError::Clash {
                        name: object.clone(),
                        other: other.clone(),
                    }
                }));
            }
            if let Some(named) = self
                .get_resource::<SaveState>()
                .and_then(|state| state.named.get(&id))
                && (named.name != *object || self.is_alive(named.entity))
            {
                return Err(name_error(if named.name == *object {
                    NameError::Taken {
                        name: object.clone(),
                    }
                } else {
                    NameError::Clash {
                        name: object.clone(),
                        other: named.name.clone(),
                    }
                }));
            }
        }

        if let Some(maps) = self.get_resource_mut::<TilemapRegistry>() {
            maps.revert(map);
        }
        if !self.has_resource::<SaveState>() {
            self.insert_resource(SaveState::default());
        }
        self.resource_mut::<SaveState>()
            .loaded
            .entry(map.to_owned())
            .or_default()
            .insert(map.to_owned());
        let mut spawned = vec![self.spawn_with((TilemapInstance::new(map, origin),))];
        for (prefab, object) in prepared {
            let entity = prefab.spawn(self);
            if !object.is_empty() {
                name(self, entity, Some(map), &object).map_err(name_error)?;
            }
            spawned.push(entity);
        }
        Ok(spawned)
    }
}

/// After the game spawns area `level`, applies its kept record as
/// [`restore_world`](super::restore_world) does, each spawn with `with`,
/// then resolves the references waiting on it; the area is no longer kept.
/// An area with no kept record resolves what waits alone.
///
/// # Errors
///
/// As `restore_world`, with the World and the kept record as they were.
pub fn enter_area<B: Bundle + Clone>(
    world: &mut World,
    level: &str,
    with: &B,
) -> Result<RestoreReport, RestoreError> {
    super::world::enter(world, level, with)
}

#[cfg(test)]
#[path = "../tests/save/area.rs"]
mod tests;
