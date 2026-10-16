//! Inspector rows: each kit component registered in core's
//! `InspectRegistry`, which the umbrella's inspector overlay prints for a
//! picked entity. The `Inspectable` impls live with their types; each
//! plugin registers its rows through [`register_rows`] in `build`.

use tungsten_core::{Entity, InspectRegistry, Inspectable, World};

/// Registers `T`'s rows under `label`, inserting the `InspectRegistry` when
/// the World has none: the App inserts the engine's before the plugins
/// build, so a kit plugin finds it in an App and inserts one in a bare
/// World. A label already registered is left as it is, so a plugin built
/// twice registers once.
pub(crate) fn register_rows<T: Inspectable + 'static>(world: &mut World, label: &'static str) {
    let registry = registry_mut(world);
    if !registry.iter().any(|(known, _)| known == label) {
        registry.register::<T>(label);
    }
}

/// Registers rows computed by `rows` under `label`, for a type whose rows
/// are not its own `Inspectable`; once per label, as [`register_rows`].
pub(crate) fn register_rows_with(
    world: &mut World,
    label: &'static str,
    rows: impl Fn(&World, Entity) -> Vec<(&'static str, String)> + 'static,
) {
    let registry = registry_mut(world);
    if !registry.iter().any(|(known, _)| known == label) {
        registry.register_with(label, rows);
    }
}

fn registry_mut(world: &mut World) -> &mut InspectRegistry {
    if !world.has_resource::<InspectRegistry>() {
        world.insert_resource(InspectRegistry::new());
    }
    world.resource_mut::<InspectRegistry>()
}

/// `(x, y)` to two decimals.
pub(crate) fn point(v: glam::Vec2) -> String {
    format!("({:.2}, {:.2})", v.x, v.y)
}

#[cfg(test)]
#[path = "../tests/inspect.rs"]
mod tests;
