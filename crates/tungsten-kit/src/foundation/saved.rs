//! The kit's saves (redo 7.9): each kit plugin registers, in its `build`,
//! the components and resources that hold state play changes, under their
//! scene names; [`UNSAVED_COMPONENTS`] names the described components it
//! does not save, each with its reason.

/// A component a prefab gives and play does not change.
const RULE: &str = "rule data a prefab gives";
/// Presentation, which a load restarts.
const PRESENTATION: &str = "presentation, which a load restarts";
/// The hierarchy's: its children come back from their parent's fresh spawn.
const HIERARCHY: &str = "the hierarchy: children come back from their parent's fresh spawn";

/// The described components the kit does not save, by scene name, each
/// with its reason; every other described component is saved, so a game
/// that changes one of these in play registers it itself.
pub const UNSAVED_COMPONENTS: &[(&str, &str)] = &[
    ("action_input", RULE),
    ("ailment_threshold", RULE),
    ("animation", PRESENTATION),
    ("camera_target", RULE),
    ("character_animator", PRESENTATION),
    ("conversion", RULE),
    ("cues", RULE),
    ("depth_sort", PRESENTATION),
    ("despawn_outside", RULE),
    ("hit_flash", PRESENTATION),
    ("hurtbox", RULE),
    ("penetration", RULE),
    ("spawn_children", HIERARCHY),
    ("speed_limit", RULE),
    ("stay_in_view", RULE),
    ("wrap", RULE),
    ("zone", RULE),
];

#[cfg(test)]
#[path = "../tests/saved.rs"]
mod tests;
