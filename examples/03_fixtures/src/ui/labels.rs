//! World-anchored text (`D-161`): labels a game pushes into `WorldLabels`
//! every frame at world points, drawn under the UI at their own scale and
//! colour, culled off the view and capped. The `labels` fixture pushes a
//! fixed set for the pixel test; the interactive run floats damage numbers
//! that rise, pop and fade, ticked on the game clock.

use glam::Vec2;
use tungsten::core::ui::UiTree;
use tungsten::core::{ScaleFactor, Time, World, WorldLabel, WorldLabels};

use crate::ui::inventory::Inventory;

const RED: [u8; 4] = [255, 90, 90, 255];
const GREEN: [u8; 4] = [110, 230, 120, 255];
const BLUE: [u8; 4] = [150, 190, 255, 255];
const GOLD: [u8; 4] = [255, 200, 40, 255];
const WHITE: [u8; 4] = [255, 244, 236, 255];

/// The `labels` fixture's labels, pushed every frame: five on screen at
/// scales 1.0, 1.25 and 1.5 in three colours, a crit in `sans_bold`, one
/// straddling the inventory window's left edge, which the panel partly
/// covers, and one off the view, which the extract culls. The camera stays
/// at the origin, so world units are physical pixels; the places suit the
/// reference capture's 1280×720 at an OS scale of 1.75, and the straddling
/// label reads the window's edge from the last layout.
pub fn fixture_labels_system(world: &mut World) {
    let scale = world
        .get_resource::<ScaleFactor>()
        .map_or(1.0, ScaleFactor::get);
    let edge = world
        .get_resource::<Inventory>()
        .and_then(|inventory| world.resource::<UiTree>().rect(inventory.ui.root))
        .map(|rect| Vec2::new(rect.min.x, f32::midpoint(rect.min.y, rect.max.y)) * scale);
    let Some(labels) = world.get_resource_mut::<WorldLabels>() else {
        return;
    };
    let on_screen = [
        ("12", Vec2::new(110.0, 600.0), 1.0, RED),
        ("250", Vec2::new(300.0, 680.0), 1.25, GREEN),
        ("Lantern", Vec2::new(1080.0, 160.0), 1.0, BLUE),
        ("7", Vec2::new(1040.0, 300.0), 1.5, RED),
        ("+30", Vec2::new(800.0, 690.0), 1.25, GREEN),
    ];
    for (text, anchor, scale, color) in on_screen {
        labels.push(
            WorldLabel::new(text, anchor, "sans", 20.0)
                .with_scale(scale)
                .with_color(color),
        );
    }
    labels.push(
        WorldLabel::new("99!", Vec2::new(500.0, 690.0), "sans_bold", 26.0)
            .with_scale(1.5)
            .with_color(GOLD),
    );
    if let Some(edge) = edge {
        labels.push(WorldLabel::new("under 5", edge, "sans", 18.0).with_color(WHITE));
    }
    labels.push(WorldLabel::new(
        "culled",
        Vec2::new(-400.0, 300.0),
        "sans",
        18.0,
    ));
}

/// Seconds between two damage numbers at the start: four a second.
const SPAWN_INTERVAL: f32 = 0.25;
/// A damage number's life in seconds.
const LIFE: f32 = 0.8;
/// How far a number floats up over its life, in UI units.
const RISE: f32 = 40.0;
/// The pop: the scale falls from `POP_SCALE` to 1 over `POP_TIME` seconds.
const POP_SCALE: f32 = 1.6;
const POP_TIME: f32 = 0.15;
/// The world point the numbers spawn near.
const SPAWN_POINT: Vec2 = Vec2::new(1040.0, 560.0);

#[derive(Debug)]
struct DamageNumber {
    value: u32,
    crit: bool,
    anchor: Vec2,
    age: f32,
}

/// The interactive run's floating damage numbers, a `World` resource the
/// lab owns; the controls window switches them and sets their rate.
#[derive(Debug)]
pub struct DamageNumbers {
    numbers: Vec<DamageNumber>,
    until_next: f32,
    spawned: u32,
    enabled: bool,
    interval: f32,
}

impl Default for DamageNumbers {
    fn default() -> Self {
        Self {
            numbers: Vec::new(),
            until_next: 0.0,
            spawned: 0,
            enabled: true,
            interval: SPAWN_INTERVAL,
        }
    }
}

impl DamageNumbers {
    /// Whether new numbers spawn; the ones afloat finish either way.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Seconds between two numbers.
    #[must_use]
    pub fn interval(&self) -> f32 {
        self.interval
    }

    /// Starts or stops new numbers.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// Sets the spawn rate, numbers a second.
    pub fn set_rate(&mut self, per_second: f32) {
        self.interval = 1.0 / per_second;
    }
}

/// While enabled, spawns a number every interval (0.25 s at the start) near
/// a fixed point, every fourth a gold crit in `sans_bold`; each floats 40 UI
/// units up over 0.8 s, pops from scale 1.6 to 1 over its first 0.15 s and
/// fades out, ticked on `Time::delta`.
pub fn damage_numbers_system(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    let Some(mut damage) = world.remove_resource::<DamageNumbers>() else {
        return;
    };
    if damage.enabled {
        damage.until_next -= dt;
    }
    while damage.enabled && damage.until_next <= 0.0 {
        damage.until_next += damage.interval;
        let n = damage.spawned;
        damage.spawned += 1;
        let jitter = [-24.0, 8.0, -8.0, 24.0][(n % 4) as usize];
        damage.numbers.push(DamageNumber {
            value: 7 + (n * 37) % 90,
            crit: n % 4 == 3,
            anchor: SPAWN_POINT + Vec2::new(jitter, 0.0),
            age: 0.0,
        });
    }
    for number in &mut damage.numbers {
        number.age += dt;
    }
    damage.numbers.retain(|number| number.age < LIFE);
    if let Some(labels) = world.get_resource_mut::<WorldLabels>() {
        for number in &damage.numbers {
            let t = number.age / LIFE;
            let pop = (number.age / POP_TIME).min(1.0);
            let scale = POP_SCALE + (1.0 - POP_SCALE) * pop;
            let fade = ((LIFE - number.age) / (LIFE * 0.5)).clamp(0.0, 1.0);
            let (font, size, mut color) = if number.crit {
                ("sans_bold", 26.0, GOLD)
            } else {
                ("sans", 20.0, WHITE)
            };
            color[3] = (255.0 * fade) as u8;
            labels.push(
                WorldLabel::new(number.value.to_string(), number.anchor, font, size)
                    .with_offset(Vec2::new(0.0, -RISE * t))
                    .with_scale(scale)
                    .with_color(color),
            );
        }
    }
    world.insert_resource(damage);
}
