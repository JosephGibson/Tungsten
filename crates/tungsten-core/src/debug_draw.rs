//! `DebugDraw` resource (M21): pure POD debug primitives produced by systems
//! and drained by the umbrella crate's extract stage. `tungsten-core` stays
//! free of `wgpu`/`winit` types per `D-007` / `D-016`; the render seam is
//! crossed by expanding commands into `QuadInstance` (AABB edges) and
//! `DebugLineInstance` (arbitrary-angle lines, circle polylines) POD slices
//! inside `tungsten`.

use glam::Vec2;

/// Default polyline segment count used for `draw_circle` commands.
pub const DEFAULT_CIRCLE_SEGMENTS: u16 = 24;

/// Debug primitive shape. All fields are world-space; thickness is carried on
/// `DebugCommand`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DebugShape {
    Aabb {
        min: Vec2,
        max: Vec2,
    },
    Circle {
        center: Vec2,
        radius: f32,
        segments: u16,
    },
    Line {
        a: Vec2,
        b: Vec2,
    },
}

/// One queued debug draw command. `color` is linear-space RGBA; `thickness`
/// is in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DebugCommand {
    pub shape: DebugShape,
    pub color: [f32; 4],
    pub thickness: f32,
}

/// Accumulator resource. Systems push commands; the extract stage drains and
/// clears the vector before the render stage consumes the expansion.
#[derive(Debug, Default)]
pub struct DebugDraw {
    cmds: Vec<DebugCommand>,
}

impl DebugDraw {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn draw_aabb(&mut self, min: Vec2, max: Vec2, color: [f32; 4], thickness: f32) {
        self.cmds.push(DebugCommand {
            shape: DebugShape::Aabb { min, max },
            color,
            thickness,
        });
    }

    pub fn draw_circle(&mut self, center: Vec2, radius: f32, color: [f32; 4], thickness: f32) {
        self.cmds.push(DebugCommand {
            shape: DebugShape::Circle {
                center,
                radius,
                segments: DEFAULT_CIRCLE_SEGMENTS,
            },
            color,
            thickness,
        });
    }

    pub fn draw_circle_with_segments(
        &mut self,
        center: Vec2,
        radius: f32,
        segments: u16,
        color: [f32; 4],
        thickness: f32,
    ) {
        self.cmds.push(DebugCommand {
            shape: DebugShape::Circle {
                center,
                radius,
                segments,
            },
            color,
            thickness,
        });
    }

    pub fn draw_line(&mut self, a: Vec2, b: Vec2, color: [f32; 4], thickness: f32) {
        self.cmds.push(DebugCommand {
            shape: DebugShape::Line { a, b },
            color,
            thickness,
        });
    }

    pub fn clear(&mut self) {
        self.cmds.clear();
    }

    pub fn drain(&mut self) -> std::vec::Drain<'_, DebugCommand> {
        self.cmds.drain(..)
    }

    pub fn commands(&self) -> &[DebugCommand] {
        &self.cmds
    }

    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }

    pub fn len(&self) -> usize {
        self.cmds.len()
    }
}

#[cfg(test)]
#[path = "tests/debug_draw.rs"]
mod tests;
