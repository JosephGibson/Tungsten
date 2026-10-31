use super::*;
use crate::physics::collision::{FACE_ALL, FACE_BOTTOM, FACE_LEFT, FACE_RIGHT, FACE_TOP};

/// Tiles by letter: `#` plain, `a` and `b` of materials 1 and 2, `=`
/// one-way, `.` empty.
const CLASSES: [TileClass; 4] = [
    TileClass {
        material: 0,
        one_way: false,
    },
    TileClass {
        material: 1,
        one_way: false,
    },
    TileClass {
        material: 2,
        one_way: false,
    },
    TileClass {
        material: 0,
        one_way: true,
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
    let cells = LayerCells {
        width,
        height,
        tiles,
        classes: &CLASSES,
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
