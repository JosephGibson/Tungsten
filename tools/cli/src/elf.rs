//! The ELF reads `package`'s debug split checks (`D-120`), ported from
//! `scripts/release.py`: the section headers of a 64-bit little-endian
//! file, its GNU build-id note and whether it holds line tables.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

const SHT_NOTE: u32 = 7;
const SHT_NOBITS: u32 = 8;
const NT_GNU_BUILD_ID: u32 = 3;

/// One section: its type, offset, size and alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Section {
    pub(crate) kind: u32,
    pub(crate) offset: usize,
    pub(crate) size: usize,
    pub(crate) align: u64,
}

/// Why a binary and its debug file do not make a split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SplitError {
    /// Not an ELF file this reads, or its headers run past the file.
    Malformed(String),
    /// The binary carries no GNU build-id.
    NoBuildId,
    /// The debug file's build-id differs from the binary's.
    Mismatch { binary: String, debug: String },
    /// The debug file holds no `.debug_line`: the build had no line tables.
    NoLineTables,
}

impl fmt::Display for SplitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(why) => write!(f, "{why}"),
            Self::NoBuildId => f.write_str("the binary has no GNU build-id"),
            Self::Mismatch { binary, debug } => write!(
                f,
                "the binary's build-id {binary} differs from its debug file's ({debug})"
            ),
            Self::NoLineTables => f.write_str("the debug file holds no .debug_line section"),
        }
    }
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(data: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(data.get(at..at + 8)?.try_into().ok()?))
}

/// The sections by name, from a 64-bit little-endian ELF's section headers.
pub(crate) fn sections(data: &[u8]) -> Result<BTreeMap<String, Section>, SplitError> {
    if data.get(..4) != Some(b"\x7fELF") || data.get(4..6) != Some(&[2, 1]) {
        return Err(SplitError::Malformed(
            "not a 64-bit little-endian ELF file".to_owned(),
        ));
    }
    let malformed = || SplitError::Malformed("malformed ELF section headers".to_owned());
    let shoff =
        usize::try_from(u64_at(data, 0x28).ok_or_else(malformed)?).map_err(|_| malformed())?;
    let entsize = usize::from(u16_at(data, 0x3A).ok_or_else(malformed)?);
    let count = usize::from(u16_at(data, 0x3C).ok_or_else(malformed)?);
    let names_index = usize::from(u16_at(data, 0x3E).ok_or_else(malformed)?);
    let header = |i: usize| -> Option<(u32, u32, usize, usize, u64)> {
        let at = shoff.checked_add(i.checked_mul(entsize)?)?;
        Some((
            u32_at(data, at)?,
            u32_at(data, at + 4)?,
            usize::try_from(u64_at(data, at + 0x18)?).ok()?,
            usize::try_from(u64_at(data, at + 0x20)?).ok()?,
            u64_at(data, at + 0x30)?,
        ))
    };
    let names = header(names_index).ok_or_else(malformed)?.2;
    let mut out = BTreeMap::new();
    for i in 0..count {
        let (name, kind, offset, size, align) = header(i).ok_or_else(malformed)?;
        let start = names.checked_add(name as usize).ok_or_else(malformed)?;
        let rest = data.get(start..).ok_or_else(malformed)?;
        let end = rest.iter().position(|&b| b == 0).ok_or_else(malformed)?;
        let label = String::from_utf8_lossy(&rest[..end]).into_owned();
        out.insert(
            label,
            Section {
                kind,
                offset,
                size,
                align,
            },
        );
    }
    Ok(out)
}

/// The GNU build-id from the note sections, if any.
pub(crate) fn build_id(data: &[u8]) -> Result<Option<Vec<u8>>, SplitError> {
    for section in sections(data)?.values() {
        if section.kind != SHT_NOTE {
            continue;
        }
        let Some(notes) = data.get(section.offset..section.offset.saturating_add(section.size))
        else {
            continue;
        };
        let step = if section.align == 8 { 8 } else { 4 };
        let round = |n: usize| n.div_ceil(step) * step;
        let mut at = 0;
        while at + 12 <= notes.len() {
            let (Some(name_size), Some(desc_size), Some(kind)) = (
                u32_at(notes, at),
                u32_at(notes, at + 4),
                u32_at(notes, at + 8),
            ) else {
                break;
            };
            let (name_size, desc_size) = (name_size as usize, desc_size as usize);
            let desc = at + 12 + round(name_size);
            if kind == NT_GNU_BUILD_ID && notes.get(at + 12..at + 12 + name_size) == Some(b"GNU\0")
            {
                return Ok(notes.get(desc..desc + desc_size).map(<[u8]>::to_vec));
            }
            at = desc + round(desc_size);
        }
    }
    Ok(None)
}

/// Whether section `name` exists with contents.
pub(crate) fn has_section(data: &[u8], name: &str) -> Result<bool, SplitError> {
    Ok(sections(data)?
        .get(name)
        .is_some_and(|section| section.kind != SHT_NOBITS && section.size > 0))
}

/// `debug` belongs to `binary` (equal GNU build-ids) and holds line tables.
pub(crate) fn check_split(binary: &[u8], debug: &[u8]) -> Result<(), SplitError> {
    let Some(binary_id) = build_id(binary)? else {
        return Err(SplitError::NoBuildId);
    };
    let debug_id = build_id(debug)?;
    if debug_id.as_ref() != Some(&binary_id) {
        return Err(SplitError::Mismatch {
            binary: hex(&binary_id),
            debug: debug_id.map_or_else(|| "none".to_owned(), |id| hex(&id)),
        });
    }
    if !has_section(debug, ".debug_line")? {
        return Err(SplitError::NoLineTables);
    }
    Ok(())
}

/// [`check_split`] on two files.
pub(crate) fn check_split_files(binary: &Path, debug: &Path) -> Result<(), SplitError> {
    let read = |path: &Path| {
        std::fs::read(path)
            .map_err(|e| SplitError::Malformed(format!("cannot read '{}': {e}", path.display())))
    };
    check_split(&read(binary)?, &read(debug)?)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

#[cfg(test)]
#[path = "tests/elf.rs"]
mod tests;
