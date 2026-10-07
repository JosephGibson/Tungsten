//! The 14 `ecs` systems plus the audit, in the order `SYSTEMS` runs them.

use std::f32::consts::{PI, TAU};

use glam::Vec2;
use tungsten::core::{CommandBuffer, Position, Time, With, World};

use super::{
    Acc, Age, Bag, BenchCounters, Brain, COOLDOWN_PERIODS, Cooldowns, EcsCounts, FACTIONS, Faction,
    Follow, FollowScratch, HEALTH_BUCKETS, Heading, Health, MAX_HEALTH, Phase, Reductions, Regen,
    Stats, TEAMS, Team, Tint, VIEWPORT, Vel, WANDER,
};

const MAX_SPEED: f32 = 160.0;
const FOLLOW_ACCEL: f32 = 40.0;
const WANDER_JITTER: f32 = 0.5;
const HEADING_BLEND: f32 = 0.2;
const LOW_HEALTH: f32 = 25.0;
const DASH_DRAIN: f32 = 8.0;
const REST_GAIN: f32 = 2.0;
const BUFF_RATE: f32 = 0.1;
const STATS_DECAY: f32 = 0.5;
const STATS_FLOOR: f32 = 0.001;
const PHASE_RATE: f32 = PI;
const MAX_AGE: f32 = 30.0;

// `Brain::state` values besides `WANDER`.
const DASH: u8 = 1;
const REST: u8 = 2;
const FLEE: u8 = 3;

fn delta_seconds(world: &World) -> f32 {
    world.get_resource::<Time>().map_or(0.0, Time::delta)
}

fn with_counts(world: &mut World, update: impl FnOnce(&mut EcsCounts)) {
    if let Some(counters) = world.get_resource_mut::<BenchCounters<EcsCounts>>() {
        update(&mut counters.counts);
    }
}

fn xorshift(state: &mut u32) -> u32 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    x
}

/// Branching state machine: wander, dash (drains health), rest, and flee
/// below `LOW_HEALTH`. Speed is capped at `MAX_SPEED`.
pub(super) fn brain(world: &mut World) {
    let dt = delta_seconds(world);
    for (brain, vel, health) in world.query_mut::<(&mut Brain, &mut Vel, &mut Health)>() {
        brain.timer -= dt;
        if brain.timer <= 0.0 {
            let roll = xorshift(&mut brain.seed);
            brain.state = if health.0 < LOW_HEALTH {
                FLEE
            } else {
                (roll % 3) as u8
            };
            brain.timer = 0.2 + ((roll >> 8) & 63) as f32 / 64.0;
            if brain.state == FLEE {
                vel.0 = -vel.0;
            }
        }
        match brain.state {
            WANDER => {
                let roll = xorshift(&mut brain.seed);
                let jitter = Vec2::new(
                    (roll & 0xff) as f32 - 127.5,
                    ((roll >> 8) & 0xff) as f32 - 127.5,
                );
                vel.0 += jitter * (WANDER_JITTER * dt);
            }
            DASH => {
                vel.0 *= 1.0 + dt;
                health.0 -= DASH_DRAIN * dt;
            }
            REST => {
                vel.0 *= 1.0 - 2.0 * dt;
                health.0 += REST_GAIN * dt;
            }
            _ => vel.0 *= 1.0 + 0.5 * dt,
        }
        let speed_sq = vel.0.length_squared();
        if speed_sq > MAX_SPEED * MAX_SPEED {
            vel.0 *= MAX_SPEED / speed_sq.sqrt();
        }
        health.0 = health.0.clamp(0.0, MAX_HEALTH);
    }
}

/// Branching timers: each expired cooldown restarts its period.
pub(super) fn cooldowns(world: &mut World) {
    let dt = delta_seconds(world);
    for cooldowns in world.query_mut::<&mut Cooldowns>() {
        for (timer, period) in cooldowns.0.iter_mut().zip(COOLDOWN_PERIODS) {
            *timer -= dt;
            if *timer <= 0.0 {
                *timer += period;
            }
        }
    }
}

/// The optional-column path: `Regen` shortens a cooldown, `Stats` tracks
/// health.
pub(super) fn buffs(world: &mut World) {
    let dt = delta_seconds(world);
    for (health, cooldowns, regen, stats) in
        world.query_mut::<(&Health, &mut Cooldowns, Option<&Regen>, Option<&mut Stats>)>()
    {
        if let Some(regen) = regen {
            cooldowns.0[1] -= regen.0 * BUFF_RATE * dt;
        }
        if let Some(stats) = stats {
            stats.0[0] = health.0 * 0.01 + stats.0[1] * 0.5;
        }
    }
}

pub(super) fn regen(world: &mut World) {
    let dt = delta_seconds(world);
    for (health, regen) in world.query_mut::<(&mut Health, &Regen)>() {
        health.0 = (health.0 + regen.0 * dt).min(MAX_HEALTH);
    }
}

pub(super) fn stats_decay(world: &mut World) {
    let keep = 1.0 - STATS_DECAY * delta_seconds(world);
    for stats in world.query_mut::<&mut Stats>() {
        for stat in &mut stats.0 {
            *stat = *stat * keep + STATS_FLOOR;
        }
    }
}

pub(super) fn accelerate(world: &mut World) {
    let dt = delta_seconds(world);
    for (vel, acc) in world.query_mut::<(&mut Vel, &Acc)>() {
        vel.0 += acc.0 * dt;
    }
}

/// Random access: each follower reads its leader's `Position` through
/// `World::get`, then steers toward it. Every archetype holding `Follow` also
/// holds `Position` and `Vel`, so both passes visit the same archetypes in
/// the same order and the steer list zips by row.
pub(super) fn follow(world: &mut World) {
    let dt = delta_seconds(world);
    let Some(mut scratch) = world.remove_resource::<FollowScratch>() else {
        return;
    };
    scratch.0.clear();
    for (follow, position) in world.query::<(&Follow, &Position)>() {
        let leader = world
            .get::<Position>(follow.0)
            .map_or(position.0, |leader| leader.0);
        scratch
            .0
            .push((leader - position.0).normalize_or_zero() * (FOLLOW_ACCEL * dt));
    }
    for (vel, steer) in world
        .query_mut_filtered::<&mut Vel, With<Follow>>()
        .zip(&scratch.0)
    {
        vel.0 += *steer;
    }
    let lookups = scratch.0.len() as u32;
    world.insert_resource(scratch);
    with_counts(world, |counts| counts.lookups += lookups);
}

pub(super) fn integrate(world: &mut World) {
    let dt = delta_seconds(world);
    for (position, vel) in world.query_mut::<(&mut Position, &Vel)>() {
        position.0 += vel.0 * dt;
    }
}

/// Branching wrap into the viewport-sized world.
pub(super) fn bounds_wrap(world: &mut World) {
    for position in world.query_mut::<&mut Position>() {
        let point = &mut position.0;
        if point.x < 0.0 {
            point.x += VIEWPORT.x;
        } else if point.x >= VIEWPORT.x {
            point.x -= VIEWPORT.x;
        }
        if point.y < 0.0 {
            point.y += VIEWPORT.y;
        } else if point.y >= VIEWPORT.y {
            point.y -= VIEWPORT.y;
        }
    }
}

/// Heading eases toward the velocity direction through `atan2`.
pub(super) fn heading(world: &mut World) {
    for (heading, vel) in world.query_mut::<(&mut Heading, &Vel)>() {
        let target = vel.0.y.atan2(vel.0.x);
        let mut turn = target - heading.0;
        if turn > PI {
            turn -= TAU;
        } else if turn < -PI {
            turn += TAU;
        }
        heading.0 += turn * HEADING_BLEND;
        if heading.0 > PI {
            heading.0 -= TAU;
        } else if heading.0 < -PI {
            heading.0 += TAU;
        }
    }
}

pub(super) fn tint(world: &mut World) {
    for (tint, health) in world.query_mut::<(&mut Tint, &Health)>() {
        let t = health.0 / MAX_HEALTH;
        tint.0 = [(255.0 * (1.0 - t)) as u8, (255.0 * t) as u8, 64, 255];
    }
}

pub(super) fn age_phase(world: &mut World) {
    let dt = delta_seconds(world);
    for (age, phase) in world.query_mut::<(&mut Age, &mut Phase)>() {
        age.0 += dt;
        if age.0 >= MAX_AGE {
            age.0 = 0.0;
        }
        phase.0 += dt * (PHASE_RATE + age.0 / MAX_AGE);
        if phase.0 >= TAU {
            phase.0 -= TAU;
        }
    }
}

/// Read-only reduction: bag totals per team.
pub(super) fn team_bags(world: &mut World) {
    let mut totals = [0_u64; TEAMS];
    for (team, bag) in world.query::<(&Team, &Bag)>() {
        totals[team.0 as usize] += bag.0.iter().map(|&item| u64::from(item)).sum::<u64>();
    }
    if let Some(reductions) = world.get_resource_mut::<Reductions>() {
        reductions.team_bags = totals;
    }
}

/// Read-only reduction: faction histogram over health buckets, read from `Tint`.
pub(super) fn faction_histogram(world: &mut World) {
    let mut histogram = [[0_u32; HEALTH_BUCKETS]; FACTIONS];
    for (faction, tint) in world.query::<(&Faction, &Tint)>() {
        histogram[faction.0 as usize][(tint.0[1] >> 6) as usize] += 1;
    }
    if let Some(reductions) = world.get_resource_mut::<Reductions>() {
        reductions.factions = histogram;
    }
}

/// Last system: counts structural commands queued this frame plus any change
/// in the live entity count. Both stay 0 by design.
pub(super) fn ecs_audit(world: &mut World) {
    let queued = world
        .get_resource::<CommandBuffer>()
        .map_or(0, CommandBuffer::len) as u32;
    let live = world.entity_count();
    with_counts(world, |counts| {
        counts.structural += queued + live.abs_diff(counts.live);
        counts.live = live;
    });
}
