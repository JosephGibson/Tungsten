//! Prints the kit's components snapshot: every scene component's field
//! descriptions and every data kind, as `scripts/public-api.sh` writes
//! `api/tungsten-kit-components.txt` (`D-107`).

use tungsten_core::{Schedule, World};
use tungsten_kit::{KitDescriptions, KitPlugins};

fn main() {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    KitPlugins::set().build(&mut schedule, &mut world);
    print!("{}", world.resource::<KitDescriptions>().render());
}
