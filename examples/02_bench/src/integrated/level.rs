//! `integrated` level: a seeded side-scroller `width` tiles wide and 96 high,
//! 16 px tiles, y down. The ground surface wanders between rows 80 and 90 in
//! single steps and staircases over a three-tile collision crust (the
//! rendered fill reaches the bottom row); full-height walls close both ends,
//! short walls stand on the ground and three tiers of one-tile platforms
//! float above it. Walkable surfaces come out as spans for placement.

use std::collections::HashMap;

use glam::Vec2;
use tungsten::core::{EMPTY_TILE, Pcg32, splitmix64};

/// Tile edge in px.
pub(super) const TILE: f32 = 16.0;
/// Level height in tiles.
pub(super) const ROWS: u32 = 96;
/// Tileset indices: the `gen::tile_rgba` variants.
pub(super) const DIRT: i32 = 0;
pub(super) const STONE: i32 = 1;
const GRASS: i32 = 2;
const MOSS: i32 = 3;

const SALT: u64 = 0x1E7E_1000_0000_0001;
const START_ROW: i32 = 86;
const GROUND_TOP: i32 = 80;
const GROUND_BOTTOM: i32 = 90;
/// Solid rows under each surface cell; bodies never get deeper.
const CRUST: u32 = 3;
/// Base rows of the platform tiers.
const TIERS: [i32; 3] = [72, 60, 48];

/// A walkable surface: its top edge `y` and horizontal extent, in px.
#[derive(Debug, Clone, Copy)]
pub(super) struct Span {
    pub(super) y: f32,
    pub(super) x0: f32,
    pub(super) x1: f32,
}

#[derive(Debug)]
pub(super) struct Level {
    pub(super) width: u32,
    /// Collision cells, row-major.
    solid: Vec<bool>,
    /// Render layers, row-major tileset indices: the terrain, then grass and
    /// moss decoration.
    pub(super) fill: Vec<i32>,
    pub(super) grass: Vec<i32>,
    pub(super) moss: Vec<i32>,
    /// Walkable surfaces, ordered by `x0`, then `y`.
    pub(super) spans: Vec<Span>,
}

/// Deterministic share in [0, 1) for decorating cell (`col`, `row`).
fn roll(col: usize, row: u32, salt: u64) -> f32 {
    let hash = splitmix64(((col as u64) << 32) ^ u64::from(row) ^ salt.wrapping_mul(0x9E37_79B9));
    (hash >> 40) as f32 / (1u64 << 24) as f32
}

/// Surface row per column: flat runs joined by single steps and staircases.
fn surface(rng: &mut Pcg32, width: usize) -> Vec<i32> {
    let mut rows = Vec::with_capacity(width + 32);
    let mut row = START_ROW;
    let mut dir = -1;
    while rows.len() < width {
        rows.extend(std::iter::repeat_n(row, (8 + rng.next_u32() % 17) as usize));
        let steps = match rng.next_u32() % 4 {
            0 => 0,
            1 | 2 => 1,
            _ => 3 + rng.next_u32() % 3,
        };
        if rng.next_u32().is_multiple_of(3) {
            dir = -dir;
        }
        for step in 0..steps {
            if !(GROUND_TOP..=GROUND_BOTTOM).contains(&(row + dir)) {
                dir = -dir;
            }
            row += dir;
            if step + 1 < steps {
                rows.extend(std::iter::repeat_n(row, (2 + rng.next_u32() % 3) as usize));
            }
        }
    }
    rows.truncate(width);
    rows
}

impl Level {
    pub(super) fn generate(width: u32, seed: u64) -> Self {
        let mut rng = Pcg32::seeded(splitmix64(seed ^ SALT));
        let w = width as usize;
        let cells = w * ROWS as usize;
        let mut level = Self {
            width,
            solid: vec![false; cells],
            fill: vec![EMPTY_TILE; cells],
            grass: vec![EMPTY_TILE; cells],
            moss: vec![EMPTY_TILE; cells],
            spans: Vec::new(),
        };
        let ground = surface(&mut rng, w);
        for (col, &top) in ground.iter().enumerate() {
            let top = top as u32;
            for row in top..ROWS {
                let index = level.index(col, row);
                level.fill[index] = if row == top { DIRT } else { STONE };
                level.solid[index] = row < top + CRUST;
            }
            if roll(col, top, 1) < 0.2 {
                let index = level.index(col, top);
                level.moss[index] = MOSS;
            }
        }
        for col in [0, w - 1] {
            for row in 0..ROWS {
                level.set_stone(col, row);
            }
        }

        let mut walls = vec![false; w];
        let mut col = 24 + (rng.next_u32() % 32) as usize;
        while col + 3 < w {
            let thick = 1 + (rng.next_u32() % 2) as usize;
            let height = 3 + rng.next_u32() % 4;
            let base = (col..col + thick)
                .map(|c| ground[c])
                .min()
                .unwrap_or(START_ROW) as u32;
            for (c, wall) in walls.iter_mut().enumerate().skip(col).take(thick) {
                for row in base - height..ground[c] as u32 {
                    level.set_stone(c, row);
                }
                *wall = true;
            }
            col += 48 + (rng.next_u32() % 49) as usize;
        }

        let mut spans = Vec::new();
        for base in TIERS {
            let mut col = 3 + (rng.next_u32() % 10) as usize;
            while col + 12 < w {
                let end = (col + 10 + (rng.next_u32() % 27) as usize).min(w - 2);
                let row = (base + (rng.next_u32() % 3) as i32 - 1) as u32;
                for c in col..end {
                    level.set_stone(c, row);
                    if roll(c, row, 2) < 0.55 {
                        let index = level.index(c, row - 1);
                        level.grass[index] = GRASS;
                    }
                    if roll(c, row, 3) < 0.4 {
                        let index = level.index(c, row + 1);
                        level.moss[index] = MOSS;
                    }
                }
                spans.push(span(row, col, end));
                col = end + 3 + (rng.next_u32() % 7) as usize;
            }
        }

        // Ground spans: runs of one surface row with no wall on them.
        let mut run: Option<(usize, i32)> = None;
        for col in 1..w - 1 {
            let key = (!walls[col]).then_some(ground[col]);
            if let Some(row) = key
                && roll(col, row as u32, 4) < 0.55
            {
                let index = level.index(col, row as u32 - 1);
                level.grass[index] = GRASS;
            }
            if run.is_some_and(|(_, row)| key == Some(row)) {
                continue;
            }
            if let Some((start, row)) = run {
                spans.push(span(row as u32, start, col));
            }
            run = key.map(|row| (col, row));
        }
        if let Some((start, row)) = run {
            spans.push(span(row as u32, start, w - 1));
        }
        spans.sort_by(|a, b| a.x0.total_cmp(&b.x0).then(a.y.total_cmp(&b.y)));
        level.spans = spans;
        level
    }

    fn index(&self, col: usize, row: u32) -> usize {
        row as usize * self.width as usize + col
    }

    fn set_stone(&mut self, col: usize, row: u32) {
        let index = self.index(col, row);
        self.solid[index] = true;
        self.fill[index] = STONE;
    }

    /// Level size in px.
    pub(super) fn size(&self) -> Vec2 {
        Vec2::new(self.width as f32, ROWS as f32) * TILE
    }

    /// Whether the collision cell under world point `at` is solid.
    pub(super) fn solid_at(&self, at: Vec2) -> bool {
        if at.x < 0.0 || at.y < 0.0 {
            return false;
        }
        let (col, row) = ((at.x / TILE) as usize, (at.y / TILE) as u32);
        col < self.width as usize && row < ROWS && self.solid[self.index(col, row)]
    }

    /// Solid cells: the tile proxies the physics step gathers every frame.
    pub(super) fn solid_tiles(&self) -> usize {
        self.solid.iter().filter(|&&solid| solid).count()
    }

    /// The collision layer's tiles: stone where solid.
    pub(super) fn collision_tiles(&self) -> Vec<i32> {
        self.solid
            .iter()
            .map(|&solid| if solid { STONE } else { EMPTY_TILE })
            .collect()
    }

    /// Summed span width in px.
    pub(super) fn walkable(&self) -> f32 {
        self.spans.iter().map(|span| span.x1 - span.x0).sum()
    }

    /// The solid cells as few static boxes, `(center, half extents)`: runs
    /// along each row, merged downward while a run keeps its columns.
    /// `tile_collision=off` spawns these instead of per-tile proxies.
    pub(super) fn boxes(&self) -> Vec<(Vec2, Vec2)> {
        let w = self.width as usize;
        let mut boxes: Vec<(usize, usize, u32, u32)> = Vec::new();
        let mut open: HashMap<(usize, usize), usize> = HashMap::new();
        for row in 0..ROWS {
            let mut next = HashMap::new();
            let mut col = 0;
            while col < w {
                if !self.solid[self.index(col, row)] {
                    col += 1;
                    continue;
                }
                let start = col;
                while col < w && self.solid[self.index(col, row)] {
                    col += 1;
                }
                let index = if let Some(&index) = open.get(&(start, col)) {
                    boxes[index].3 = row + 1;
                    index
                } else {
                    boxes.push((start, col, row, row + 1));
                    boxes.len() - 1
                };
                next.insert((start, col), index);
            }
            open = next;
        }
        boxes
            .into_iter()
            .map(|(c0, c1, r0, r1)| {
                let min = Vec2::new(c0 as f32, r0 as f32) * TILE;
                let max = Vec2::new(c1 as f32, r1 as f32) * TILE;
                ((min + max) * 0.5, (max - min) * 0.5)
            })
            .collect()
    }

    /// Slots `spacing` apart along every span wide enough for them, `margin`
    /// in from the span's ends and centered in it; each is (x, surface y).
    pub(super) fn slots(&self, spacing: f32, margin: f32) -> Vec<Vec2> {
        let mut slots = Vec::new();
        for span in &self.spans {
            let room = span.x1 - span.x0 - 2.0 * margin;
            if room < 0.0 {
                continue;
            }
            let count = (room / spacing).floor() as usize + 1;
            let x = span.x0 + margin + (room - (count - 1) as f32 * spacing) * 0.5;
            slots.extend((0..count).map(|k| Vec2::new(x + k as f32 * spacing, span.y)));
        }
        slots
    }
}

fn span(row: u32, col0: usize, col1: usize) -> Span {
    Span {
        y: row as f32 * TILE,
        x0: col0 as f32 * TILE,
        x1: col1 as f32 * TILE,
    }
}

/// `count` of `slots`, spread evenly; past one per slot, further layers
/// spread over the slots again. Returns each point's slot and layer.
pub(super) fn spread(slots: &[Vec2], count: usize) -> Vec<(Vec2, u32)> {
    if slots.is_empty() {
        return Vec::new();
    }
    let total = slots.len();
    (0..count)
        .map(|index| {
            let layer = index / total;
            let in_layer = (count - layer * total).min(total);
            (slots[index % total * total / in_layer], layer as u32)
        })
        .collect()
}
