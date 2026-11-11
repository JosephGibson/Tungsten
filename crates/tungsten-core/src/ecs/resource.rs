use std::any::{Any, TypeId};

use super::archetype::TypeIdMap;

/// Type-indexed singleton resource storage. A `TypeId` is a hash already,
/// so the map passes it through: a system's every resource read keys it.
pub(crate) struct ResourceMap {
    resources: TypeIdMap<Box<dyn Any>>,
}

impl ResourceMap {
    pub fn new() -> Self {
        Self {
            resources: TypeIdMap::default(),
        }
    }

    pub fn insert<T: 'static>(&mut self, resource: T) {
        self.resources.insert(TypeId::of::<T>(), Box::new(resource));
    }

    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.resources
            .get(&TypeId::of::<T>())
            .and_then(|r| r.downcast_ref::<T>())
    }

    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.resources
            .get_mut(&TypeId::of::<T>())
            .and_then(|r| r.downcast_mut::<T>())
    }

    pub fn contains<T: 'static>(&self) -> bool {
        self.resources.contains_key(&TypeId::of::<T>())
    }

    pub fn remove<T: 'static>(&mut self) -> Option<T> {
        self.resources
            .remove(&TypeId::of::<T>())
            .and_then(|r| r.downcast::<T>().ok())
            .map(|b| *b)
    }
}
