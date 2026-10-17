//! Motion: the controllers, the path follower, the movers, placers and
//! constraints of the kit's motion write rule, the targets they share and
//! the cast-and-slide the controllers share.

pub(crate) mod area;
pub(crate) mod drive;
pub(crate) mod follow;
pub(crate) mod knockback;
pub(crate) mod linear;
pub(crate) mod orbit;
pub(crate) mod oscillator;
pub(crate) mod path;
pub(crate) mod platformer;
pub(crate) mod slide;
pub(crate) mod speed_limit;
pub(crate) mod stay_in_view;
pub(crate) mod steering;
pub(crate) mod target;
pub(crate) mod top_down;
pub(crate) mod wrap;
