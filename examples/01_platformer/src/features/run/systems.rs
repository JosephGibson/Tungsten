//! The run's systems.

use tungsten::core::{EventCursor, EventQueue, World};
use tungsten::kit::ZoneEntered;

use super::components::Run;
use crate::features::level::components::Area;
use crate::shell;

/// `run_areas`, in the react phase: the player entering an area other than
/// its current one names it on the banner and asks for an autosave, which the
/// shell takes once a capture succeeds. A body set down astride two areas
/// enters both in one step: the current one, when it is among them, stays,
/// else the last entered is taken. `cursor` reads each entry once.
pub fn areas(world: &mut World, cursor: &mut EventCursor<ZoneEntered>) {
    let Some(run) = world.get_resource::<Run>() else {
        return;
    };
    let Some(events) = world.get_resource::<EventQueue<ZoneEntered>>() else {
        return;
    };
    let entered: Vec<&str> = cursor
        .read(events)
        .filter(|event| event.entity == run.player)
        .filter_map(|event| world.get::<Area>(event.zone).map(|area| area.name.as_str()))
        .collect();
    if entered.is_empty() || entered.contains(&run.area.as_deref().unwrap_or_default()) {
        return;
    }
    let name = entered[entered.len() - 1].to_owned();
    world.resource_mut::<Run>().area = Some(name.clone());
    super::banner::show(world, &name);
    shell::request_autosave(world);
}
