use super::*;

#[test]
fn area_bin_byte_length_matches_format() {
    assert_eq!(area_bytes().len(), AREA_TEX_LEN);
    assert_eq!(AREA_TEX_LEN, 160 * 560 * 2);
}

#[test]
fn search_bin_byte_length_matches_format() {
    assert_eq!(search_bytes().len(), SEARCH_TEX_LEN);
    assert_eq!(SEARCH_TEX_LEN, 64 * 16);
}
