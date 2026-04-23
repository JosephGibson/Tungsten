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
    // Third sprite forces page overflow: 130 + 130 > 256.
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
