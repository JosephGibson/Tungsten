/// Resource that tracks elapsed time between frames.
#[derive(Debug, Clone, Copy)]
pub struct DeltaTime {
    /// Seconds elapsed since last frame.
    pub dt: f32,
}

impl DeltaTime {
    pub fn new() -> Self {
        Self { dt: 0.0 }
    }

    pub fn seconds(&self) -> f32 {
        self.dt
    }
}

impl Default for DeltaTime {
    fn default() -> Self {
        Self::new()
    }
}
