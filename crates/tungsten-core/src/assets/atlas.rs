//! CPU-side shelf atlas packer; no I/O, no `wgpu`.
//!
//! Order: sort by height desc, width desc, id asc; return sprites in input order.
//! Mip invariant: half-texel inset + 1 px padding assumes non-mip sampling.

/// Normalized UV rectangle on one atlas page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvRect {
    pub min: [f32; 2],
    pub max: [f32; 2],
}

impl UvRect {
    /// Full page rectangle.
    pub const FULL: Self = Self {
        min: [0.0, 0.0],
        max: [1.0, 1.0],
    };
}

/// Packer input sprite dimensions.
#[derive(Debug, Clone, Copy)]
pub struct PackInput<'a> {
    pub id: &'a str,
    pub width: u32,
    pub height: u32,
}

/// Packed sprite page and pixel rect.
#[derive(Debug, Clone, PartialEq)]
pub struct PackedSprite {
    pub id: String,
    pub page: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Atlas page dimensions in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtlasPage {
    pub width: u32,
    pub height: u32,
}

/// Packed pages and sprites.
#[derive(Debug, Clone, PartialEq)]
pub struct PackResult {
    pub pages: Vec<AtlasPage>,
    pub sprites: Vec<PackedSprite>,
}

/// Run deterministic shelf-next-fit packer.
///
/// # Panics
///
/// Panics when a sprite exceeds `max_dim - 2 * padding`.
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

    // Sort copy; emit original order.
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

    let mut placements: Vec<Option<PagePlacement>> = vec![None; inputs.len()];
    let mut pages: Vec<AtlasPage> = Vec::new();

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

        if shelf_height == 0 {
            shelf_height = cell_h;
        }

        if shelf_x + cell_w > max_dim {
            let new_shelf_y = shelf_y + shelf_height;
            if new_shelf_y + cell_h > max_dim {
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
#[path = "../tests/assets/atlas.rs"]
mod tests;
