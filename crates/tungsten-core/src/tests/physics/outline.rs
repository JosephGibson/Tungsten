use super::*;
use crate::physics::collision::{FACE_ALL, FACE_BOTTOM, FACE_LEFT, FACE_RIGHT, FACE_TOP};

/// Tiles by letter: `#` plain, `a` and `b` of materials 1 and 2, `=`
/// one-way, `.` empty.
const CLASSES: [TileClass; 4] = [
    TileClass {
        material: 0,
        one_way: false,
        slope: 0,
    },
    TileClass {
        material: 1,
        one_way: false,
        slope: 0,
    },
    TileClass {
        material: 2,
        one_way: false,
        slope: 0,
    },
    TileClass {
        material: 0,
        one_way: true,
        slope: 0,
    },
];

/// A map's width, height and tiles from its rows of letters.
fn picture(rows: &[&str]) -> (u32, u32, Vec<TileIndex>) {
    let tiles = rows
        .iter()
        .flat_map(|row| row.chars())
        .map(|letter| match letter {
            '#' => 0,
            'a' => 1,
            'b' => 2,
            '=' => 3,
            _ => -1,
        })
        .collect();
    (rows[0].len() as u32, rows.len() as u32, tiles)
}

/// Every chunk's runs, chunk by chunk in row-major order, and the masks the
/// trace wrote.
fn trace_all(width: u32, height: u32, tiles: &[TileIndex]) -> (Vec<Run>, Vec<u8>) {
    trace_classes(width, height, tiles, &CLASSES)
}

/// [`trace_all`] with the tiles' classes given.
fn trace_classes(
    width: u32,
    height: u32,
    tiles: &[TileIndex],
    classes: &[TileClass],
) -> (Vec<Run>, Vec<u8>) {
    let cells = LayerCells {
        width,
        height,
        tiles,
        classes,
    };
    let (across, down) = cells.chunks();
    let mut masks = vec![0xFF; tiles.len()];
    let mut all = Vec::new();
    let mut runs = Vec::new();
    for cy in 0..down {
        for cx in 0..across {
            trace_chunk(&cells, cx, cy, &mut masks, &mut runs);
            all.extend_from_slice(&runs);
        }
    }
    (all, masks)
}

fn run(col: u32, row: u32, cols: u32, rows: u32, mask: u8) -> Run {
    Run {
        col,
        row,
        cols,
        rows,
        mask,
    }
}

const T: u8 = FACE_TOP;
const B: u8 = FACE_BOTTOM;
const L: u8 = FACE_LEFT;
const R: u8 = FACE_RIGHT;
const O: u8 = FACE_ONE_WAY;

#[test]
fn outline_fixtures_trace_their_runs() {
    let cases: [(&str, &[&str], Vec<Run>); 10] = [
        (
            "a cell",
            &["...", ".#.", "..."],
            vec![run(1, 1, 1, 1, FACE_ALL)],
        ),
        (
            "an L",
            &[".....", ".#...", ".#...", ".###.", "....."],
            vec![
                run(1, 1, 1, 1, T | L | R),
                run(1, 2, 1, 1, L | R),
                run(1, 3, 1, 1, B | L),
                run(2, 3, 1, 1, T | B),
                run(3, 3, 1, 1, T | B | R),
            ],
        ),
        (
            "a step",
            &["......", "....##", "..####", "######"],
            vec![
                run(4, 1, 1, 1, T | L),
                run(5, 1, 1, 1, T | R),
                run(2, 2, 1, 1, T | L),
                run(3, 2, 1, 1, T),
                run(5, 2, 1, 1, R),
                run(0, 3, 1, 1, T | B | L),
                run(1, 3, 1, 1, T | B),
                run(2, 3, 3, 1, B),
                run(5, 3, 1, 1, B | R),
            ],
        ),
        (
            "a one-cell pillar",
            &["...", ".#.", ".#.", ".#.", ".#.", "..."],
            vec![
                run(1, 1, 1, 1, T | L | R),
                run(1, 2, 1, 2, L | R),
                run(1, 4, 1, 1, B | L | R),
            ],
        ),
        (
            "a floor across a chunk border",
            &[
                "....................",
                "..################..",
                "....................",
            ],
            vec![
                run(2, 1, 1, 1, FACE_ALL & !R),
                run(3, 1, 13, 1, T | B),
                run(16, 1, 1, 1, T | B),
                run(17, 1, 1, 1, FACE_ALL & !L),
            ],
        ),
        (
            "a material change",
            &["........", ".#aaabb.", "........"],
            vec![
                run(1, 1, 1, 1, FACE_ALL & !R),
                run(2, 1, 3, 1, T | B),
                run(5, 1, 1, 1, T | B),
                run(6, 1, 1, 1, FACE_ALL & !L),
            ],
        ),
        (
            "one-way cells beside a wall",
            &["....#.", ".===#.", "....#."],
            vec![
                run(4, 0, 1, 1, T | L | R),
                run(1, 1, 1, 1, T | L | O),
                run(2, 1, 1, 1, T | O),
                run(3, 1, 1, 1, T | R | O),
                run(4, 1, 1, 1, L | R),
                run(4, 2, 1, 1, B | L | R),
            ],
        ),
        (
            "one-way cells over a solid cell, and stacked",
            &["......", ".=.=..", ".#.=..", "......"],
            vec![
                run(1, 1, 1, 1, T | L | R | O),
                run(3, 1, 1, 1, T | L | R | O),
                run(1, 2, 1, 1, FACE_ALL),
            ],
        ),
        (
            "the map's border",
            &["##", "##"],
            vec![
                run(0, 0, 1, 1, T | L),
                run(1, 0, 1, 1, T | R),
                run(0, 1, 1, 1, B | L),
                run(1, 1, 1, 1, B | R),
            ],
        ),
        (
            "a block's interior",
            &["......", ".####.", ".####.", ".####.", ".####.", "......"],
            vec![
                run(1, 1, 1, 1, T | L),
                run(2, 1, 2, 1, T),
                run(4, 1, 1, 1, T | R),
                run(1, 2, 1, 2, L),
                run(4, 2, 1, 2, R),
                run(1, 4, 1, 1, B | L),
                run(2, 4, 2, 1, B),
                run(4, 4, 1, 1, B | R),
            ],
        ),
    ];
    for (name, rows, want) in cases {
        let (width, height, tiles) = picture(rows);
        assert_eq!(trace_all(width, height, &tiles).0, want, "{name}");
    }
}

#[test]
fn every_exposed_face_is_on_exactly_one_box() {
    for seed in 0..200 {
        let mut rng = crate::Pcg32::seeded(0x0071_17E0 + seed);
        let (width, height) = (40, 40);
        let tiles: Vec<TileIndex> = (0..width * height)
            .map(|_| match rng.next_u32() % 10 {
                0..=3 => -1,
                4..=6 => 0,
                7 => 1,
                8 => 2,
                _ => 3,
            })
            .collect();
        let cells = LayerCells {
            width,
            height,
            tiles: &tiles,
            classes: &CLASSES,
        };
        let (runs, masks) = trace_all(width, height, &tiles);
        let mut owner = vec![None; (width * height) as usize];
        for (index, run) in runs.iter().enumerate() {
            assert!(
                run.cols == 1 || run.rows == 1,
                "seed {seed}: {run:?} is a run"
            );
            assert_eq!(
                (run.col / CHUNK, run.row / CHUNK),
                (
                    (run.col + run.cols - 1) / CHUNK,
                    (run.row + run.rows - 1) / CHUNK
                ),
                "seed {seed}: {run:?} stays in its chunk"
            );
            for row in run.row..run.row + run.rows {
                for col in run.col..run.col + run.cols {
                    assert_eq!(
                        cells.mask(col, row),
                        run.mask,
                        "seed {seed}: ({col}, {row}) shares {run:?}'s mask"
                    );
                    let slot = &mut owner[(row * width + col) as usize];
                    assert!(slot.is_none(), "seed {seed}: ({col}, {row}) on two boxes");
                    *slot = Some(index);
                }
            }
        }
        // Every cell with an exposed face is on a box, so its faces are the
        // box's; an interior, empty or covered one-way cell is on none. The
        // trace wrote every cell's mask.
        for row in 0..height {
            for col in 0..width {
                let at = (row * width + col) as usize;
                assert_eq!(masks[at], cells.mask(col, row), "seed {seed}: mask");
                let exposed = cells.mask(col, row) & FACE_ALL != 0;
                assert_eq!(owner[at].is_some(), exposed, "seed {seed}: ({col}, {row})");
            }
        }
    }
}

/// Each slope kind's legs by hand, by code less 1: a side and its span
/// along it from its top or left end, in cell units.
const LEGS: [&[(u8, f32, f32)]; 12] = [
    // 45: br, bl, tr, tl.
    &[(B, 0.0, 1.0), (R, 0.0, 1.0)],
    &[(B, 0.0, 1.0), (L, 0.0, 1.0)],
    &[(T, 0.0, 1.0), (R, 0.0, 1.0)],
    &[(T, 0.0, 1.0), (L, 0.0, 1.0)],
    // 2:1 low.
    &[(B, 0.0, 1.0), (R, 0.5, 1.0)],
    &[(B, 0.0, 1.0), (L, 0.5, 1.0)],
    &[(T, 0.0, 1.0), (R, 0.0, 0.5)],
    &[(T, 0.0, 1.0), (L, 0.0, 0.5)],
    // 2:1 high.
    &[(B, 0.0, 1.0), (R, 0.0, 1.0), (L, 0.5, 1.0)],
    &[(B, 0.0, 1.0), (L, 0.0, 1.0), (R, 0.5, 1.0)],
    &[(T, 0.0, 1.0), (R, 0.0, 1.0), (L, 0.0, 0.5)],
    &[(T, 0.0, 1.0), (L, 0.0, 1.0), (R, 0.0, 0.5)],
];

/// What a cell of `kind` covers of its side `side`: a full cell (kind 0)
/// the whole side, a slope its leg there, an empty cell nothing.
fn cover(kind: Option<u8>, side: u8) -> Option<(f32, f32)> {
    match kind? {
        0 => Some((0.0, 1.0)),
        code => LEGS[usize::from(code - 1)]
            .iter()
            .find(|leg| leg.0 == side)
            .map(|leg| (leg.1, leg.2)),
    }
}

/// Each slope kind's polygon by hand, by code less 1: its vertices in the
/// cell's local units, y down.
const POLYGONS: [&[(f32, f32)]; 12] = [
    &[(0.0, 1.0), (1.0, 1.0), (1.0, 0.0)],
    &[(0.0, 0.0), (0.0, 1.0), (1.0, 1.0)],
    &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)],
    &[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)],
    &[(0.0, 1.0), (1.0, 1.0), (1.0, 0.5)],
    &[(0.0, 0.5), (0.0, 1.0), (1.0, 1.0)],
    &[(0.0, 0.0), (1.0, 0.0), (1.0, 0.5)],
    &[(0.0, 0.0), (1.0, 0.0), (0.0, 0.5)],
    &[(0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.5)],
    &[(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.5)],
    &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 0.5)],
    &[(0.0, 0.0), (1.0, 0.0), (1.0, 0.5), (0.0, 1.0)],
];

/// The solid part of a cell of `kind`, in its local units.
fn polygon_of(kind: u8) -> Vec<(f32, f32)> {
    match kind {
        0 => vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
        code => POLYGONS[usize::from(code - 1)].to_vec(),
    }
}

/// Whether the box `min`–`max` overlaps the solid part of a cell of `kind`,
/// in its local units, by SAT on the axes and the polygon's edges' normals.
fn box_meets(kind: u8, min: (f32, f32), max: (f32, f32)) -> bool {
    let points = polygon_of(kind);
    let corners = [
        (min.0, min.1),
        (max.0, min.1),
        (max.0, max.1),
        (min.0, max.1),
    ];
    let mut axes = vec![(1.0, 0.0), (0.0, 1.0)];
    for (i, &p) in points.iter().enumerate() {
        let q = points[(i + 1) % points.len()];
        axes.push((q.1 - p.1, p.0 - q.0));
    }
    axes.into_iter().all(|(x, y)| {
        let span = |of: &[(f32, f32)]| {
            of.iter()
                .map(|&(u, v)| u * x + v * y)
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), d| {
                    (lo.min(d), hi.max(d))
                })
        };
        let (a, b) = (span(&points), span(&corners));
        a.0 < b.1 && b.0 < a.1
    })
}

/// The mask of `(col, row)` in a map of kinds: each side whose span, a full
/// cell's whole side or a slope's leg, the neighbour across leaves
/// uncovered by `LEGS`, and each side of a slope without a leg where a box
/// of 0.05 cells resting against its vertex farthest across, centred on it,
/// meets no neighbour's solid part.
fn expected_mask(kinds: &[Option<u8>], width: u32, col: u32, row: u32) -> u8 {
    let height = kinds.len() as u32 / width;
    let kind_at = |c: i64, r: i64| {
        let inside = (0..i64::from(width)).contains(&c) && (0..i64::from(height)).contains(&r);
        inside
            .then(|| kinds[(r * i64::from(width) + c) as usize])
            .flatten()
    };
    let (col, row) = (i64::from(col), i64::from(row));
    let me = kind_at(col, row).expect("a solid cell");
    let open = |side: u8, across: u8, (dc, dr): (i64, i64)| {
        let Some(code) = (me != 0)
            .then_some(me)
            .filter(|&code| cover(Some(code), side).is_none())
        else {
            let need = cover(Some(me), side).unwrap();
            return !cover(kind_at(col + dc, row + dr), across)
                .is_some_and(|(low, high)| low <= need.0 && need.1 <= high);
        };
        let (out_u, out_v) = (dc as f32, dr as f32);
        let points = POLYGONS[usize::from(code - 1)];
        let (u, v) = points.iter().copied().fold(points[0], |best, p| {
            if p.0 * out_u + p.1 * out_v > best.0 * out_u + best.1 * out_v {
                p
            } else {
                best
            }
        });
        let (near, far) = (1e-4, 1e-4 + 0.05);
        let (min, max) = match side {
            T => ((u - 0.025, v - far), (u + 0.025, v - near)),
            B => ((u - 0.025, v + near), (u + 0.025, v + far)),
            L => ((u - far, v - 0.025), (u - near, v + 0.025)),
            _ => ((u + near, v - 0.025), (u + far, v + 0.025)),
        };
        let around = [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ];
        !around.into_iter().any(|(nc, nr)| {
            kind_at(col + nc, row + nr).is_some_and(|kind| {
                let shift = |(a, b): (f32, f32)| (a - nc as f32, b - nr as f32);
                box_meets(kind, shift(min), shift(max))
            })
        })
    };
    [
        (T, B, (0, -1)),
        (B, T, (0, 1)),
        (L, R, (-1, 0)),
        (R, L, (1, 0)),
    ]
    .into_iter()
    .filter(|&(side, across, step)| open(side, across, step))
    .fold(0, |mask, (side, ..)| mask | side)
}

#[test]
fn slope_cells_mask_their_legs() {
    // Tile 0 is full and tile `k` slope kind `k`.
    let classes: Vec<TileClass> = (0..=12)
        .map(|slope| TileClass {
            material: 0,
            one_way: false,
            slope,
        })
        .collect();
    let check = |name: &str, width: u32, kinds: &[Option<u8>]| -> Vec<u8> {
        let height = kinds.len() as u32 / width;
        let tiles: Vec<TileIndex> = kinds
            .iter()
            .map(|kind| kind.map_or(-1, TileIndex::from))
            .collect();
        let (runs, masks) = trace_classes(width, height, &tiles, &classes);
        for (at, kind) in kinds.iter().enumerate() {
            let (col, row) = (at as u32 % width, at as u32 / width);
            let Some(kind) = *kind else { continue };
            let want = expected_mask(kinds, width, col, row);
            assert_eq!(masks[at], want, "{name}: ({col}, {row})'s mask");
            if kind != 0 {
                // A slope cell is a run of its own, whatever its mask.
                let on: Vec<&Run> = runs
                    .iter()
                    .filter(|r| {
                        (r.col..r.col + r.cols).contains(&col)
                            && (r.row..r.row + r.rows).contains(&row)
                    })
                    .collect();
                assert_eq!(
                    on,
                    [&run(col, row, 1, 1, want)],
                    "{name}: ({col}, {row})'s run"
                );
            }
        }
        masks
    };
    for code in 1..=12u8 {
        let k = Some(code);
        let (e, f) = (None, Some(0));
        let alone = check(
            &format!("kind {code} alone"),
            3,
            &[e, e, e, e, k, e, e, e, e],
        );
        assert_eq!(alone[4], FACE_ALL, "kind {code} alone");
        let walled = check(
            &format!("kind {code} in full cells"),
            3,
            &[f, f, f, f, k, f, f, f, f],
        );
        assert_eq!(walled[4], 0, "kind {code} in full cells");
        // A full neighbour's face is masked by a full leg alone.
        let full_leg = |side| LEGS[usize::from(code - 1)].contains(&(side, 0.0, 1.0));
        assert_eq!(
            walled[1] & B == 0,
            full_leg(T),
            "kind {code}: the cell above"
        );
        assert_eq!(
            walled[3] & R == 0,
            full_leg(L),
            "kind {code}: the cell left"
        );
        check(
            &format!("kind {code} beside its kind"),
            4,
            &[e, e, e, e, e, k, k, e, e, e, e, e],
        );
        check(
            &format!("kind {code} over its kind"),
            3,
            &[e, e, e, e, k, e, e, k, e, e, e, e],
        );
    }
    // A low cell's half leg meets a high cell's equal one: one 2:1 surface.
    let (e, f) = (None, Some(0));
    for (left, right) in [(5, 9), (10, 6), (7, 11), (12, 8)] {
        let name = format!("{left} beside {right}");
        let masks = check(
            &name,
            4,
            &[e, e, e, e, e, Some(left), Some(right), e, f, f, f, f],
        );
        assert_eq!(masks[5] & R, 0, "{name}: the left cell's right leg");
        assert_eq!(masks[6] & L, 0, "{name}: the right cell's left leg");
    }
    // A full leg covers a half one but not the other way, and halves of
    // either end cover neither.
    let masks = check(
        "9 beside 6",
        4,
        &[e, e, e, e, e, Some(9), Some(6), e, e, e, e, e],
    );
    assert_eq!((masks[5] & R, masks[6] & L), (R, 0), "9 beside 6");
    let masks = check(
        "5 beside 8",
        4,
        &[e, e, e, e, e, Some(5), Some(8), e, e, e, e, e],
    );
    assert_eq!((masks[5] & R, masks[6] & L), (R, L), "5 beside 8");

    // Ramps: each cell's corners where the face goes on into the next cell
    // are shadowed, so a box sliding down meets no lower cell's top corner;
    // the map's edge exposes the end cells' outer legs.
    let (br, tr) = (Some(1), Some(3));
    let masks = check(
        "a 45 floor ramp",
        4,
        &[e, e, e, br, e, e, br, f, e, br, f, f, br, f, f, f],
    );
    let ramp: Vec<u8> = [12, 9, 6, 3].iter().map(|&at| masks[at]).collect();
    assert_eq!(ramp, [L | B, 0, 0, T | R], "a 45 floor ramp, bottom to top");
    let masks = check(
        "a 45 ceiling ramp",
        4,
        &[tr, f, f, f, e, tr, f, f, e, e, tr, f, e, e, e, tr],
    );
    let ramp: Vec<u8> = [0, 5, 10, 15].iter().map(|&at| masks[at]).collect();
    assert_eq!(
        ramp,
        [L | T, 0, 0, B | R],
        "a 45 ceiling ramp, top to bottom"
    );
    let (low, high) = (Some(5), Some(9));
    let masks = check("a 2:1 floor", 4, &[e, e, low, high, low, high, f, f]);
    let surface: Vec<u8> = [4, 5, 2, 3].iter().map(|&at| masks[at]).collect();
    assert_eq!(surface, [L | B, B, 0, T | R], "a 2:1 floor, bottom to top");
    // A low cell's top vertex is on its side, so a full cell over it leaves
    // room for a box on it.
    let masks = check(
        "a low cell under a ceiling",
        3,
        &[f, f, f, e, low, e, e, e, e],
    );
    assert_eq!(masks[4], FACE_ALL, "a low cell under a ceiling");
}
