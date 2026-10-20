use super::*;

const BUILD_ID: &[u8] = &[0xde, 0xad, 0xbe, 0xef, 1, 2, 3, 4];

/// A 64-bit little-endian ELF: a GNU build-id note when given, then the
/// named sections, as `scripts/test-release.py`'s `synthetic_elf` builds it.
fn synthetic_elf(build_id: Option<&[u8]>, named: &[&str]) -> Vec<u8> {
    let mut body: Vec<(String, u32, Vec<u8>, u64)> = Vec::new();
    if let Some(id) = build_id {
        let mut note = Vec::new();
        for n in [4u32, id.len() as u32, 3] {
            note.extend_from_slice(&n.to_le_bytes());
        }
        note.extend_from_slice(b"GNU\0");
        note.extend_from_slice(id);
        while note.len() % 4 != 0 {
            note.push(0);
        }
        body.push((".note.gnu.build-id".to_owned(), 7, note, 4));
    }
    for name in named {
        body.push(((*name).to_owned(), 1, vec![1; 16], 1));
    }
    let mut names = vec![0u8];
    let mut offsets = Vec::new();
    for name in body
        .iter()
        .map(|b| b.0.clone())
        .chain([".shstrtab".to_owned()])
    {
        offsets.push(names.len() as u32);
        names.extend_from_slice(name.as_bytes());
        names.push(0);
    }
    body.push((".shstrtab".to_owned(), 3, names, 1));
    let mut data = vec![0u8; 64];
    let mut headers = vec![vec![0u8; 64]];
    for ((_, kind, content, align), name) in body.iter().zip(&offsets) {
        let mut header = Vec::new();
        header.extend_from_slice(&name.to_le_bytes());
        header.extend_from_slice(&kind.to_le_bytes());
        header.extend_from_slice(&0u64.to_le_bytes());
        header.extend_from_slice(&0u64.to_le_bytes());
        header.extend_from_slice(&(data.len() as u64).to_le_bytes());
        header.extend_from_slice(&(content.len() as u64).to_le_bytes());
        header.extend_from_slice(&0u32.to_le_bytes());
        header.extend_from_slice(&0u32.to_le_bytes());
        header.extend_from_slice(&align.to_le_bytes());
        header.extend_from_slice(&0u64.to_le_bytes());
        headers.push(header);
        data.extend_from_slice(content);
    }
    let shoff = data.len() as u64;
    let count = headers.len() as u16;
    for header in headers {
        data.extend_from_slice(&header);
    }
    data[..4].copy_from_slice(b"\x7fELF");
    data[4] = 2;
    data[5] = 1;
    data[0x28..0x30].copy_from_slice(&shoff.to_le_bytes());
    data[0x3A..0x3C].copy_from_slice(&64u16.to_le_bytes());
    data[0x3C..0x3E].copy_from_slice(&count.to_le_bytes());
    data[0x3E..0x40].copy_from_slice(&(count - 1).to_le_bytes());
    data
}

#[test]
fn the_build_id_note_reads_back() {
    let elf = synthetic_elf(Some(BUILD_ID), &[".text"]);
    assert_eq!(build_id(&elf), Ok(Some(BUILD_ID.to_vec())));
    assert!(sections(&elf).unwrap().contains_key(".text"));
}

#[test]
fn no_note_is_no_build_id() {
    let elf = synthetic_elf(None, &[".text"]);
    assert_eq!(build_id(&elf), Ok(None));
    assert_eq!(check_split(&elf, &elf), Err(SplitError::NoBuildId));
}

#[test]
fn a_split_needs_equal_ids_and_line_tables() {
    let binary = synthetic_elf(Some(BUILD_ID), &[".text"]);
    let debug = synthetic_elf(Some(BUILD_ID), &[".debug_line", ".debug_info"]);
    assert_eq!(check_split(&binary, &debug), Ok(()));
    let bare = synthetic_elf(Some(BUILD_ID), &[".debug_info"]);
    assert_eq!(check_split(&binary, &bare), Err(SplitError::NoLineTables));
    let other = synthetic_elf(Some(&[9; 8]), &[".debug_line"]);
    assert!(matches!(
        check_split(&binary, &other),
        Err(SplitError::Mismatch { ref binary, ref debug }) if binary == "deadbeef01020304" && debug == "0909090909090909"
    ));
}

#[test]
fn a_truncated_header_is_malformed() {
    let elf = synthetic_elf(Some(BUILD_ID), &[".text"]);
    assert!(matches!(
        sections(&elf[..80]),
        Err(SplitError::Malformed(_))
    ));
    assert!(matches!(
        sections(b"MZ not an elf"),
        Err(SplitError::Malformed(_))
    ));
    assert!(matches!(
        build_id(&elf[..10]),
        Err(SplitError::Malformed(_))
    ));
}
