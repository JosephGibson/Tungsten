/// Opaque entity identifier. Uses `u32` for simplicity in Phase 1.
/// Decision: locked at M2 exit per DESIGN.md open question 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Entity(pub(crate) u32);

impl Entity {
    pub fn id(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Entity({})", self.0)
    }
}
