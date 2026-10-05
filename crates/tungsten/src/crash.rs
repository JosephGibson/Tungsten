//! The crash report (`D-119`): a panic hook, installed once per process by
//! `App::new` when the game has a user folder, that writes
//! `<logs>/<stem>-<stamp>-<pid>-crash.txt` and then calls the hook it
//! replaced. `scripts/crash-report.py` symbolizes the file.
//!
//! The hook ignores every I/O error and never logs through `log`, since the
//! panicking thread may hold the logger's lock. Only panics leave a file:
//! the hook runs before `panic = "abort"` ends a release process.

use std::backtrace::Backtrace;
use std::ffi::OsStr;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::SystemTime;

use tungsten_core::{GameConfig, World};

use crate::logging::{self, LogFile};

/// Set and not empty, `App::new` registers [`test_panic_system`].
pub(crate) const TEST_PANIC_ENV: &str = "TUNGSTEN_TEST_PANIC";

/// Guards std's one hook slot, so a second `App` chains no second report:
/// the only process-wide state the engine adds (`D-119`).
static INSTALL: Once = Once::new();

const PT_NOTE: u32 = 4;
const NT_GNU_BUILD_ID: u32 = 3;
const IMAGE_DEBUG_TYPE_CODEVIEW: u32 = 2;
const DEBUG_DIRECTORY: usize = 6;
/// Bounds what a malformed header can make the readers allocate.
const MAX_RECORD: usize = 64 * 1024;
/// Crash files one process may write in one millisecond.
const MAX_SUFFIX: u32 = 100;
/// What a field the report cannot fill reads as.
const NONE: &str = "(none)";

/// What a report says that is known when the hook is installed.
pub(crate) struct CrashContext {
    game: Option<String>,
    game_version: Option<String>,
    executable: String,
    build_id: Option<BuildId>,
    logs: PathBuf,
    stem: String,
    log_file: LogFile,
}

impl CrashContext {
    /// Reads the executable's path and identity now, once.
    pub(crate) fn new(game: &GameConfig, logs: PathBuf, log_file: LogFile) -> Self {
        let exe = std::env::current_exe().ok();
        Self {
            game: game.id.clone(),
            game_version: game.version.clone(),
            executable: exe
                .as_ref()
                .map_or_else(|| NONE.to_string(), |exe| exe.display().to_string()),
            build_id: exe
                .and_then(|exe| File::open(exe).ok())
                .and_then(|mut file| read_build_id(&mut file)),
            logs,
            stem: logging::exe_stem(),
            log_file,
        }
    }
}

/// Installs the hook, once per process: `take_hook`, then `set_hook` with
/// a hook that writes the report and then calls the one it took.
pub(crate) fn install(context: CrashContext) {
    INSTALL.call_once(move || {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            write_report(&context, info);
            previous(info);
        }));
    });
}

fn write_report(context: &CrashContext, info: &PanicHookInfo<'_>) {
    let now = SystemTime::now();
    let thread = std::thread::current();
    let location = info.location().map(ToString::to_string);
    let backtrace = format!("{:#}", Backtrace::force_capture());
    let modules = modules();
    let report = format_report(&ReportFields {
        game: context.game.as_deref(),
        game_version: context.game_version.as_deref(),
        executable: &context.executable,
        time: &logging::utc_stamp(now),
        thread: thread.name().unwrap_or("<unnamed>"),
        message: info.payload_as_str().unwrap_or("Box<dyn Any>"),
        location: location.as_deref(),
        anchor: anchor_address(),
        build_id: context.build_id.as_ref(),
        backtrace: &backtrace,
        modules: modules.as_deref(),
    });
    let Some((path, mut file)) = create_report_file(&context.logs, &context.stem, now) else {
        return;
    };
    let _ = file.write_all(report.as_bytes());
    if let Some(mut log) = context.log_file.get() {
        let _ = writeln!(
            log,
            "[{} ERROR tungsten::crash] Crash report written to '{}'",
            logging::utc_stamp(now),
            path.display()
        );
    }
}

/// `<stem>-<stamp>-<pid>-crash.txt`, created new; on a collision, with a
/// counter before `-crash.txt`.
fn create_report_file(logs: &Path, stem: &str, now: SystemTime) -> Option<(PathBuf, File)> {
    let _ = std::fs::create_dir_all(logs);
    let base = format!("{stem}-{}-{}", logging::utc_stamp(now), std::process::id());
    for n in 1..=MAX_SUFFIX {
        let name = if n == 1 {
            format!("{base}-crash.txt")
        } else {
            format!("{base}-{n}-crash.txt")
        };
        let path = logs.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Some((path, file)),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            Err(_) => return None,
        }
    }
    None
}

/// A report's fields, laid out by [`format_report`].
pub(crate) struct ReportFields<'a> {
    pub(crate) game: Option<&'a str>,
    pub(crate) game_version: Option<&'a str>,
    pub(crate) executable: &'a str,
    pub(crate) time: &'a str,
    pub(crate) thread: &'a str,
    pub(crate) message: &'a str,
    pub(crate) location: Option<&'a str>,
    pub(crate) anchor: usize,
    pub(crate) build_id: Option<&'a BuildId>,
    pub(crate) backtrace: &'a str,
    pub(crate) modules: Option<&'a str>,
}

/// One `key: value` line per field, then `backtrace:` and, where known,
/// `modules:`, each followed by its lines. A field that is not known reads
/// `(none)`; line breaks in the message are escaped.
pub(crate) fn format_report(fields: &ReportFields<'_>) -> String {
    let build_id = fields
        .build_id
        .map_or_else(|| NONE.to_string(), ToString::to_string);
    let mut report = format!(
        "Tungsten crash report\n\
         game: {}\n\
         game_version: {}\n\
         engine_version: {}\n\
         target: {}\n\
         executable: {}\n\
         time: {}\n\
         thread: {}\n\
         message: {}\n\
         location: {}\n\
         anchor: {:#x} tungsten::crash::anchor\n\
         build_id: {build_id}\n\
         backtrace:\n",
        fields.game.unwrap_or(NONE),
        fields.game_version.unwrap_or(NONE),
        env!("CARGO_PKG_VERSION"),
        env!("TUNGSTEN_TARGET"),
        fields.executable,
        fields.time,
        fields.thread,
        fields.message.replace('\r', "\\r").replace('\n', "\\n"),
        fields.location.unwrap_or(NONE),
        fields.anchor,
    );
    push_lines(&mut report, fields.backtrace);
    if let Some(modules) = fields.modules {
        report.push_str("modules:\n");
        push_lines(&mut report, modules);
    }
    report
}

fn push_lines(report: &mut String, text: &str) {
    report.push_str(text);
    if !text.ends_with('\n') {
        report.push('\n');
    }
}

/// Printed in every report: the anchor's runtime address less its address
/// in the binary is the module's base, which a Windows backtrace does not
/// print. The constant keeps it from being folded into another function.
#[inline(never)]
fn anchor() -> u64 {
    0x7475_6e67_7374_656e
}

fn anchor_address() -> usize {
    anchor as fn() -> u64 as usize
}

/// The file-backed lines of `/proc/self/maps`: each module's mappings,
/// from which the symbolizer takes its base.
#[cfg(target_os = "linux")]
fn modules() -> Option<String> {
    let maps = std::fs::read_to_string("/proc/self/maps").ok()?;
    Some(
        maps.lines()
            .filter(|line| {
                line.split_whitespace()
                    .nth(5)
                    .is_some_and(|path| path.starts_with('/'))
            })
            .flat_map(|line| [line, "\n"])
            .collect(),
    )
}

#[cfg(not(target_os = "linux"))]
fn modules() -> Option<String> {
    None
}

/// `TUNGSTEN_TEST_PANIC` set and not empty asks for a first-frame panic.
pub(crate) fn test_panic_requested(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| !value.is_empty())
}

/// Registered under `TUNGSTEN_TEST_PANIC` as `__test_panic`, to check a
/// shipped build's crash report.
pub(crate) fn test_panic_system(_world: &mut World) {
    panic!("TUNGSTEN_TEST_PANIC is set: panicking in the first frame");
}

/// The running executable's identity, which the symbolizer checks a debug
/// file against before using it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BuildId {
    /// The ELF `NT_GNU_BUILD_ID` note, printed as lowercase hex.
    Gnu(Vec<u8>),
    /// The PE CodeView record: printed as the PDB's GUID and age in symbol
    /// server form, then the PDB path it names.
    Pdb {
        guid: [u8; 16],
        age: u32,
        name: String,
    },
}

impl fmt::Display for BuildId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gnu(bytes) => bytes.iter().try_for_each(|byte| write!(f, "{byte:02x}")),
            Self::Pdb { guid, age, name } => {
                let data1 = u32::from_le_bytes([guid[0], guid[1], guid[2], guid[3]]);
                let data2 = u16::from_le_bytes([guid[4], guid[5]]);
                let data3 = u16::from_le_bytes([guid[6], guid[7]]);
                write!(f, "{data1:08X}{data2:04X}{data3:04X}")?;
                guid[8..]
                    .iter()
                    .try_for_each(|byte| write!(f, "{byte:02X}"))?;
                write!(f, "{age:X} {name}")
            }
        }
    }
}

/// Reads the identity from an ELF or PE file's headers, with plain reads;
/// `None` for any other file, or one without it.
pub(crate) fn read_build_id(file: &mut (impl Read + Seek)) -> Option<BuildId> {
    let mut magic = [0u8; 4];
    read_at(file, 0, &mut magic)?;
    if magic == *b"\x7fELF" {
        elf_build_id(file).map(BuildId::Gnu)
    } else if magic[..2] == *b"MZ" {
        pe_build_id(file)
    } else {
        None
    }
}

/// The GNU build-id note from a 64-bit little-endian ELF's program headers.
fn elf_build_id(file: &mut (impl Read + Seek)) -> Option<Vec<u8>> {
    let mut header = [0u8; 64];
    read_at(file, 0, &mut header)?;
    if header[4] != 2 || header[5] != 1 {
        return None;
    }
    let phoff = u64_at(&header, 0x20)?;
    let phentsize = u64::from(u16_at(&header, 0x36)?);
    let phnum = u64::from(u16_at(&header, 0x38)?);
    if phentsize < 56 {
        return None;
    }
    for index in 0..phnum {
        let mut program = [0u8; 56];
        read_at(file, phoff.checked_add(index * phentsize)?, &mut program)?;
        if u32_at(&program, 0)? != PT_NOTE {
            continue;
        }
        let size = usize::try_from(u64_at(&program, 32)?).ok()?;
        if size > MAX_RECORD {
            continue;
        }
        let align = if u64_at(&program, 48)? == 8 { 8 } else { 4 };
        let mut notes = vec![0u8; size];
        read_at(file, u64_at(&program, 8)?, &mut notes)?;
        if let Some(id) = gnu_build_id(&notes, align) {
            return Some(id);
        }
    }
    None
}

/// Walks one note segment for the `GNU` note of type `NT_GNU_BUILD_ID`.
fn gnu_build_id(notes: &[u8], align: usize) -> Option<Vec<u8>> {
    let mut at = 0usize;
    while at.checked_add(12)? <= notes.len() {
        let name_size = u32_at(notes, at)? as usize;
        let desc_size = u32_at(notes, at + 4)? as usize;
        let kind = u32_at(notes, at + 8)?;
        let name_start = at + 12;
        let desc_start = name_start.checked_add(name_size.next_multiple_of(align))?;
        let name = notes.get(name_start..name_start.checked_add(name_size)?)?;
        let desc = notes.get(desc_start..desc_start.checked_add(desc_size)?)?;
        if kind == NT_GNU_BUILD_ID && name == b"GNU\0" {
            return Some(desc.to_vec());
        }
        at = desc_start.checked_add(desc_size.next_multiple_of(align))?;
    }
    None
}

/// The CodeView (`RSDS`) record that a PE file's debug directory names.
fn pe_build_id(file: &mut (impl Read + Seek)) -> Option<BuildId> {
    let mut dos = [0u8; 64];
    read_at(file, 0, &mut dos)?;
    let pe = u64::from(u32_at(&dos, 0x3c)?);
    let mut coff = [0u8; 24];
    read_at(file, pe, &mut coff)?;
    if coff[..4] != *b"PE\0\0" {
        return None;
    }
    let section_count = usize::from(u16_at(&coff, 6)?);
    let optional_size = u16_at(&coff, 20)?;
    let mut optional = vec![0u8; usize::from(optional_size)];
    read_at(file, pe + 24, &mut optional)?;
    let directories = match u16_at(&optional, 0)? {
        0x10b => 96,
        0x20b => 112,
        _ => return None,
    };
    if (u32_at(&optional, directories - 4)? as usize) <= DEBUG_DIRECTORY {
        return None;
    }
    let debug_rva = u32_at(&optional, directories + 8 * DEBUG_DIRECTORY)?;
    let debug_size = u32_at(&optional, directories + 8 * DEBUG_DIRECTORY + 4)?;
    let mut sections = vec![0u8; 40 * section_count];
    read_at(file, pe + 24 + u64::from(optional_size), &mut sections)?;
    let debug_offset = u64::from(rva_to_offset(&sections, debug_rva)?);
    for index in 0..u64::from(debug_size / 28) {
        let mut entry = [0u8; 28];
        read_at(file, debug_offset + index * 28, &mut entry)?;
        if u32_at(&entry, 12)? != IMAGE_DEBUG_TYPE_CODEVIEW {
            continue;
        }
        let size = u32_at(&entry, 16)? as usize;
        if !(24..=MAX_RECORD).contains(&size) {
            continue;
        }
        let mut record = vec![0u8; size];
        read_at(file, u64::from(u32_at(&entry, 24)?), &mut record)?;
        if record[..4] != *b"RSDS" {
            continue;
        }
        let name = &record[24..];
        let end = name
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(name.len());
        return Some(BuildId::Pdb {
            guid: record[4..20].try_into().ok()?,
            age: u32_at(&record, 20)?,
            name: String::from_utf8_lossy(&name[..end]).into_owned(),
        });
    }
    None
}

/// The file offset of `rva`, from the section that holds it.
fn rva_to_offset(sections: &[u8], rva: u32) -> Option<u32> {
    sections.as_chunks::<40>().0.iter().find_map(|section| {
        let virtual_size = u32_at(section, 8)?;
        let virtual_address = u32_at(section, 12)?;
        let raw_size = u32_at(section, 16)?;
        let raw_offset = u32_at(section, 20)?;
        let within = rva.checked_sub(virtual_address)?;
        if within < virtual_size.max(raw_size) {
            raw_offset.checked_add(within)
        } else {
            None
        }
    })
}

fn read_at(file: &mut (impl Read + Seek), offset: u64, buf: &mut [u8]) -> Option<()> {
    file.seek(SeekFrom::Start(offset)).ok()?;
    file.read_exact(buf).ok()
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

#[cfg(test)]
#[path = "tests/crash.rs"]
mod tests;
