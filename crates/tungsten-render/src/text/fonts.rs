//! Font sources, the fallback chain as cosmic-text's `Fallback`, family-name
//! interning and face selection inside a family (`D-116`).

use std::collections::HashSet;

use glyphon::cosmic_text::{Fallback, PlatformFallback};
use unicode_script::Script;

/// Where the text engine's faces come from (`D-116`).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FontSource {
    /// Only the faces the manifest registers.
    #[default]
    Packaged,
    /// The manifest's faces, then the system's: `render.system_fonts`. A
    /// packaged face replaces a system face of the same family, weight and
    /// style, and the platform's fallback lists follow the chain.
    PackagedThenSystem,
}

/// cosmic-text's fallback: the chain's fontdb family names, in order, then,
/// for [`FontSource::PackagedThenSystem`], the platform's lists.
pub(super) struct ChainFallback {
    common: Vec<&'static str>,
    system: bool,
    platform: PlatformFallback,
}

impl ChainFallback {
    pub(super) fn new(chain: &[&'static str], source: FontSource) -> Self {
        let system = source == FontSource::PackagedThenSystem;
        let platform = PlatformFallback;
        let mut common = chain.to_vec();
        if system {
            for &name in platform.common_fallback() {
                if !common.contains(&name) {
                    common.push(name);
                }
            }
        }
        Self {
            common,
            system,
            platform,
        }
    }
}

impl Fallback for ChainFallback {
    fn common_fallback(&self) -> &[&'static str] {
        &self.common
    }

    fn forbidden_fallback(&self) -> &[&'static str] {
        if self.system {
            self.platform.forbidden_fallback()
        } else {
            &[]
        }
    }

    fn script_fallback(&self, script: Script, locale: &str) -> &[&'static str] {
        if self.system {
            self.platform.script_fallback(script, locale)
        } else {
            &[]
        }
    }
}

/// Family names as `&'static str`, which cosmic-text's `Fallback` returns.
/// Each distinct name leaks once; the set is bounded by the faces ever
/// registered.
#[derive(Default)]
pub(super) struct FamilyNames(HashSet<&'static str>);

impl FamilyNames {
    pub(super) fn intern(&mut self, name: &str) -> &'static str {
        if let Some(&interned) = self.0.get(name) {
            return interned;
        }
        let interned: &'static str = Box::leak(name.to_owned().into_boxed_str());
        self.0.insert(interned);
        interned
    }
}

/// A face a family offers: its weight and whether it is italic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FaceStyle {
    pub(super) weight: u16,
    pub(super) italic: bool,
}

/// The face of `faces` a style asks for: the exact weight, else the nearest,
/// ties to the heavier; italic exact, else the other style. Returns its index
/// and whether the style had to change, or `None` for an empty family.
pub(super) fn pick_face(faces: &[FaceStyle], weight: u16, italic: bool) -> Option<(usize, bool)> {
    let styled = faces.iter().any(|face| face.italic == italic);
    faces
        .iter()
        .enumerate()
        .filter(|(_, face)| !styled || face.italic == italic)
        .min_by_key(|(_, face)| (face.weight.abs_diff(weight), std::cmp::Reverse(face.weight)))
        .map(|(index, _)| (index, !styled))
}
