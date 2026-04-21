//! CPU-side sprite atlas packer (M22).
//!
//! Pure-data module: no `wgpu`, no I/O. Given a slice of [`PackInput`] sized
//! rectangles and a `max_dim`/`padding` budget, returns a [`PackResult`] that
//! names one or more power-of-two [`AtlasPage`]s and places every sprite on
//! exactly one page with `(x, y, width, height)` coordinates.
//!
//! ## Algorithm — shelf next-fit, deterministic tie-break
//!
//! A stable copy of `inputs` is sorted by `(height desc, width desc, id asc)`.
//! The packer opens shelves top-to-bottom inside the current page; each sprite
//! is placed on the rightmost position of the current shelf that still fits.
//! When a sprite would overflow the shelf horizontally, a new shelf of the new
//! sprite's height is started below. When the new shelf would overflow the
//! page vertically, the page is finalised and a new page opens.
//!
//! Per-sprite padding of `padding` pixels is baked in on every side — the
//! drawn texels are placed at `(x + padding/2, y + padding/2)` with the
//! sprite width/height. Callers apply a half-texel UV inset so bilinear
//! sampling cannot reach the padding column.
//!
//! Page dimensions are powers of two (`next_power_of_two` of the observed
//! extent), clamped to `max_dim`. Sprites are returned in the original
//! `inputs` order for stable iteration by caller code.
//!
//! ## Overflow
//!
//! A sprite whose width or height exceeds `max_dim - 2 * padding` panics
//! with a clear message — at item granularity an atlas layout is
//! unrecoverable without switching strategy (array textures, BC-compressed
//! pages, etc. — all explicit Phase 4 non-goals).
//!
//! ## Mipmaps
//!
//! The half-texel UV inset plus 1 px transparent padding suppress neighbour
//! bleed at non-mip bilinear sampling. Mipmaps are out of scope; enabling
//! them in the future needs either wider padding or dedicated per-sprite
//! sub-textures — do not silently turn mips on without revisiting this
//! module.

/// A normalised UV rectangle on a single atlas page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvRect {
    pub min: [f32; 2],
    pub max: [f32; 2],
}

impl UvRect {
    /// The full `[0,0]..[1,1]` rectangle — useful for callers that want the
    /// pipeline to behave as if the texture is their own private sprite.
    pub const FULL: Self = Self {
        min: [0.0, 0.0],
        max: [1.0, 1.0],
    };
}

/// Input to the packer: a logical sprite id plus its source dimensions.
#[derive(Debug, Clone, Copy)]
pub struct PackInput<'a> {
    pub id: &'a str,
    pub width: u32,
    pub height: u32,
}

/// Output of the packer: the page index and pixel rect for a single sprite.
#[derive(Debug, Clone, PartialEq)]
pub struct PackedSprite {
    pub id: String,
    pub page: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Dimensions of a packed atlas page (in pixels).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtlasPage {
    pub width: u32,
    pub height: u32,
}

/// Complete pack result: one entry per page, one entry per packed sprite.
#[derive(Debug, Clone, PartialEq)]
pub struct PackResult {
    pub pages: Vec<AtlasPage>,
    pub sprites: Vec<PackedSprite>,
}

/// Run the shelf-next-fit packer.
///
/// # Panics
///
/// Panics when any single sprite exceeds `max_dim - 2 * padding` on either
/// axis — the caller cannot recover from that without a new strategy, and
/// downgrading to per-sprite textures would defeat the point of this
/// milestone.
pub fn pack_shelf(inputs: &[PackInput<'_>], max_dim: u32, padding: u32) -> PackResult {
    if inputs.is_empty() {
        return PackResult {
            pages: Vec::new(),
            sprites: Vec::new(),
        };
    }

    let pad = padding;
    let max_usable = max_dim.saturating_sub(2 * pad);
    for input in inputs {
        assert!(
            input.width <= max_usable && input.height <= max_usable,
            "sprite '{}' ({}x{}) exceeds max atlas page dimension {} (padding {})",
            input.id,
            input.width,
            input.height,
            max_dim,
            pad
        );
    }

    // Stable copy, indexed so we can return sprites in original order.
    let mut ordered: Vec<(usize, PackInput<'_>)> = inputs.iter().copied().enumerate().collect();
    ordered.sort_by(|a, b| {
        b.1.height
            .cmp(&a.1.height)
            .then_with(|| b.1.width.cmp(&a.1.width))
            .then_with(|| a.1.id.cmp(b.1.id))
    });

    #[derive(Clone, Copy)]
    struct PagePlacement {
        page: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    }

    // Page accumulator state.
    let mut placements: Vec<Option<PagePlacement>> = vec![None; inputs.len()];
    let mut pages: Vec<AtlasPage> = Vec::new();

    // Current page's shelf geometry.
    let mut cur_page: u32 = 0;
    let mut shelf_y: u32 = pad;
    let mut shelf_x: u32 = pad;
    let mut shelf_height: u32 = 0;
    let mut page_extent_x: u32 = 0;
    let mut page_extent_y: u32 = 0;
    let mut page_open = false;

    let open_page = |cur_page: &mut u32,
                     shelf_y: &mut u32,
                     shelf_x: &mut u32,
                     shelf_height: &mut u32,
                     page_extent_x: &mut u32,
                     page_extent_y: &mut u32,
                     page_open: &mut bool,
                     first: bool| {
        if !first {
            *cur_page += 1;
        }
        *shelf_y = pad;
        *shelf_x = pad;
        *shelf_height = 0;
        *page_extent_x = 0;
        *page_extent_y = 0;
        *page_open = true;
    };

    let finalise_page = |pages: &mut Vec<AtlasPage>, page_extent_x: u32, page_extent_y: u32| {
        let w = next_power_of_two_clamped(page_extent_x.max(1), max_dim);
        let h = next_power_of_two_clamped(page_extent_y.max(1), max_dim);
        pages.push(AtlasPage {
            width: w,
            height: h,
        });
    };

    for (orig_idx, input) in &ordered {
        if !page_open {
            open_page(
                &mut cur_page,
                &mut shelf_y,
                &mut shelf_x,
                &mut shelf_height,
                &mut page_extent_x,
                &mut page_extent_y,
                &mut page_open,
                pages.is_empty(),
            );
        }

        let cell_w = input.width + pad;
        let cell_h = input.height + pad;

        // Opening a fresh shelf (shelf_height == 0) sets its height to this sprite's.
        if shelf_height == 0 {
            shelf_height = cell_h;
        }

        // If adding to the current shelf would overflow in X, close the shelf
        // and move down.
        if shelf_x + cell_w > max_dim {
            let new_shelf_y = shelf_y + shelf_height;
            if new_shelf_y + cell_h > max_dim {
                // Page vertical overflow: finalise this page and open a new one.
                finalise_page(&mut pages, page_extent_x, page_extent_y);
                open_page(
                    &mut cur_page,
                    &mut shelf_y,
                    &mut shelf_x,
                    &mut shelf_height,
                    &mut page_extent_x,
                    &mut page_extent_y,
                    &mut page_open,
                    false,
                );
                shelf_height = cell_h;
            } else {
                shelf_y = new_shelf_y;
                shelf_x = pad;
                shelf_height = cell_h;
            }
        }

        placements[*orig_idx] = Some(PagePlacement {
            page: cur_page,
            x: shelf_x,
            y: shelf_y,
            width: input.width,
            height: input.height,
        });

        shelf_x += cell_w;
        let used_x = shelf_x; // right edge (with trailing padding)
        let used_y = shelf_y + shelf_height; // bottom edge (with trailing padding)
        if used_x > page_extent_x {
            page_extent_x = used_x;
        }
        if used_y > page_extent_y {
            page_extent_y = used_y;
        }
    }

    if page_open {
        finalise_page(&mut pages, page_extent_x, page_extent_y);
    }

    let sprites = placements
        .into_iter()
        .zip(inputs.iter())
        .map(|(p, inp)| {
            let p = p.expect("every input must receive a placement");
            PackedSprite {
                id: inp.id.to_string(),
                page: p.page,
                x: p.x,
                y: p.y,
                width: p.width,
                height: p.height,
            }
        })
        .collect();

    PackResult { pages, sprites }
}

fn next_power_of_two_clamped(value: u32, max: u32) -> u32 {
    let pow = value.next_power_of_two();
    pow.min(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pi<'a>(id: &'a str, w: u32, h: u32) -> PackInput<'a> {
        PackInput {
            id,
            width: w,
            height: h,
        }
    }

    #[test]
    fn empty_input_produces_empty_result() {
        let result = pack_shelf(&[], 1024, 1);
        assert_eq!(
            result,
            PackResult {
                pages: Vec::new(),
                sprites: Vec::new(),
            }
        );
    }

    #[test]
    fn single_small_sprite_lands_at_padding_origin() {
        let inputs = [pi("a", 16, 16)];
        let result = pack_shelf(&inputs, 1024, 1);
        assert_eq!(result.pages.len(), 1);
        assert_eq!(result.sprites.len(), 1);
        let s = &result.sprites[0];
        assert_eq!(s.page, 0);
        assert_eq!((s.x, s.y), (1, 1));
        assert_eq!((s.width, s.height), (16, 16));
    }

    #[test]
    fn two_equal_sprites_share_a_page() {
        let inputs = [pi("a", 128, 128), pi("b", 128, 128)];
        let result = pack_shelf(&inputs, 256, 0);
        assert_eq!(result.pages.len(), 1);
        assert!(result.sprites.iter().all(|s| s.page == 0));
        let xs: Vec<_> = result.sprites.iter().map(|s| s.x).collect();
        assert_eq!(xs.iter().filter(|&&x| x == 0).count(), 1);
        assert_eq!(xs.iter().filter(|&&x| x == 128).count(), 1);
    }

    #[test]
    fn three_sprites_overflow_to_two_pages() {
        // Three 128-wide, 130-tall sprites in a 256-wide page: two share the
        // first shelf (2 × 128 == 256 width), the third forces a second shelf
        // whose bottom (130 + 130 = 260) exceeds max_dim → new page.
        let inputs = [pi("a", 128, 130), pi("b", 128, 130), pi("c", 128, 130)];
        let result = pack_shelf(&inputs, 256, 0);
        assert_eq!(result.pages.len(), 2);
        let page0 = result.sprites.iter().filter(|s| s.page == 0).count();
        let page1 = result.sprites.iter().filter(|s| s.page == 1).count();
        assert_eq!(page0, 2);
        assert_eq!(page1, 1);
    }

    #[test]
    #[should_panic(expected = "exceeds max atlas page dimension")]
    fn single_sprite_exceeding_max_panics() {
        let inputs = [pi("huge", 200, 200)];
        // max_dim = 256, padding = 1 → usable 254; 200 <= 254 passes, so use
        // a larger sprite to force the panic path.
        let _ = pack_shelf(&[pi("huge", 256, 256)], 256, 1);
        let _ = inputs;
    }

    #[test]
    fn determinism_same_input_same_output() {
        let a = [pi("a", 16, 16), pi("b", 32, 8), pi("c", 24, 24)];
        let r1 = pack_shelf(&a, 128, 1);
        let r2 = pack_shelf(&a, 128, 1);
        assert_eq!(r1, r2);
    }
}
