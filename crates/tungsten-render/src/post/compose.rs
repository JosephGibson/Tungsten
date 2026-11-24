//! The post composer (`D-307`): which stages of a post stack draw in one
//! pass, and that pass's generated module.
//!
//! A post effect's file marks one section, between `// @fx begin <class>`
//! and `// @fx end`, holding one function and its helpers. The file's own
//! `fs_main` calls that function, so a stage drawn alone and a stage drawn
//! in a fused pass run the same arithmetic. This module reads sections,
//! holds the rule that says which stages fuse, and writes the fused pass's
//! WGSL. It builds no GPU object: the post stack's renderer owns the
//! pipelines and their cache.

use std::fmt::Write as _;
use std::ops::Range;

use tungsten_core::post::PostPass;
use wgpu::naga;

/// What a section's function does, which decides where its stage may sit in
/// a fused pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Class {
    /// Samples the source itself, at its own pixel or at others: it can only
    /// lead a pass.
    Head,
    /// Reads its own pixel alone, each colour channel its own input channel
    /// times a factor from 0 to 1 plus a constant from 0 up; the factor and
    /// the constant may follow the pixel's place and the parameters, never
    /// the colour or the alpha. It returns the alpha it is handed.
    Mix,
    /// Reads its own pixel alone and is continuous in it.
    Pixel,
    /// Reads its own pixel and quantises or branches on it: it can only lead
    /// a pass, so it reads stored pixels as it does alone.
    Steps,
}

impl Class {
    fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "head" => Self::Head,
            "mix" => Self::Mix,
            "pixel" => Self::Pixel,
            "steps" => Self::Steps,
            _ => return None,
        })
    }

    /// Whether the section's function takes the colour it is handed
    /// (`<name>_fx`) and may follow another stage, where a leading class
    /// samples the source itself (`<name>_head`).
    pub(crate) fn follows(self) -> bool {
        matches!(self, Self::Mix | Self::Pixel)
    }
}

/// The marker lines of a section.
const BEGIN: &str = "// @fx begin ";
const END: &str = "// @fx end";

/// The binding every post module reads its parameters through, which a
/// fused module replaces with an array of its stages' blocks.
const PARAMS_BINDING: &str = "@group(1) @binding(0) var<uniform> params: Params;";

/// One stock file, for the prefix every post module shares.
const PREFIX_SOURCE: &str = include_str!("../shaders/stock/fade.wgsl");

/// At most this many stages in one fused pass.
pub(crate) const MAX_STAGES: usize = 8;

/// The text every post module starts with after its header comment:
/// `Params`, the source texture and its sampler, the parameter block, the
/// vertex output and `vs_main`. A test holds every stock file to these
/// bytes (`every_engine_file_holds_a_valid_section_of_its_class`).
pub(crate) fn prefix() -> &'static str {
    let start = PREFIX_SOURCE.find("struct Params").unwrap_or(0);
    let end = PREFIX_SOURCE.find(BEGIN).unwrap_or(PREFIX_SOURCE.len());
    &PREFIX_SOURCE[start..end]
}

/// The prefix's `Params` struct alone: all a section that follows another
/// stage may name beyond its own declarations.
fn params_struct() -> &'static str {
    let prefix = prefix();
    &prefix[..prefix.find("@group(0)").unwrap_or(prefix.len())]
}

/// An effect's marked section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Section {
    /// The effect's name, which every name the section declares starts with.
    pub name: String,
    pub class: Class,
    /// The section's text, without its marker lines.
    pub text: String,
}

/// The class and the text between the markers of `wgsl`, unchecked: for the
/// engine's own files, which the tests check.
pub(crate) fn section_text(wgsl: &str) -> Result<(Class, &str), String> {
    let mut begins = wgsl.match_indices(BEGIN);
    let (Some((begin, _)), None) = (begins.next(), begins.next()) else {
        return Err(format!("expected one `{}<class>` line", BEGIN.trim_end()));
    };
    let line_end = wgsl[begin..]
        .find('\n')
        .map_or(wgsl.len(), |offset| begin + offset);
    let word = wgsl[begin + BEGIN.len()..line_end].trim();
    let class = Class::parse(word)
        .ok_or_else(|| format!("unknown class `{word}`: expected head, mix, pixel or steps"))?;
    let body = line_end.saturating_add(1).min(wgsl.len());
    let mut ends = wgsl.match_indices(END);
    let (Some((end, _)), None) = (ends.next(), ends.next()) else {
        return Err(format!("expected one `{END}` line"));
    };
    if end < body {
        return Err(format!("`{END}` comes before the section's start"));
    }
    Ok((class, &wgsl[body..end]))
}

/// Reads and checks the section of effect `name` in `wgsl`:
///
/// - the markers, once each, with a known class;
/// - the text is valid WGSL beside what it may name: the whole prefix for a
///   class that leads a pass, `Params` alone for one that follows another
///   stage, so such a section names no module-scope variable (no `src`, no
///   `params`) and calls nothing outside itself: a fused stage cannot sample
///   around the colour it is handed;
/// - every function, constant, variable and struct it declares is named
///   `<name>_…`, so two effects' sections never collide in one module;
/// - its function is `<name>_fx(vec4<f32>, vec2<f32>, Params) -> vec4<f32>`
///   or, for a leading class, `<name>_head(vec2<f32>, Params) -> vec4<f32>`.
///
/// - a `mix` hands on the alpha it is given ([`mix_alpha_fault`]).
///
/// The engine's own files are held to this by the tests, and read at run
/// time by [`section_text`] alone; a game's effect is read through here when
/// its shader loads (`D-308`).
///
/// # Errors
///
/// What the section breaks, as one line or Naga's report.
pub(crate) fn read_section(name: &str, wgsl: &str) -> Result<Section, String> {
    let (class, text) = section_text(wgsl)?;
    let code = uncommented(text);
    for entry in ["@fragment", "@vertex", "@compute"] {
        if code.contains(entry) {
            return Err(format!(
                "a section holds no entry point: it declares `{entry}`"
            ));
        }
    }
    // What a generated pass gives a section: `Params` alone for one that
    // follows another stage, and for one that leads the prefix without its
    // own block, which a generated module replaces with a block a stage.
    let scope = if class.follows() {
        params_struct().to_string()
    } else {
        prefix().replace(PARAMS_BINDING, "")
    };
    let module_text = format!("{scope}\n{text}");
    let module = naga::front::wgsl::parse_str(&module_text)
        .map_err(|error| error.emit_to_string(&module_text))?;
    crate::shader_hot_reload::validate_wgsl_source(name, &module_text)
        .map_err(|error| error.to_string())?;

    // A constant is often upper case: names compare without their case.
    let prefix = format!("{}_", name.to_ascii_lowercase());
    let own = |declared: Option<&str>| {
        declared.is_some_and(|declared| declared.to_ascii_lowercase().starts_with(&prefix))
    };
    let outside = |kind: &str, declared: Option<&str>| {
        format!(
            "{kind} `{}` is not named `{name}_…`",
            declared.unwrap_or("<unnamed>")
        )
    };
    for (_, function) in module.functions.iter() {
        if !own(function.name.as_deref()) {
            return Err(outside("function", function.name.as_deref()));
        }
    }
    for (_, constant) in module.constants.iter() {
        if constant.name.is_some() && !own(constant.name.as_deref()) {
            return Err(outside("constant", constant.name.as_deref()));
        }
    }
    for (_, variable) in module.global_variables.iter() {
        let declared = variable.name.as_deref();
        // A section that follows another stage is read beside `Params`
        // alone, so every variable here is its own.
        if class.follows() {
            return Err(format!(
                "a `{}` section declares no module-scope variable",
                if class == Class::Mix { "mix" } else { "pixel" }
            ));
        }
        // The prefix's own, which a leading section may name.
        if matches!(declared, Some("src" | "src_sampler")) && !code.contains("var src") {
            continue;
        }
        if !own(declared) {
            return Err(outside("variable", declared));
        }
    }
    for (_, ty) in module.types.iter() {
        let is_struct = matches!(ty.inner, naga::TypeInner::Struct { .. });
        let declared = ty.name.as_deref();
        if is_struct && !matches!(declared, Some("Params" | "VertexOutput")) && !own(declared) {
            return Err(outside("struct", declared));
        }
    }

    let entry = if class.follows() {
        format!("{name}_fx")
    } else {
        format!("{name}_head")
    };
    let Some((_, function)) = module
        .functions
        .iter()
        .find(|(_, function)| function.name.as_deref() == Some(entry.as_str()))
    else {
        return Err(format!("no function `{entry}`"));
    };
    let vector = |handle: naga::Handle<naga::Type>, size: naga::VectorSize| {
        matches!(
            module.types[handle].inner,
            naga::TypeInner::Vector { size: held, scalar } if held == size && scalar == naga::Scalar::F32
        )
    };
    let params =
        |handle: naga::Handle<naga::Type>| module.types[handle].name.as_deref() == Some("Params");
    let arguments: Vec<naga::Handle<naga::Type>> = function
        .arguments
        .iter()
        .map(|argument| argument.ty)
        .collect();
    let shaped = if class.follows() {
        matches!(arguments[..], [color, uv, block]
            if vector(color, naga::VectorSize::Quad) && vector(uv, naga::VectorSize::Bi) && params(block))
    } else {
        matches!(arguments[..], [uv, block] if vector(uv, naga::VectorSize::Bi) && params(block))
    };
    let returns = function
        .result
        .as_ref()
        .is_some_and(|result| vector(result.ty, naga::VectorSize::Quad));
    if !shaped || !returns {
        return Err(if class.follows() {
            format!("`{entry}` is not `fn(vec4<f32>, vec2<f32>, Params) -> vec4<f32>`")
        } else {
            format!("`{entry}` is not `fn(vec2<f32>, Params) -> vec4<f32>`")
        });
    }
    if class == Class::Mix
        && let Some(fault) = mix_alpha_fault(function, &function_text(&code, &entry))
    {
        return Err(fault);
    }
    Ok(Section {
        name: name.to_string(),
        class,
        text: text.to_string(),
    })
}

/// The values `block` returns, through every block nested in it.
fn returned(block: &naga::Block, values: &mut Vec<Option<naga::Handle<naga::Expression>>>) {
    for statement in block {
        match statement {
            naga::Statement::Return { value } => values.push(*value),
            naga::Statement::Block(inner) => returned(inner, values),
            naga::Statement::If { accept, reject, .. } => {
                returned(accept, values);
                returned(reject, values);
            }
            naga::Statement::Switch { cases, .. } => {
                for case in cases {
                    returned(&case.body, values);
                }
            }
            naga::Statement::Loop {
                body, continuing, ..
            } => {
                returned(body, values);
                returned(continuing, values);
            }
            _ => {}
        }
    }
}

/// `text` without its comments, line and block, a block's nesting counted:
/// what the reader's textual checks scan, so that no comment decoys them.
fn uncommented(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut code = String::with_capacity(text.len());
    let (mut at, mut depth) = (0, 0usize);
    while at < bytes.len() {
        let pair = &bytes[at..(at + 2).min(bytes.len())];
        if pair == b"/*" {
            depth += 1;
            at += 2;
        } else if depth > 0 && pair == b"*/" {
            depth -= 1;
            at += 2;
        } else if depth > 0 {
            at += 1;
        } else if pair == b"//" {
            while at < bytes.len() && bytes[at] != b'\n' {
                at += 1;
            }
        } else {
            // A comment starts and ends on ASCII, so this is a char's start.
            let ch = text[at..].chars().next().unwrap_or(' ');
            code.push(ch);
            at += ch.len_utf8();
        }
    }
    code
}

/// The source of function `entry` in `code`, a section's text without its
/// comments, from its name to its closing brace; empty when it is not found.
fn function_text(code: &str, entry: &str) -> String {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    // `fn`, then the name as a whole word, then its parameters, however
    // they are spaced.
    let start = code.match_indices(entry).map(|(at, _)| at).find(|&at| {
        let before = code[..at].trim_end();
        let after = code[at + entry.len()..].trim_start();
        code[..at].ends_with(char::is_whitespace)
            && before.ends_with("fn")
            && !before[..before.len() - 2]
                .chars()
                .next_back()
                .is_some_and(word)
            && after.starts_with('(')
    });
    let Some(start) = start else {
        return String::new();
    };
    let body = &code[start..];
    let Some(open) = body.find('{') else {
        return String::new();
    };
    let mut depth = 0usize;
    for (at, character) in body[open..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return body[..=open + at].to_string();
                }
            }
            _ => {}
        }
    }
    String::new()
}

/// Why a `mix` function does not hand on the alpha it is given, or reads
/// it for something else; `None` when it does. A fused pass clamps only the
/// colour between its stages ([`Between::Clamp`]), which is two passes'
/// picture only while its last stage returns its input's alpha untouched
/// and computes no colour from it.
///
/// Every value `function` returns is its first argument itself or a vector
/// whose last component is that argument's alpha, read from the parsed
/// function, wherever the `return` stands. And its `source` names the
/// argument nowhere else but for its colour channels: once with its alpha
/// for each such vector, and whole once for each such return and once in
/// the signature. A helper handed the whole colour, or a factor computed
/// from the alpha, is refused.
fn mix_alpha_fault(function: &naga::Function, source: &str) -> Option<String> {
    let mut values = Vec::new();
    returned(&function.body, &mut values);
    let argument = |handle: naga::Handle<naga::Expression>| {
        matches!(
            function.expressions[handle],
            naga::Expression::FunctionArgument(0)
        )
    };
    let (mut whole, mut composed) = (0usize, 0usize);
    for value in values.into_iter().flatten() {
        match &function.expressions[value] {
            naga::Expression::FunctionArgument(0) => whole += 1,
            naga::Expression::Compose { components, .. }
                if components.last().is_some_and(|last| {
                    matches!(
                        function.expressions[*last],
                        naga::Expression::AccessIndex { base, index: 3 } if argument(base)
                    )
                }) =>
            {
                composed += 1;
            }
            _ => {
                return Some(
                    "a `mix` returns the colour it is handed or a vector that ends in that colour's alpha: a `return` gives another alpha"
                        .to_string(),
                );
            }
        }
    }
    let name = function
        .arguments
        .first()
        .and_then(|argument| argument.name.as_deref())
        .unwrap_or("color");
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let (mut alpha, mut bare) = (0usize, 0usize);
    let mut at = 0;
    while let Some(found) = source[at..].find(name) {
        let start = at + found;
        let end = start + name.len();
        at = end;
        // Another name that holds this one, or another value's field.
        if source[..start]
            .chars()
            .next_back()
            .is_some_and(|c| word(c) || c == '.')
            || source[end..].chars().next().is_some_and(word)
        {
            continue;
        }
        let Some(accessor) = source[end..].strip_prefix('.') else {
            bare += 1;
            continue;
        };
        let channels: String = accessor.chars().take_while(|c| word(*c)).collect();
        if channels == "a" || channels == "w" {
            // The alpha is named only where it is handed on: the last
            // component of a vector, at that vector's end. Bound to a name
            // first, it could reach the colour.
            let handed = source[..start].trim_end().ends_with(',')
                && source[end + 1 + channels.len()..]
                    .trim_start()
                    .starts_with(')');
            if !handed {
                return Some(format!(
                    "a `mix` names `{name}.{channels}` only as the last component of the vector it returns"
                ));
            }
            alpha += 1;
        } else if channels.is_empty() || !channels.chars().all(|c| "rgbxyz".contains(c)) {
            return Some(format!(
                "a `mix` reads `{name}.{channels}`: it reads the colour's channels and hands the alpha on"
            ));
        }
    }
    (alpha != composed || bare != whole + 1).then(|| {
        format!(
            "a `mix` names `{name}` whole, or its alpha, for more than handing it on: it reads the colour's channels alone"
        )
    })
}

/// A stage of a stack, as far as the fusing rule goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StageInfo {
    /// The class of the stage's section; `None` where the stage draws
    /// alone whatever its neighbours: its file is replaced or its section
    /// was refused.
    pub class: Option<Class>,
    /// Whether a `mix` stage's parameters keep its class's promise
    /// ([`bounded`]).
    pub bounded: bool,
}

/// Whether `pass`'s parameters keep a `mix` section's promise: the colour it
/// mixes towards is not negative (nor a NaN), so its constant is from 0 up.
pub(crate) fn bounded(pass: &PostPass) -> bool {
    let from_zero = |color: [f32; 4]| color[..3].iter().all(|channel| *channel >= 0.0);
    match pass {
        PostPass::Vignette(params) => from_zero(params.color),
        PostPass::Fade(params) => from_zero(params.color),
        PostPass::Fog(params) => from_zero(params.color),
        PostPass::Dissolve(params) => from_zero(params.edge_color),
        _ => true,
    }
}

/// The stages of a frame's stack that draw in one pass under 8-bit
/// targets: the stack's last two, when the last is a bounded `mix` and the
/// one before it has a section. The bound on a fused pass is one 8-bit step
/// a channel of the stack's output, so nothing may follow a fused pass that
/// could amplify that step, and the second stage must not grow the first's
/// lost rounding in the encoded space the target rounds in, which a `mix`
/// does not.
///
/// `None` as well when fusing is off (`render.post_fuse`), and when an SMAA
/// tail follows the stack (`smaa`): the bound is the stack's output's, and
/// SMAA's edge test turns a one-step difference at an edge into another
/// blend (`D-307`: 67 pixels of the bench's frame past one step, the
/// furthest 115).
pub(crate) fn fused_pair(stages: &[StageInfo], fuse: bool, smaa: bool) -> Option<Range<usize>> {
    let count = stages.len();
    let pair = fuse
        && !smaa
        && count >= 2
        && stages[count - 1].class == Some(Class::Mix)
        && stages[count - 1].bounded
        && stages[count - 2].class.is_some();
    pair.then(|| count - 2..count)
}

/// The stack's stages grouped into passes, in order: each range draws in one
/// pass, a range of one by its stage's own pipeline.
///
/// Under 8-bit targets one case fuses, [`fused_pair`]'s, which is what a
/// frame asks.
///
/// Under float targets a pass's rounding is the float format's, so every
/// stage from the stack's last `steps` stage on fuses with those after it:
/// a stage that leads a pass starts one, and a pass holds [`MAX_STAGES`] at
/// most. The stages before that `steps` stage draw alone, so a quantising
/// stage reads what it reads unfused; a stage without a class may quantise
/// (a replaced file's is anything), so it counts as one.
#[cfg(test)]
pub(crate) fn runs(stages: &[StageInfo], float: bool) -> Vec<Range<usize>> {
    let mut runs = Vec::new();
    runs_into(stages, float, &mut runs);
    runs
}

/// [`runs`] into `runs`, which a frame keeps for its allocation.
pub(crate) fn runs_into(stages: &[StageInfo], float: bool, runs: &mut Vec<Range<usize>>) {
    runs.clear();
    let count = stages.len();
    if !float {
        let pair = fused_pair(stages, true, false);
        let alone = pair.as_ref().map_or(count, |pair| pair.start);
        runs.extend((0..alone).map(|index| index..index + 1));
        runs.extend(pair);
        return;
    }
    let from = stages
        .iter()
        .rposition(|stage| matches!(stage.class, Some(Class::Steps) | None))
        .unwrap_or(0);
    runs.extend((0..from).map(|index| index..index + 1));
    let mut start = from;
    while start < count {
        let mut end = start + 1;
        if stages[start].class.is_some() {
            while end < count
                && end - start < MAX_STAGES
                && stages[end].class.is_some_and(Class::follows)
            {
                end += 1;
            }
        }
        runs.push(start..end);
        start = end;
    }
}

/// Whether the frame's tail rides the pass that draws run `index` of
/// `runs`, the float rule's over `stages` (`D-309`): the stack's last run,
/// every stage of it with a section. A stage without one, a replaced file's
/// or a game's module that marks none, draws with its own pipeline, which
/// ends in no tail: the tail then draws in a pass of its own.
pub(crate) fn tail_rides(stages: &[StageInfo], runs: &[Range<usize>], index: usize) -> bool {
    index + 1 == runs.len()
        && stages[runs[index].clone()]
            .iter()
            .all(|stage| stage.class.is_some())
}

/// What a fused pass does to the colour between two of its stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Between {
    /// Clamps the colour to 0..1, as the 8-bit target between the two passes
    /// did. The alpha is left as the first stage wrote it: a pass under
    /// 8-bit targets ends in a `mix`, which returns the alpha it is handed
    /// and reads it for nothing, so the output's own target clamps it as
    /// the two passes' targets would. A clamp of the alpha here also moved
    /// the colour's last bit on the reference machine's driver: bloom's
    /// composite with a vignette read blue one step lower in 1 to 4 pixels
    /// of a 2160p frame than the pass `D-302` wrote by hand, which clamped
    /// the colour alone.
    Clamp,
    /// Nothing: a float target holds what a stage wrote (`render.hdr`).
    Nothing,
}

/// What a pass does last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Finish {
    /// Nothing: the pass writes a version of the frame's colour.
    Plain,
    /// The frame's tail (`D-309`): the pass reads or computes a float colour
    /// and writes the frame's 8-bit output, so it clamps the colour to 0..1,
    /// which is the 8-bit scene's look, and dithers it in the values the
    /// output stores, against the banding a float gradient shows in 8 bits.
    /// `srgb` when the output's format encodes what a pass writes, so the
    /// tail dithers the encoded value.
    Tail { srgb: bool },
}

/// How much of an 8-bit step the tail's noise spans, end to end. Under one:
/// the noise reaches 0.28 of a step either way, so a colour within 0.22 of
/// a value 8 bits hold is stored as that value, whatever a half-float scene,
/// the tail's own encode and decode and the output's conversion round by,
/// which is under a fifth of a step together. A full step would dither a
/// gradient with no error left, and would move a tenth of such colours a
/// step on some pixels (`D-309`).
pub(crate) const TAIL_NOISE_SPAN: f32 = 0.6;

/// The tail's functions; `NOISE_SPAN` stands for [`TAIL_NOISE_SPAN`], which
/// [`generate`] writes in.
const TAIL: &str = "const TUNGSTEN_TAIL_BAYER: array<f32, 16> = array<f32, 16>(
    0.0, 8.0, 2.0, 10.0,
    12.0, 4.0, 14.0, 6.0,
    3.0, 11.0, 1.0, 9.0,
    15.0, 7.0, 13.0, 5.0,
);

fn tungsten_tail_noise(position: vec2<f32>) -> f32 {
    let cell = vec2<u32>(position) % vec2<u32>(4u);
    let level = (TUNGSTEN_TAIL_BAYER[cell.y * 4u + cell.x] + 0.5) / 16.0 - 0.5;
    return level * NOISE_SPAN / 255.0;
}
";

/// The tail over an output that stores what a pass writes.
const TAIL_PLAIN: &str = "fn tungsten_tail(color: vec4<f32>, position: vec2<f32>) -> vec4<f32> {
    let rgb = clamp(color.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let stored = rgb + vec3<f32>(tungsten_tail_noise(position));
    return vec4<f32>(clamp(stored, vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
}
";

/// The tail over an sRGB output: the noise is added to the value the output
/// stores, and the sum decoded for the output to encode again.
const TAIL_SRGB: &str = "fn tungsten_tail_encode(linear: vec3<f32>) -> vec3<f32> {
    let low = linear * 12.92;
    let high = 1.055 * pow(linear, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(high, low, linear <= vec3<f32>(0.0031308));
}

fn tungsten_tail_decode(stored: vec3<f32>) -> vec3<f32> {
    let low = stored / 12.92;
    let high = pow((stored + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(high, low, stored <= vec3<f32>(0.04045));
}

fn tungsten_tail(color: vec4<f32>, position: vec2<f32>) -> vec4<f32> {
    let rgb = clamp(color.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let stored = tungsten_tail_encode(rgb) + vec3<f32>(tungsten_tail_noise(position));
    return vec4<f32>(tungsten_tail_decode(clamp(stored, vec3<f32>(0.0), vec3<f32>(1.0))), color.a);
}
";

/// The module of a pass that draws `stages` in order: the shared prefix with
/// its parameter block replaced by one block a stage (256 bytes each, the
/// stages' own blocks in a row: `Params`' 96 bytes and 160 of padding spelt
/// out as a member, since a `@size` attribute is lost in Naga's GLSL, where
/// the second stage would read the first block's tail), each distinct
/// section once, and an
/// `fs_main` that calls the first stage on the source and each later stage
/// on the colour before it, then the tail where `finish` asks it. With no
/// stage the pass is the tail alone, over the source's own pixel.
///
/// # Panics
///
/// When a stage after the first does not follow another stage: the rule
/// ([`runs`]) never places one there.
pub(crate) fn generate(stages: &[&Section], between: Between, finish: Finish) -> String {
    let blocks = format!(
        "struct Stage {{\n    p: Params,\n    pad: array<vec4<f32>, 10>,\n}};\n\n@group(1) @binding(0) var<uniform> stages: array<Stage, {}>;",
        stages.len().max(1)
    );
    let mut module = prefix().replace(PARAMS_BINDING, &blocks);
    let mut written: Vec<&str> = Vec::new();
    for section in stages {
        if !written.contains(&section.name.as_str()) {
            written.push(&section.name);
            module.push_str(&section.text);
            module.push('\n');
        }
    }
    if let Finish::Tail { srgb } = finish {
        module.push_str(&TAIL.replace("NOISE_SPAN", &format!("{TAIL_NOISE_SPAN:?}")));
        module.push('\n');
        module.push_str(if srgb { TAIL_SRGB } else { TAIL_PLAIN });
        module.push('\n');
    }
    module.push_str("@fragment\nfn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {\n");
    if stages.is_empty() {
        module.push_str("    var color = textureSample(src, src_sampler, in.uv);\n");
    }
    for (index, section) in stages.iter().enumerate() {
        let name = &section.name;
        if index == 0 {
            // A write to a `String` cannot fail.
            let _ = if section.class.follows() {
                writeln!(
                    module,
                    "    var color = {name}_fx(textureSample(src, src_sampler, in.uv), in.uv, stages[0].p);"
                )
            } else {
                writeln!(module, "    var color = {name}_head(in.uv, stages[0].p);")
            };
            continue;
        }
        assert!(
            section.class.follows(),
            "`{name}` leads a pass and cannot follow another stage"
        );
        if between == Between::Clamp {
            module.push_str(
                "    color = vec4<f32>(clamp(color.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), color.a);\n",
            );
        }
        let _ = writeln!(
            module,
            "    color = {name}_fx(color, in.uv, stages[{index}].p);"
        );
    }
    if matches!(finish, Finish::Tail { .. }) {
        module.push_str("    color = tungsten_tail(color, in.position.xy);\n");
    }
    module.push_str("    return color;\n}\n");
    module
}

/// The timing label of a pass that draws the stages named `names`, the
/// first in post-stack slot `slot`: `post<slot>_` and the names joined by
/// `_`.
pub(crate) fn label(slot: usize, names: &[&str]) -> String {
    format!("post{slot}_{}", names.join("_"))
}

/// Fused passes by what each was generated from: `S` its signature, `T`
/// its pipeline, generic so the cache's rule is tested without a device. A
/// pass is built at its signature's first use. One whose build failed is
/// held as failed, so its stages draw alone and nothing is built a frame,
/// until a member's file reloads.
pub(crate) struct Fused<S, T> {
    passes: Vec<(S, Option<T>)>,
}

impl<S, T> Default for Fused<S, T> {
    fn default() -> Self {
        Self { passes: Vec::new() }
    }
}

impl<S: PartialEq, T> Fused<S, T> {
    /// The pass of `signature`, built by `build` at its first use; `None`
    /// when its build failed.
    pub(crate) fn get_or_build(
        &mut self,
        signature: S,
        build: impl FnOnce() -> Option<T>,
    ) -> Option<&T> {
        let held = self.passes.iter().position(|(held, _)| *held == signature);
        let index = held.unwrap_or_else(|| {
            self.passes.push((signature, build()));
            self.passes.len() - 1
        });
        self.passes[index].1.as_ref()
    }

    /// The pass of `signature`, when it is built.
    pub(crate) fn get(&self, signature: &S) -> Option<&T> {
        self.passes
            .iter()
            .find(|(held, _)| held == signature)
            .and_then(|(_, pass)| pass.as_ref())
    }

    /// Drops every pass, built or failed, whose signature `holds` says a
    /// reloaded file is part of.
    pub(crate) fn forget(&mut self, holds: impl Fn(&S) -> bool) {
        self.passes.retain(|(signature, _)| !holds(signature));
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.passes.len()
    }
}

#[cfg(test)]
#[path = "../tests/compose.rs"]
mod tests;
