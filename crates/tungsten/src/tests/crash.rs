use super::*;
use std::io::Cursor;

const BUILD_ID: [u8; 20] = [
    0x1a, 0x2b, 0x3c, 0x4d, 0x5e, 0x6f, 0x70, 0x81, 0x92, 0xa3, 0xb4, 0xc5, 0xd6, 0xe7, 0xf8, 0x09,
    0x10, 0x21, 0x32, 0x43,
];
const GUID: [u8; 16] = [
    0x33, 0x22, 0x11, 0x00, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
];

fn put(bytes: &mut Vec<u8>, at: usize, value: &[u8]) {
    if bytes.len() < at + value.len() {
        bytes.resize(at + value.len(), 0);
    }
    bytes[at..at + value.len()].copy_from_slice(value);
}

/// One ELF note: sizes, type, the name and the description, each padded
/// to four bytes.
fn note(name: &[u8], kind: u32, desc: &[u8]) -> Vec<u8> {
    let mut note = Vec::new();
    note.extend_from_slice(&(name.len() as u32).to_le_bytes());
    note.extend_from_slice(&(desc.len() as u32).to_le_bytes());
    note.extend_from_slice(&kind.to_le_bytes());
    note.extend_from_slice(name);
    note.resize(note.len().next_multiple_of(4), 0);
    note.extend_from_slice(desc);
    note.resize(note.len().next_multiple_of(4), 0);
    note
}

/// A 64-bit little-endian ELF whose one program header, `kind`, covers
/// `notes`.
fn elf(kind: u32, notes: &[u8]) -> Vec<u8> {
    let mut elf = Vec::new();
    put(&mut elf, 0, b"\x7fELF\x02\x01\x01");
    put(&mut elf, 0x20, &64u64.to_le_bytes());
    put(&mut elf, 0x36, &56u16.to_le_bytes());
    put(&mut elf, 0x38, &1u16.to_le_bytes());
    put(&mut elf, 64, &kind.to_le_bytes());
    put(&mut elf, 64 + 8, &128u64.to_le_bytes());
    put(&mut elf, 64 + 32, &(notes.len() as u64).to_le_bytes());
    put(&mut elf, 64 + 48, &4u64.to_le_bytes());
    put(&mut elf, 128, notes);
    elf
}

/// A PE32+ file with one section holding its debug directory and a
/// CodeView record of `magic`.
fn pe(magic: [u8; 4]) -> Vec<u8> {
    let pe = 0x40usize;
    let optional = pe + 24;
    let optional_size = 240usize;
    let sections = optional + optional_size;
    let mut file = Vec::new();
    put(&mut file, 0, b"MZ");
    put(&mut file, 0x3c, &(pe as u32).to_le_bytes());
    put(&mut file, pe, b"PE\0\0");
    put(&mut file, pe + 4, &0x8664u16.to_le_bytes());
    put(&mut file, pe + 6, &1u16.to_le_bytes());
    put(&mut file, pe + 20, &(optional_size as u16).to_le_bytes());
    put(&mut file, optional, &0x20bu16.to_le_bytes());
    put(&mut file, optional + 108, &16u32.to_le_bytes());
    put(&mut file, optional + 112 + 8 * 6, &0x1000u32.to_le_bytes());
    put(&mut file, optional + 112 + 8 * 6 + 4, &28u32.to_le_bytes());
    put(&mut file, sections, b".rdata\0\0");
    put(&mut file, sections + 8, &0x200u32.to_le_bytes());
    put(&mut file, sections + 12, &0x1000u32.to_le_bytes());
    put(&mut file, sections + 16, &0x200u32.to_le_bytes());
    put(&mut file, sections + 20, &0x400u32.to_le_bytes());
    let name = b"D:\\build\\release\\deps\\game.pdb\0";
    let record = 0x400 + 28;
    put(&mut file, 0x400 + 12, &2u32.to_le_bytes());
    put(
        &mut file,
        0x400 + 16,
        &(24 + name.len() as u32).to_le_bytes(),
    );
    put(&mut file, 0x400 + 24, &(record as u32).to_le_bytes());
    put(&mut file, record, &magic);
    put(&mut file, record + 4, &GUID);
    put(&mut file, record + 20, &3u32.to_le_bytes());
    put(&mut file, record + 24, name);
    file
}

fn read(bytes: Vec<u8>) -> Option<BuildId> {
    read_build_id(&mut Cursor::new(bytes))
}

#[test]
fn the_elf_reader_finds_the_gnu_build_id_after_other_notes() {
    let mut notes = note(b"GNU\0", 5, &[1, 2, 3, 4, 5, 6, 7, 8]);
    notes.extend(note(b"GNU\0", NT_GNU_BUILD_ID, &BUILD_ID));
    let id = read(elf(PT_NOTE, &notes)).unwrap();
    assert_eq!(id, BuildId::Gnu(BUILD_ID.to_vec()));
    assert_eq!(id.to_string(), "1a2b3c4d5e6f708192a3b4c5d6e7f80910213243");
}

#[test]
fn the_elf_reader_finds_nothing_without_the_note() {
    let other = note(b"GNU\0", 5, &[1, 2, 3, 4]);
    assert_eq!(read(elf(PT_NOTE, &other)), None);
    let named_otherwise = note(b"Go\0\0", NT_GNU_BUILD_ID, &BUILD_ID);
    assert_eq!(read(elf(PT_NOTE, &named_otherwise)), None);
    // The note outside any PT_NOTE segment is not read.
    let id = note(b"GNU\0", NT_GNU_BUILD_ID, &BUILD_ID);
    assert_eq!(read(elf(1, &id)), None);
    let mut truncated = elf(PT_NOTE, &id);
    truncated.truncate(140);
    assert_eq!(read(truncated), None);
}

#[test]
fn the_pe_reader_finds_the_codeview_guid_age_and_name() {
    let id = read(pe(*b"RSDS")).unwrap();
    assert_eq!(
        id,
        BuildId::Pdb {
            guid: GUID,
            age: 3,
            name: r"D:\build\release\deps\game.pdb".to_string(),
        }
    );
    assert_eq!(
        id.to_string(),
        r"00112233445566778899AABBCCDDEEFF3 D:\build\release\deps\game.pdb"
    );
}

#[test]
fn the_pe_reader_finds_nothing_without_a_codeview_record() {
    assert_eq!(read(pe(*b"NB10")), None);
    let mut no_signature = pe(*b"RSDS");
    put(&mut no_signature, 0x40, b"NE\0\0");
    assert_eq!(read(no_signature), None);
}

#[test]
fn other_files_have_no_identity() {
    assert_eq!(read(b"#!/bin/sh\necho hi\n".to_vec()), None);
    assert_eq!(read(Vec::new()), None);
    assert_eq!(read(b"MZ".to_vec()), None);
}

fn fields<'a>(build_id: Option<&'a BuildId>, modules: Option<&'a str>) -> ReportFields<'a> {
    ReportFields {
        game: Some("my-game"),
        game_version: None,
        executable: "/games/my-game/bin/x86-64/my-game",
        time: "20261005T120000.000Z",
        thread: "main",
        message: "boom\nsecond line",
        location: Some("src/main.rs:10:5"),
        anchor: 0x55d0_c0a1_b2c3,
        build_id,
        backtrace: "   0:     0x55d0c0a1b2c3 - my_game::main\n",
        modules,
    }
}

#[test]
fn the_report_lays_out_its_fields_then_the_backtrace_and_modules() {
    let id = BuildId::Gnu(BUILD_ID.to_vec());
    let modules = "55d0c09f0000-55d0c0a00000 r--p 00000000 103:02 7 /games/my-game\n";
    let report = format_report(&fields(Some(&id), Some(modules)));
    let expected = format!(
        "Tungsten crash report\n\
         game: my-game\n\
         game_version: (none)\n\
         engine_version: {}\n\
         target: {}\n\
         executable: /games/my-game/bin/x86-64/my-game\n\
         time: 20261005T120000.000Z\n\
         thread: main\n\
         message: boom\\nsecond line\n\
         location: src/main.rs:10:5\n\
         anchor: 0x55d0c0a1b2c3 tungsten::crash::anchor\n\
         build_id: 1a2b3c4d5e6f708192a3b4c5d6e7f80910213243\n\
         backtrace:\n   0:     0x55d0c0a1b2c3 - my_game::main\n\
         modules:\n{modules}",
        env!("CARGO_PKG_VERSION"),
        env!("TUNGSTEN_TARGET"),
    );
    assert_eq!(report, expected);
}

#[test]
fn a_report_without_identity_or_modules_says_so() {
    let report = format_report(&fields(None, None));
    assert!(report.contains("\nbuild_id: (none)\n"), "{report}");
    assert!(report.ends_with("my_game::main\n"), "{report}");
    assert!(!report.contains("modules:"), "{report}");
}

#[test]
fn the_target_is_the_build_triple() {
    let target = env!("TUNGSTEN_TARGET");
    assert!(target.starts_with(std::env::consts::ARCH), "{target}");
    assert!(target.contains(std::env::consts::OS), "{target}");
}

#[test]
fn the_test_panic_variable_counts_only_when_set_and_not_empty() {
    assert!(!test_panic_requested(None));
    assert!(!test_panic_requested(Some(OsStr::new(""))));
    assert!(test_panic_requested(Some(OsStr::new("1"))));
    assert!(test_panic_requested(Some(OsStr::new("0"))));
}

#[test]
#[should_panic(expected = "TUNGSTEN_TEST_PANIC")]
fn the_test_panic_system_panics_naming_the_variable() {
    test_panic_system(&mut World::new());
}

#[test]
fn report_files_take_a_counter_on_a_collision() {
    let dir = std::env::temp_dir().join(format!("tungsten_crash_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let now = SystemTime::UNIX_EPOCH;
    let (first, _) = create_report_file(&dir, "game", now).unwrap();
    let (second, _) = create_report_file(&dir, "game", now).unwrap();
    let pid = std::process::id();
    assert_eq!(
        first.file_name().unwrap().to_str(),
        Some(format!("game-19700101T000000.000Z-{pid}-crash.txt").as_str())
    );
    assert_eq!(
        second.file_name().unwrap().to_str(),
        Some(format!("game-19700101T000000.000Z-{pid}-2-crash.txt").as_str())
    );
    let _ = std::fs::remove_dir_all(&dir);
}
