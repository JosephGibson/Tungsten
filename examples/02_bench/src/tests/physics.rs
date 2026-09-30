use super::*;
use crate::knobs::resolve;

#[test]
fn pachinko_invariants_hold_for_every_radius_mix() {
    for mix in ["uniform", "mixed", "wide"] {
        let cfg = resolve(&BENCH, None, None, Some(&format!("radius_mix={mix}"))).unwrap();
        let board = Board::new(&cfg);
        let pegs = board.pegs();
        let gap = |a: Vec2, b: Vec2| a.distance(b) - 2.0 * board.peg_radius;

        // Same row: neighbors one pitch apart; next row: the diagonal pegs.
        let mut row_gap = f32::INFINITY;
        let mut diagonal_gap = f32::INFINITY;
        for (index, peg) in pegs.iter().enumerate() {
            for other in &pegs[index + 1..] {
                if other.y == peg.y {
                    row_gap = row_gap.min(gap(*peg, *other));
                } else if (other.y - peg.y - board.pitch_y).abs() < 1e-3 {
                    diagonal_gap = diagonal_gap.min(gap(*peg, *other));
                }
            }
        }
        assert!(row_gap >= 3.0 * board.d_max, "{mix}: row gap {row_gap}");
        assert!(
            diagonal_gap >= 2.5 * board.d_max,
            "{mix}: diagonal gap {diagonal_gap}"
        );

        let view = VIEWPORT / board.zoom();
        assert!(
            board.width <= view.x && board.height <= view.y,
            "{mix}: board {}x{} exceeds view {view}",
            board.width,
            board.height
        );

        let balls = board.place_balls(cfg.int("balls") as usize, cfg.seed());
        assert_eq!(balls.len(), 8_000, "{mix}");
        for ball in &balls {
            let (center, radius) = (ball.center, ball.radius);
            assert!(
                center.x >= radius
                    && center.x <= board.width - radius
                    && center.y >= radius
                    && center.y <= board.height - radius,
                "{mix}: ball at {center} leaves the board"
            );
            for peg in &pegs {
                assert!(
                    peg.distance(center) >= radius + board.peg_radius,
                    "{mix}: ball at {center} overlaps peg {peg}"
                );
            }
        }
        // Sweep along x: only balls closer than d_max in x can overlap.
        let mut order: Vec<&PlacedBall> = balls.iter().collect();
        order.sort_by(|a, b| a.center.x.total_cmp(&b.center.x));
        for (index, ball) in order.iter().enumerate() {
            for other in &order[index + 1..] {
                if other.center.x - ball.center.x >= board.d_max {
                    break;
                }
                assert!(
                    ball.center.distance(other.center) >= ball.radius + other.radius,
                    "{mix}: balls at {} and {} overlap",
                    ball.center,
                    other.center
                );
            }
        }
    }
}
