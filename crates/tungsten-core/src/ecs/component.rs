use std::any::{Any, TypeId};
use std::collections::HashMap;

use super::entity::Entity;

/// Type-erased storage for a single component type.
/// Maps entity IDs to boxed component values.
pub(crate) struct ComponentStore {
    data: HashMap<u32, Box<dyn Any>>,
}

impl ComponentStore {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn insert(&mut self, entity: Entity, component: Box<dyn Any>) {
        self.data.insert(entity.0, component);
    }

    pub fn remove(&mut self, entity: Entity) -> Option<Box<dyn Any>> {
        self.data.remove(&entity.0)
    }

    pub fn get(&self, entity: Entity) -> Option<&dyn Any> {
        self.data.get(&entity.0).map(|b| b.as_ref())
    }

    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut dyn Any> {
        self.data.get_mut(&entity.0).map(|b| b.as_mut())
    }

    pub fn contains(&self, entity: Entity) -> bool {
        self.data.contains_key(&entity.0)
    }

    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.data.keys().map(|&id| Entity(id))
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.data.len()
    }
}

/// Registry of all component stores, keyed by `TypeId`.
pub(crate) struct ComponentRegistry {
    stores: HashMap<TypeId, ComponentStore>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            stores: HashMap::new(),
        }
    }

    pub fn get_or_create_store<T: 'static>(&mut self) -> &mut ComponentStore {
        self.stores
            .entry(TypeId::of::<T>())
            .or_insert_with(ComponentStore::new)
    }

    pub fn get_store<T: 'static>(&self) -> Option<&ComponentStore> {
        self.stores.get(&TypeId::of::<T>())
    }

    pub fn get_store_mut<T: 'static>(&mut self) -> Option<&mut ComponentStore> {
        self.stores.get_mut(&TypeId::of::<T>())
    }

    pub fn remove_entity(&mut self, entity: Entity) {
        for store in self.stores.values_mut() {
            store.remove(entity);
        }
    }
}
