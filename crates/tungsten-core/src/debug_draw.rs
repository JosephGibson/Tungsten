//! D-007/D-016 debug draw commands; core stays renderer-free.

use glam::Vec2;

/// Default circle polyline segment count.
pub const DEFAULT_CIRCLE_SEGMENTS: u16 = 24;

/// World-space debug primitive shape.
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

/// Queued debug draw command.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DebugCommand {
    pub shape: DebugShape,
    pub color: [f32; 4],
    pub thickness: f32,
}

/// Debug command accumulator resource.
#[derive(Debug, Default)]
pub struct DebugDraw {
    cmds: Vec<DebugCommand>,
}

impl DebugDraw {
    #[must_use]
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

    #[must_use]
    pub fn commands(&self) -> &[DebugCommand] {
        &self.cmds
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.cmds.len()
    }
}

#[cfg(test)]
#[path = "tests/debug_draw.rs"]
mod tests;
