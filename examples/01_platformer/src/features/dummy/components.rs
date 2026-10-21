//! The dummy's components and resources.

use serde::Deserialize;

/// The training dummy's marker, which its prefab names `dummy`.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct Dummy;

/// The save IDs of the dummies the map placed for this run, sorted: a
/// World resource the run's start fills.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlacedDummies(pub Vec<u64>);
