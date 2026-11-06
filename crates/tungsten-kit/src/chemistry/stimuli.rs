//! Chemistry's stimuli past a landed hit: a status's or an output's
//! emission to the receivers in a radius, a body's impact and the light at
//! a receiver's point.

use glam::Vec2;
use tungsten_core::lighting::{light_from, luma};
use tungsten_core::physics::ContactBegan;
use tungsten_core::{Entity, Light, Transform, World};

use crate::chemistry::grid::{Candidate, Grid, locate};
use crate::chemistry::queue::{Chemistry, Receiver, Settings, Stimulus};
use crate::chemistry::rules::{Interaction, rules_of};
use crate::chemistry::tiles::{Cell, MapNames, cell_of_hit, cells_within};
use crate::foundation::saved::order_of;
use crate::foundation::source::Source;
use crate::foundation::tags::{STIMULUS_LIGHT, Tags, covers};

/// A stimulus's name as an emission carries it: interned in the queue's
/// names, or new to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Named {
    Known(u32),
    New(String),
}

/// One emission waiting for chemistry's next run: `amount` of the stimulus
/// to every receiver within `radius` of `point` but `from`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Emission {
    pub(crate) stimulus: u32,
    pub(crate) amount: f32,
    pub(crate) radius: f32,
    pub(crate) point: Vec2,
    /// The emitter, which it does not reach.
    pub(crate) from: Option<Receiver>,
    pub(crate) credit: Option<Entity>,
    pub(crate) depth: u8,
}

/// Queues an emission from `from`'s point for chemistry's next run, `depth`
/// reactions deep; nothing without chemistry or a point.
pub(crate) fn emit_from(
    world: &mut World,
    from: Entity,
    stimulus: Named,
    amount: f32,
    radius: f32,
    credit: Option<Entity>,
    depth: u8,
) {
    let Some((point, _)) = locate(world, from) else {
        return;
    };
    let Some(chemistry) = world.get_resource_mut::<Chemistry>() else {
        return;
    };
    let stimulus = match stimulus {
        Named::Known(id) => id,
        Named::New(name) => chemistry.intern(&name),
    };
    chemistry.emitted.push(Emission {
        stimulus,
        amount,
        radius,
        point,
        from: Some(Receiver::Entity(from)),
        credit,
        depth,
    });
}

/// A run's kept lists for radius work: the grid of entities, a query's
/// finds in it, the tile cells in a radius and the map IDs they name.
#[derive(Default)]
pub(crate) struct Reach {
    pub(crate) grid: Grid,
    found: Vec<u32>,
    cells: Vec<Cell>,
    pub(crate) names: MapNames,
}

impl Reach {
    /// Forgets the last run's grid.
    pub(crate) fn clear(&mut self) {
        self.grid.clear();
    }

    /// Gathers what lies within `radius` of `point` but `from`: the grid's
    /// candidates (built once a run) and the material cells of every
    /// placement.
    pub(crate) fn gather(
        &mut self,
        world: &World,
        point: Vec2,
        radius: f32,
        from: Option<&Receiver>,
    ) {
        self.grid.build(world);
        self.grid.within(point, radius, &mut self.found);
        if let Some(Receiver::Entity(from)) = from {
            let grid = &self.grid;
            self.found.retain(|&index| grid.get(index).entity != *from);
        }
        let skip = match from {
            Some(Receiver::Cell(cell)) => Some(cell),
            _ => None,
        };
        cells_within(world, point, radius, skip, &mut self.names, &mut self.cells);
    }

    /// The gathered candidates, in the grid's order.
    pub(crate) fn entities(&self) -> impl Iterator<Item = &Candidate> + '_ {
        self.found.iter().map(|&index| self.grid.get(index))
    }

    /// The gathered cells, in their order.
    pub(crate) fn cells(&self) -> &[Cell] {
        &self.cells
    }
}

/// The stimulus an amount makes, at status effect 1, its damage-over-time
/// base the amount.
fn plain(amount: f32, depth: u8, credit: Option<Entity>) -> Stimulus {
    Stimulus {
        amount,
        status_effect: 1.0,
        dot_base: amount,
        depth,
        credit,
    }
}

/// Queues `emissions` on the receivers each reaches, for run `run`: each
/// tagged entity within its radius but its emitter, in the grid's order,
/// then each material cell. `answered` says whether some rule answers a
/// name; an emission no rule answers is dropped before the grid is built.
pub(crate) fn spread(
    world: &mut World,
    settings: &Settings,
    emissions: &mut Vec<Emission>,
    reach: &mut Reach,
    answered: impl Fn(&str) -> bool,
    run: u64,
) {
    for emission in emissions.drain(..) {
        let name = world.resource::<Chemistry>().name(emission.stimulus);
        if !answered(name) {
            continue;
        }
        reach.gather(
            world,
            emission.point,
            emission.radius,
            emission.from.as_ref(),
        );
        let chemistry = world.resource_mut::<Chemistry>();
        let stimulus = plain(emission.amount, emission.depth, emission.credit);
        for candidate in reach.entities() {
            if candidate.tagged {
                let receiver = Receiver::Entity(candidate.entity);
                chemistry.stimulate_id(settings, receiver, emission.stimulus, stimulus, run);
            }
        }
        for cell in reach.cells() {
            let receiver = Receiver::Cell(cell.clone());
            chemistry.stimulate_id(settings, receiver, emission.stimulus, stimulus, run);
        }
    }
}

/// What a body's impact credits: the other side's `Source` entity, else
/// the other side; a tile credits none.
fn impact_credit(world: &World, other: Option<Entity>) -> Option<Entity> {
    other.map(|other| {
        world
            .get::<Source>(other)
            .map_or(other, |source| source.entity())
    })
}

/// Queues each begin of `begins` as `stimulus.impact` on both its sides,
/// by its closing speed, for run `run`: a body, or a tile's cell when its
/// tile has a material, credited to the body.
pub(crate) fn impacts(
    world: &mut World,
    settings: &Settings,
    begins: &[ContactBegan],
    impact: u32,
    names: &mut MapNames,
    run: u64,
) {
    for begin in begins {
        let credit_a = impact_credit(world, begin.b);
        let credit_b = impact_credit(world, Some(begin.a));
        let other = match (begin.b, &begin.tile) {
            (Some(b), _) => Some(Receiver::Entity(b)),
            (None, Some(hit)) => cell_of_hit(world, hit, names).map(Receiver::Cell),
            (None, None) => None,
        };
        let chemistry = world.resource_mut::<Chemistry>();
        let a = Receiver::Entity(begin.a);
        chemistry.stimulate_id(settings, a, impact, plain(begin.speed, 0, credit_a), run);
        if let Some(other) = other {
            chemistry.stimulate_id(
                settings,
                other,
                impact,
                plain(begin.speed, 0, credit_b),
                run,
            );
        }
    }
}

/// The light sampler's kept lists: the lights at a sample and the
/// receivers it reads.
#[derive(Default)]
pub(crate) struct LightSampler {
    lights: Vec<(Vec2, Light)>,
    sampled: Vec<(Entity, Vec2)>,
}

impl LightSampler {
    /// Counts the light clock down by `dt` and, when it comes due (at once
    /// on the first step with a light rule, then every period), queues on
    /// each receiver some light rule's `on` passes a `stimulus.light` of
    /// the luma of the lights' sum at its point, for the coming run.
    /// Without a light rule the clock stops.
    pub(crate) fn tick(&mut self, world: &mut World, settings: &Settings, dt: f32) {
        let light_rules = rules_of(world)
            .filter(|rule| covers(STIMULUS_LIGHT, rule.stimulus()))
            .count();
        let Some(chemistry) = world.get_resource_mut::<Chemistry>() else {
            return;
        };
        if light_rules == 0 {
            chemistry.light_left = None;
            return;
        }
        let mut left = chemistry.light_left.unwrap_or(0.0) - dt;
        let due = left <= crate::TIME_EPSILON;
        if due {
            left += settings.light_period;
            if left <= crate::TIME_EPSILON {
                left = settings.light_period;
            }
        }
        chemistry.light_left = Some(left);
        if !due {
            return;
        }
        self.lights.clear();
        self.lights.extend(
            world
                .query::<(&Transform, &Light)>()
                .map(|(transform, light)| (transform.position, *light)),
        );
        self.sampled.clear();
        {
            let rules: Vec<&Interaction> = rules_of(world)
                .filter(|rule| covers(STIMULUS_LIGHT, rule.stimulus()))
                .collect();
            for (entity, tags) in world.query::<(Entity, &Tags)>() {
                if !rules.iter().any(|rule| rule.on().matches(Some(tags))) {
                    continue;
                }
                if let Some((point, _)) = locate(world, entity) {
                    self.sampled.push((entity, point));
                }
            }
        }
        let order = order_of(world);
        self.sampled.sort_by_key(|&(entity, _)| order.key(entity));
        let chemistry = world.resource_mut::<Chemistry>();
        let run = chemistry.run;
        let id = chemistry.intern(STIMULUS_LIGHT);
        for &(entity, point) in &self.sampled {
            let rgb = self
                .lights
                .iter()
                .fold(glam::Vec3::ZERO, |sum, (position, light)| {
                    sum + light_from(light, *position, point)
                });
            let receiver = Receiver::Entity(entity);
            chemistry.stimulate_id(settings, receiver, id, plain(luma(rgb), 0, None), run);
        }
    }
}
