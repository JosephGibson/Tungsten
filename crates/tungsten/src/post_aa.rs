//! Runtime post-AA request seam (M27).
//!
//! Mirrors the display request/apply shape: user systems see `&mut World`
//! (not `&mut App`), so they request a mode change by writing a pending world
//! resource. `App::apply_pending_post_aa_request` consumes it at a frame
//! boundary — after hot-reload, before extract — so renderer reallocation
//! never happens mid-frame.

use tungsten_core::config::PostAaMode;
use tungsten_core::ecs::World;

/// Last applied post-AA mode. Mirrors `Renderer::post_aa()`. HUD reads this so
/// it reflects the renderer state, not just the latest keypress.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PostAaState {
    pub mode: PostAaMode,
}

/// Pending request written by `request_post_aa`. Consumed at the frame
/// boundary by `App::apply_pending_post_aa_request`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PendingPostAa(pub(crate) Option<PostAaMode>);

/// Request a post-AA mode change. Applied at the next frame boundary so
/// SMAA intermediate textures are reallocated outside any active render pass.
pub fn request_post_aa(world: &mut World, mode: PostAaMode) {
    if let Some(pending) = world.get_resource_mut::<PendingPostAa>() {
        pending.0 = Some(mode);
    } else {
        world.insert_resource(PendingPostAa(Some(mode)));
    }
}

pub(crate) fn take_pending_post_aa(world: &mut World) -> Option<PostAaMode> {
    world
        .get_resource_mut::<PendingPostAa>()
        .and_then(|pending| pending.0.take())
}

pub(crate) fn sync_post_aa_state(world: &mut World, mode: PostAaMode) {
    if let Some(state) = world.get_resource_mut::<PostAaState>() {
        state.mode = mode;
    } else {
        world.insert_resource(PostAaState { mode });
    }
}

#[cfg(test)]
#[path = "tests/post_aa.rs"]
mod tests;
