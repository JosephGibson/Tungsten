/// Elapsed seconds between frames.
#[derive(Debug, Clone, Copy)]
pub struct DeltaTime {
    pub dt: f32,
}

impl DeltaTime {
    #[must_use]
    pub fn new() -> Self {
        Self { dt: 0.0 }
    }

    #[must_use]
    pub fn seconds(&self) -> f32 {
        self.dt
    }
}

impl Default for DeltaTime {
    fn default() -> Self {
        Self::new()
    }
}
