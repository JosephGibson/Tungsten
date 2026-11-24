//! The composer's rule, its section checks and the modules it generates.

use super::*;
use crate::post::bloom::{BLOOM_COMPOSITE_SECTION, BLOOM_SHADERS};
use crate::post::{STOCK_CLASSES, STOCK_SHADERS};
use tungsten_core::post::{
    DissolveParams, FadeParams, FogParams, ToneMonoMode, ToneMonoParams, VignetteParams,
};

const HEAD: Option<Class> = Some(Class::Head);
const MIX: Option<Class> = Some(Class::Mix);
const PIXEL: Option<Class> = Some(Class::Pixel);
const STEPS: Option<Class> = Some(Class::Steps);

fn stage(class: Option<Class>) -> StageInfo {
    StageInfo {
        class,
        bounded: true,
    }
}

fn stages(classes: &[Option<Class>]) -> Vec<StageInfo> {
    classes.iter().copied().map(stage).collect()
}

/// The runs of `classes`, as (start, length) pairs.
fn grouped(classes: &[Option<Class>], float: bool) -> Vec<(usize, usize)> {
    runs(&stages(classes), float)
        .into_iter()
        .map(|run| (run.start, run.len()))
        .collect()
}

fn alone(count: usize) -> Vec<(usize, usize)> {
    (0..count).map(|index| (index, 1)).collect()
}

#[test]
fn under_8_bit_targets_only_a_last_mix_fuses_with_the_stage_before_it() {
    // The stack's last two stages, when the last is a `mix` behind any
    // section: bloom's composite and a vignette, a vignette and a fade, a
    // pixelate and a fade, a dither and a fade.
    assert_eq!(grouped(&[HEAD, MIX], false), [(0, 2)]);
    assert_eq!(grouped(&[MIX, MIX], false), [(0, 2)]);
    assert_eq!(grouped(&[PIXEL, MIX], false), [(0, 2)]);
    assert_eq!(grouped(&[STEPS, MIX], false), [(0, 2)]);
    // Every stage before the pair draws alone.
    assert_eq!(
        grouped(&[HEAD, MIX, HEAD, MIX], false),
        [(0, 1), (1, 1), (2, 2)]
    );
    assert_eq!(
        grouped(&[MIX, MIX, MIX, MIX], false),
        [(0, 1), (1, 1), (2, 2)]
    );

    // Nothing follows a fused pass: bloom, vignette, dither draws alone,
    // and so does a fade and a vignette ahead of a third pass.
    assert_eq!(grouped(&[HEAD, MIX, STEPS], false), alone(3));
    assert_eq!(grouped(&[MIX, MIX, HEAD], false), alone(3));
    // A last stage that is not a `mix`: film grain, a lut, a tone, a
    // tonemap.
    assert_eq!(grouped(&[MIX, PIXEL], false), alone(2));
    assert_eq!(grouped(&[HEAD, PIXEL], false), alone(2));
    // A replaced file has no class, as the last stage or the one before.
    assert_eq!(grouped(&[None, MIX], false), alone(2));
    assert_eq!(grouped(&[HEAD, None], false), alone(2));
    // One stage, and none.
    assert_eq!(grouped(&[MIX], false), alone(1));
    assert_eq!(grouped(&[], false), alone(0));
}

#[test]
fn a_frame_fuses_no_pair_ahead_of_smaa_or_with_fusing_off() {
    let stack = stages(&[HEAD, HEAD, MIX]);
    assert_eq!(fused_pair(&stack, true, false), Some(1..3));
    // An SMAA tail follows the stack: its edge test would grow the step.
    assert_eq!(fused_pair(&stack, true, true), None);
    // `render.post_fuse = false`.
    assert_eq!(fused_pair(&stack, false, false), None);
    assert_eq!(fused_pair(&stack, false, true), None);
    // The rule itself still decides.
    assert_eq!(fused_pair(&stages(&[HEAD, PIXEL]), true, false), None);
    assert_eq!(fused_pair(&stages(&[MIX]), true, false), None);
    assert_eq!(fused_pair(&[], true, false), None);
}

#[test]
fn a_mix_whose_colour_is_negative_draws_alone() {
    let unbounded = [
        stage(HEAD),
        StageInfo {
            class: MIX,
            bounded: false,
        },
    ];
    assert_eq!(runs(&unbounded, false).len(), 2, "each stage alone");

    let colour = |red: f32| [red, 0.2, 0.1, 1.0];
    let vignette = |red| {
        PostPass::Vignette(VignetteParams {
            color: colour(red),
            ..VignetteParams::default()
        })
    };
    assert!(bounded(&vignette(0.0)));
    assert!(!bounded(&vignette(-0.01)));
    assert!(!bounded(&vignette(f32::NAN)));
    let fade = |red| {
        PostPass::Fade(FadeParams {
            color: colour(red),
            ..FadeParams::default()
        })
    };
    let fog = |red| {
        PostPass::Fog(FogParams {
            color: colour(red),
            ..FogParams::default()
        })
    };
    let dissolve = |red| {
        PostPass::Dissolve(DissolveParams {
            edge_color: colour(red),
            ..DissolveParams::default()
        })
    };
    let passes: [&dyn Fn(f32) -> PostPass; 3] = [&fade, &fog, &dissolve];
    for pass in passes {
        assert!(bounded(&pass(1.5)), "a colour above 1 is from 0 up");
        assert!(!bounded(&pass(-1.0)));
    }
    // An effect that mixes towards no colour keeps the promise whatever its
    // parameters; the alpha lane is not a colour.
    assert!(bounded(&PostPass::Pixelate(4.0)));
    assert!(bounded(&PostPass::Fade(FadeParams {
        color: [0.0, 0.0, 0.0, -1.0],
        ..FadeParams::default()
    })));
}

#[test]
fn a_duotone_is_never_a_mix_whichever_way_its_tints_run() {
    // Tints that fall as luma rises turn white's half step into three, and
    // the class is the file's, not the parameters': tone_mono is `pixel`,
    // so it ends no fused pair.
    let index = STOCK_SHADERS
        .iter()
        .position(|(name, _)| *name == "tone_mono")
        .expect("tone_mono is a stock effect");
    assert_eq!(STOCK_CLASSES[index], Class::Pixel);
    let reversed = PostPass::ToneMono(ToneMonoParams {
        mode: ToneMonoMode::Duotone,
        tint_a: [0.0, 0.0, 0.0, 1.0],
        tint_b: [1.0, 1.0, 1.0, 1.0],
        amount: 1.0,
    });
    assert!(bounded(&reversed), "its promise is not a colour's sign");
    assert_eq!(grouped(&[MIX, PIXEL], false), alone(2));
    for name in ["lut", "film_grain", "tonemap", "color_adjust"] {
        let index = STOCK_SHADERS
            .iter()
            .position(|(held, _)| *held == name)
            .expect("a stock effect");
        assert_eq!(STOCK_CLASSES[index], Class::Pixel, "{name}");
    }
}

#[test]
fn under_float_targets_the_stages_from_the_last_steps_stage_on_fuse() {
    // No quantising stage: everything fuses from the first stage, a stage
    // that leads starting a pass of its own.
    assert_eq!(grouped(&[HEAD, MIX, PIXEL, MIX], true), [(0, 4)]);
    assert_eq!(grouped(&[PIXEL, MIX, HEAD, MIX], true), [(0, 2), (2, 2)]);
    assert_eq!(grouped(&[HEAD, HEAD, MIX], true), [(0, 1), (1, 2)]);
    // The stages before the last `steps` stage draw alone, so it reads what
    // it reads unfused; it leads the pass of those after it.
    assert_eq!(
        grouped(&[PIXEL, MIX, STEPS, MIX, PIXEL], true),
        [(0, 1), (1, 1), (2, 3)]
    );
    assert_eq!(
        grouped(&[STEPS, MIX, STEPS, PIXEL], true),
        [(0, 1), (1, 1), (2, 2)]
    );
    assert_eq!(grouped(&[MIX, STEPS], true), alone(2));
    // A stage without a section draws alone and joins nothing.
    assert_eq!(
        grouped(&[PIXEL, None, MIX, MIX], true),
        [(0, 1), (1, 1), (2, 2)]
    );
    assert_eq!(grouped(&[None, MIX], true), alone(2));
    // A stage without a class may quantise, a replaced dither among them:
    // the stages before it draw alone too.
    assert_eq!(grouped(&[PIXEL, MIX, None, MIX], true), alone(4));
    assert_eq!(
        grouped(&[PIXEL, MIX, None, HEAD, MIX], true),
        [(0, 1), (1, 1), (2, 1), (3, 2)]
    );
    // Eight stages a pass at most.
    assert_eq!(grouped(&[MIX; 8], true), [(0, 8)]);
    assert_eq!(grouped(&[MIX; 9], true), [(0, 8), (8, 1)]);
    assert_eq!(grouped(&[PIXEL; 17], true), [(0, 8), (8, 8), (16, 1)]);
    // Under float targets a `mix`'s colour is no part of the rule.
    let unbounded = [
        stage(HEAD),
        StageInfo {
            class: MIX,
            bounded: false,
        },
    ];
    assert_eq!(runs(&unbounded, true), vec![0..2]);
    assert_eq!(grouped(&[], true), alone(0));
}

#[test]
fn a_mix_hands_on_the_alpha_it_is_given() {
    // A fused pass clamps only the colour between its stages (`Between`),
    // which is two passes' picture only while its last stage, a `mix`,
    // hands the alpha on untouched and reads it for nothing. The engine's
    // five are read through the same check (`engine_sections`).
    let mix = |body: &str| {
        let function = format!(
            "fn fade_fx(color: vec4<f32>, uv: vec2<f32>, p: Params) -> vec4<f32> {{\n{body}\n}}\n"
        );
        read_section("fade", &with_section("mix", &function)).map(|section| section.class)
    };
    let fault = |body: &str| mix(body).expect_err("refused");
    let t = "    let t = clamp(p.f.x, 0.0, 1.0);\n";

    assert_eq!(mix("    return color;"), Ok(Class::Mix));
    assert_eq!(
        mix(&format!("{t}    return vec4<f32>(color.rgb * t, color.a);")),
        Ok(Class::Mix)
    );
    // An early return of the colour itself, another accessor for the
    // channels, a comment that names the argument.
    assert_eq!(
        mix(&format!(
            "{t}    if (t <= 0.0) {{ return color; }} // color as it came\n    let recolor = color.xyz * t;\n    return vec4<f32>(recolor, color.a);"
        )),
        Ok(Class::Mix)
    );

    // Another alpha, wherever the `return` stands: not at its line's start,
    // split over lines, in an `else`.
    let another = "a `return` gives another alpha";
    assert!(fault("    return mix(color, p.v0, p.f.x);").contains(another));
    assert!(fault(&format!("{t}    return vec4<f32>(color.rgb * t, 1.0);")).contains(another));
    assert!(
        fault(&format!(
            "{t}    if (t <= 0.0) {{ return vec4<f32>(color.rgb, 1.0); }}\n    return vec4<f32>(color.rgb * t, color.a);"
        ))
        .contains(another)
    );
    assert!(
        fault(&format!(
            "{t}    return\n        vec4<f32>(color.rgb * t, 0.0);"
        ))
        .contains(another)
    );
    assert!(
        fault(&format!(
            "{t}    if (t > 0.5) {{ return color; }} else {{ return vec4<f32>(color.rgb * 0.5, 0.0); }}"
        ))
        .contains(another)
    );
    // The alpha read for the colour, the whole colour scaled, the alpha in
    // a wider accessor.
    let more = "for more than handing it on";
    assert!(
        fault("    return vec4<f32>(color.rgb * color.a, color.a);")
            .contains("only as the last component")
    );
    assert!(
        fault(&format!(
            "{t}    let dim = color * t;\n    return vec4<f32>(dim.rgb, color.a);"
        ))
        .contains(more)
    );
    assert!(
        fault("    let all = color.rgba;\n    return vec4<f32>(all.rgb, color.a);")
            .contains("reads `color.rgba`")
    );
    // The alpha bound to a name first could reach the colour through it:
    // it is named only where it is handed on.
    assert!(
        fault("    let a = color.a;\n    return vec4<f32>(color.rgb * a, a);")
            .contains("only as the last component")
    );
    assert!(
        fault(&format!(
            "{t}    return vec4<f32>(color.rgb * t, color.a * 1.0);"
        ))
        .contains(another)
    );
    // A comment cannot stand in for the function, nor trip the count; the
    // name may be spaced as rustfmt would never space it; the colour may go
    // by another name, a block's field of the same name beside it.
    let spaced = "/* fn fade_fx(color: vec4<f32>) { return color.a; } */\n\
                  fn  fade_fx (c: vec4<f32>, uv: vec2<f32>, p: Params) -> vec4<f32> {\n    \
                  // c.a is handed on, c is not scaled whole\n    \
                  return vec4<f32>(c.rgb * p.v0.c * p.f.x, c.a);\n}\n";
    let spaced = spaced.replace("p.v0.c", "p.v0.r");
    assert_eq!(
        read_section("fade", &with_section("mix", &spaced)).map(|section| section.class),
        Ok(Class::Mix)
    );
    let field = "fn fade_fx(x: vec4<f32>, uv: vec2<f32>, p: Params) -> vec4<f32> {\n    \
                 return vec4<f32>(x.rgb * p.f.x, x.a);\n}\n";
    assert!(read_section("fade", &with_section("mix", field)).is_ok());
    // A `pixel` is not held to it: it never ends a pass under 8-bit targets.
    let scaled = "fn fade_fx(color: vec4<f32>, uv: vec2<f32>, p: Params) -> vec4<f32> {\n    \
                  return color * p.f.x;\n}\n";
    assert!(read_section("fade", &with_section("pixel", scaled)).is_ok());
    assert!(read_section("fade", &with_section("mix", scaled)).is_err());
}

#[test]
fn a_fused_pass_holds_256_bytes_a_stage_without_a_size_attribute() {
    // Naga's GLSL writer drops `@size`, so the padding is a member: the
    // second stage's block starts at byte 256 on every backend.
    let sections = engine_sections();
    let vignette = sections
        .iter()
        .find(|section| section.name == "vignette")
        .expect("vignette");
    let text = generate(&[&sections[17], vignette], Between::Clamp, Finish::Plain);
    assert!(!text.contains("@size"));
    let module = naga::front::wgsl::parse_str(&text).expect("the fused module");
    let (_, stage) = module
        .types
        .iter()
        .find(|(_, ty)| ty.name.as_deref() == Some("Stage"))
        .expect("the block's type");
    let naga::TypeInner::Struct { members, span } = &stage.inner else {
        panic!("`Stage` is a struct");
    };
    assert_eq!(*span, 256);
    let offsets: Vec<u32> = members.iter().map(|member| member.offset).collect();
    assert_eq!(offsets, [0, 96]);
    let (_, params) = module
        .types
        .iter()
        .find(|(_, ty)| ty.name.as_deref() == Some("Params"))
        .expect("the stage's block");
    assert!(matches!(
        params.inner,
        naga::TypeInner::Struct { span: 96, .. }
    ));
}

/// Every effect with a section: the 17 stock files and bloom's composite.
fn engine_sections() -> Vec<Section> {
    let mut sections: Vec<Section> = STOCK_SHADERS
        .iter()
        .map(|(name, source)| {
            read_section(name, source).unwrap_or_else(|error| panic!("{name}: {error}"))
        })
        .collect();
    let (_, composite) = BLOOM_SHADERS[3];
    sections.push(
        read_section(BLOOM_COMPOSITE_SECTION, composite)
            .unwrap_or_else(|error| panic!("{BLOOM_COMPOSITE_SECTION}: {error}")),
    );
    sections
}

#[test]
fn every_engine_file_holds_a_valid_section_of_its_class() {
    let sections = engine_sections();
    assert_eq!(sections.len(), 18);
    for (section, class) in sections.iter().zip(STOCK_CLASSES) {
        assert_eq!(section.class, class, "{}", section.name);
    }
    assert_eq!(sections[17].class, Class::Head);
    // The five that may end a fused pair, and the one that quantises.
    let named = |class: Class| -> Vec<&str> {
        sections
            .iter()
            .filter(|section| section.class == class)
            .map(|section| section.name.as_str())
            .collect()
    };
    assert_eq!(
        named(Class::Mix),
        ["vignette", "fade", "wipe_radial", "dissolve", "fog"]
    );
    assert_eq!(named(Class::Steps), ["dither"]);
    assert_eq!(
        named(Class::Pixel),
        ["tonemap", "lut", "color_adjust", "tone_mono", "film_grain"]
    );
    // Every stock file opens with the prefix a fused module is built on.
    for (name, source) in STOCK_SHADERS.iter().chain(&BLOOM_SHADERS[3..]) {
        assert!(source.contains(prefix()), "{name} holds the shared prefix");
        let (_, text) = section_text(source).expect("markers");
        assert!(
            !text.contains("@fragment"),
            "{name}'s section holds no entry"
        );
    }
}

#[test]
fn every_section_fuses_with_every_mix_into_a_valid_module() {
    let sections = engine_sections();
    let mixes: Vec<&Section> = sections
        .iter()
        .filter(|section| section.class == Class::Mix)
        .collect();
    assert_eq!(mixes.len(), 5);
    let mut generated = 0;
    for first in &sections {
        for second in &mixes {
            for between in [Between::Clamp, Between::Nothing] {
                let module = generate(&[first, second], between, Finish::Plain);
                crate::shader_hot_reload::validate_wgsl_source("fused", &module).unwrap_or_else(
                    |error| panic!("{} then {}: {error}\n{module}", first.name, second.name),
                );
                generated += 1;
            }
        }
    }
    assert_eq!(generated, 18 * 5 * 2);

    // The pair example 01 draws: the composite on the source, a clamp, the
    // vignette on its colour, each with its own block.
    let composite = &sections[17];
    let vignette = sections
        .iter()
        .find(|section| section.name == "vignette")
        .expect("vignette");
    let module = generate(&[composite, vignette], Between::Clamp, Finish::Plain);
    assert!(module.contains("var<uniform> stages: array<Stage, 2>;"));
    assert!(!module.contains(PARAMS_BINDING));
    let body = &module[module.find("fn fs_main").expect("an entry")..];
    assert_eq!(
        body,
        "fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {\n    \
         var color = bloom_composite_head(in.uv, stages[0].p);\n    \
         color = vec4<f32>(clamp(color.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), color.a);\n    \
         color = vignette_fx(color, in.uv, stages[1].p);\n    \
         return color;\n}\n"
    );
    // A stage that follows another reads its own pixel when it leads.
    let fade = sections
        .iter()
        .find(|section| section.name == "fade")
        .expect("fade");
    let module = generate(&[vignette, fade], Between::Nothing, Finish::Plain);
    let body = &module[module.find("fn fs_main").expect("an entry")..];
    assert_eq!(
        body,
        "fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {\n    \
         var color = vignette_fx(textureSample(src, src_sampler, in.uv), in.uv, stages[0].p);\n    \
         color = fade_fx(color, in.uv, stages[1].p);\n    \
         return color;\n}\n"
    );
    // The same effect twice holds its section once and two blocks.
    let module = generate(&[fade, fade], Between::Clamp, Finish::Plain);
    assert_eq!(module.matches("fn fade_fx(").count(), 1);
    crate::shader_hot_reload::validate_wgsl_source("fade twice", &module).expect("valid");
    // Eight stages, the float rule's longest pass.
    let eight: Vec<&Section> = std::iter::once(composite)
        .chain(mixes.iter().copied().cycle().take(7))
        .collect();
    let module = generate(&eight, Between::Nothing, Finish::Plain);
    assert!(module.contains("array<Stage, 8>"));
    crate::shader_hot_reload::validate_wgsl_source("eight", &module).expect("valid");
}

/// A stock file with its section replaced by `section`.
fn with_section(class: &str, section: &str) -> String {
    let (_, fade) = STOCK_SHADERS
        .iter()
        .find(|(name, _)| *name == "fade")
        .expect("fade");
    let start = fade.find(BEGIN).expect("a section");
    let end = fade.find(END).expect("a section's end");
    format!(
        "{}{BEGIN}{class}\n{section}{}",
        &fade[..start],
        &fade[end..]
    )
}

#[test]
fn a_section_that_follows_another_stage_names_no_module_scope_variable() {
    let good = "fn fade_fx(color: vec4<f32>, uv: vec2<f32>, p: Params) -> vec4<f32> {\n    \
                return vec4<f32>(color.rgb * p.f.x, color.a);\n}\n";
    assert!(read_section("fade", &with_section("mix", good)).is_ok());
    assert!(read_section("fade", &with_section("pixel", good)).is_ok());

    // It samples the source around the colour it was handed.
    let samples = "fn fade_fx(color: vec4<f32>, uv: vec2<f32>, p: Params) -> vec4<f32> {\n    \
                   return textureSample(src, src_sampler, uv) * p.f.x;\n}\n";
    for class in ["mix", "pixel"] {
        let error = read_section("fade", &with_section(class, samples)).expect_err("refused");
        assert!(error.contains("src"), "{error}");
    }
    // It reads the module's own block, which a fused pass does not hold.
    let block = "fn fade_fx(color: vec4<f32>, uv: vec2<f32>, p: Params) -> vec4<f32> {\n    \
                 return color * params.f.x;\n}\n";
    let error = read_section("fade", &with_section("mix", block)).expect_err("refused");
    assert!(error.contains("params"), "{error}");
    // It declares a variable of its own.
    let own = "var<private> fade_seen: f32;\n\
               fn fade_fx(color: vec4<f32>, uv: vec2<f32>, p: Params) -> vec4<f32> {\n    \
               return color * fade_seen;\n}\n";
    let error = read_section("fade", &with_section("pixel", own)).expect_err("refused");
    assert!(error.contains("module-scope variable"), "{error}");

    // A `pixel` that declares the source itself would declare it twice in
    // a generated module.
    let redeclared = "@group(0) @binding(0) var src: texture_2d<f32>;\n\
                      fn fade_fx(color: vec4<f32>, uv: vec2<f32>, p: Params) -> vec4<f32> {\n    \
                      return color;\n}\n";
    let error = read_section("fade", &with_section("pixel", redeclared)).expect_err("refused");
    assert!(error.contains("module-scope variable"), "{error}");
    // A leading section may name the source, not the module's own block: a
    // generated module holds a block a stage in its place.
    let own_block = "fn fade_head(uv: vec2<f32>, p: Params) -> vec4<f32> {\n    \
                     return textureSample(src, src_sampler, uv) * params.f.x;\n}\n";
    for class in ["head", "steps"] {
        let error = read_section("fade", &with_section(class, own_block)).expect_err("refused");
        assert!(error.contains("params"), "{class}: {error}");
    }
    // A section holds no entry point.
    let entry = "fn fade_fx(color: vec4<f32>, uv: vec2<f32>, p: Params) -> vec4<f32> {\n    \
                 return color;\n}\n\
                 @fragment\nfn fade_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {\n    \
                 return vec4<f32>(uv, 0.0, 1.0);\n}\n";
    let error = read_section("fade", &with_section("pixel", entry)).expect_err("refused");
    assert!(error.contains("no entry point"), "{error}");

    // A class that leads a pass may name the source: the same text is a
    // `head`, under the leading signature.
    let head = "fn fade_head(uv: vec2<f32>, p: Params) -> vec4<f32> {\n    \
                return textureSample(src, src_sampler, uv) * p.f.x;\n}\n";
    for class in ["head", "steps"] {
        let section = read_section("fade", &with_section(class, head)).expect("a leading section");
        assert!(!section.class.follows());
    }
}

#[test]
fn a_section_is_refused_for_its_markers_its_names_or_its_signature() {
    let refused = |class: &str, section: &str| -> String {
        read_section("fade", &with_section(class, section)).expect_err("refused")
    };
    let good = "fn fade_fx(color: vec4<f32>, uv: vec2<f32>, p: Params) -> vec4<f32> {\n    \
                return color;\n}\n";
    assert!(refused("blur", good).contains("unknown class `blur`"));
    // A helper outside the effect's names could collide with another
    // effect's in a fused module.
    let helper = "fn hash12(at: vec2<f32>) -> f32 {\n    return at.x;\n}\n\
                  fn fade_fx(color: vec4<f32>, uv: vec2<f32>, p: Params) -> vec4<f32> {\n    \
                  return color * hash12(uv);\n}\n";
    assert!(refused("pixel", helper).contains("function `hash12` is not named `fade_…`"));
    let constant = "const LEVELS: f32 = 4.0;\n\
                    fn fade_fx(color: vec4<f32>, uv: vec2<f32>, p: Params) -> vec4<f32> {\n    \
                    return color * LEVELS;\n}\n";
    assert!(refused("pixel", constant).contains("constant `LEVELS`"));
    // Names compare without their case, the effect's own included: a
    // constant in capitals, an effect whose ID holds one.
    let capitals = "const FADE_LEVELS: f32 = 4.0;\n\
                    fn fade_fx(color: vec4<f32>, uv: vec2<f32>, p: Params) -> vec4<f32> {\n    \
                    return color * FADE_LEVELS;\n}\n";
    assert!(read_section("fade", &with_section("pixel", capitals)).is_ok());
    let tinted = "fn Tint_fx(color: vec4<f32>, uv: vec2<f32>, p: Params) -> vec4<f32> {\n    \
                  return color;\n}\n";
    assert!(read_section("Tint", &with_section("pixel", tinted)).is_ok());
    // The function's name and shape.
    let misnamed = "fn fade_apply(color: vec4<f32>, uv: vec2<f32>, p: Params) -> vec4<f32> {\n    \
                    return color;\n}\n";
    assert!(refused("mix", misnamed).contains("no function `fade_fx`"));
    let shape = "fn fade_fx(color: vec3<f32>, uv: vec2<f32>, p: Params) -> vec4<f32> {\n    \
                 return vec4<f32>(color, 1.0);\n}\n";
    assert!(refused("mix", shape).contains("fn(vec4<f32>, vec2<f32>, Params) -> vec4<f32>"));
    assert!(refused("head", good).contains("no function `fade_head`"));
    // Text that is not WGSL.
    assert!(refused("mix", "fn fade_fx(").len() > 10);

    // The markers themselves.
    let (_, fade) = STOCK_SHADERS
        .iter()
        .find(|(name, _)| *name == "fade")
        .expect("fade");
    let unmarked = fade.replace(BEGIN, "// fx begin ");
    assert!(section_text(&unmarked).is_err());
    let twice = format!("{fade}\n{BEGIN}mix\n{END}\n");
    assert!(section_text(&twice).is_err());
    let unended = fade.replace(END, "// fx end");
    assert!(section_text(&unended).is_err());
}

#[test]
fn a_pass_ends_in_the_tail_and_the_tail_stands_alone() {
    let sections = engine_sections();
    let vignette = sections
        .iter()
        .find(|section| section.name == "vignette")
        .expect("vignette");
    let valid = |module: &str| {
        crate::shader_hot_reload::validate_wgsl_source("tail", module)
            .unwrap_or_else(|error| panic!("{error}\n{module}"));
    };
    for srgb in [true, false] {
        let tail = Finish::Tail { srgb };
        // Every engine section alone with the tail: a stack of one stage
        // under a float scene.
        for section in &sections {
            valid(&generate(&[section], Between::Nothing, tail));
        }
        // The composite, a vignette and the tail: example 01's last pass.
        let module = generate(&[&sections[17], vignette], Between::Nothing, tail);
        valid(&module);
        let body = &module[module.find("fn fs_main").expect("an entry")..];
        assert_eq!(
            body,
            "fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {\n    \
             var color = bloom_composite_head(in.uv, stages[0].p);\n    \
             color = vignette_fx(color, in.uv, stages[1].p);\n    \
             color = tungsten_tail(color, in.position.xy);\n    \
             return color;\n}\n"
        );
        // An sRGB output dithers the value it stores.
        assert_eq!(module.contains("fn tungsten_tail_encode("), srgb);
        assert_eq!(module.contains("fn tungsten_tail_decode("), srgb);

        // The tail alone: the source's own pixel, clamped and dithered.
        let alone = generate(&[], Between::Nothing, tail);
        valid(&alone);
        let body = &alone[alone.find("fn fs_main").expect("an entry")..];
        assert_eq!(
            body,
            "fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {\n    \
             var color = textureSample(src, src_sampler, in.uv);\n    \
             color = tungsten_tail(color, in.position.xy);\n    \
             return color;\n}\n"
        );
        assert!(alone.contains("var<uniform> stages: array<Stage, 1>;"));
    }
    // A pass that writes a version of the frame's colour holds no tail.
    let plain = generate(&[vignette], Between::Nothing, Finish::Plain);
    assert!(!plain.contains("tungsten_tail"));

    // The noise is the sixteen Bayer levels about zero, scaled by the span:
    // each within 0.3 of an 8-bit step either way, so a colour within a
    // fifth of a step of a value 8 bits hold is stored as that value. The
    // shader's own lines are read, not a copy of their arithmetic.
    let module = generate(&[], Between::Nothing, Finish::Tail { srgb: true });
    let lines: Vec<&str> = module.lines().map(str::trim).collect();
    assert!(
        lines.contains(
            &"let level = (TUNGSTEN_TAIL_BAYER[cell.y * 4u + cell.x] + 0.5) / 16.0 - 0.5;"
        ),
        "the level's line moved: update this test's arithmetic with it"
    );
    let scale = format!("return level * {TAIL_NOISE_SPAN:?} / 255.0;");
    assert!(
        lines.contains(&scale.as_str()),
        "the tail no longer reads `{scale}`"
    );
    let start = module.find("array<f32, 16>(").expect("the matrix") + "array<f32, 16>(".len();
    let end = start + module[start..].find(");").expect("its end");
    let mut levels: Vec<f32> = module[start..end]
        .split(',')
        .filter_map(|level| level.trim().parse().ok())
        .collect();
    assert_eq!(levels.len(), 16);
    for level in &levels {
        let noise = ((level + 0.5) / 16.0 - 0.5) * TAIL_NOISE_SPAN;
        assert!(noise.abs() < 0.3, "{noise}");
    }
    levels.sort_by(f32::total_cmp);
    assert_eq!(levels, (0..16u8).map(f32::from).collect::<Vec<_>>());
}

#[test]
fn the_tail_rides_only_a_last_run_of_sectioned_stages() {
    let mix = stage(Some(Class::Mix));
    let steps = stage(Some(Class::Steps));
    let none = stage(None);
    let rides = |stages: &[StageInfo]| -> Vec<bool> {
        let runs = runs(stages, true);
        (0..runs.len())
            .map(|index| tail_rides(stages, &runs, index))
            .collect()
    };
    // One run of two sections: the tail ends it.
    assert_eq!(rides(&[mix, mix]), [true]);
    assert_eq!(rides(&[steps]), [true]);
    // A quantiser starts the last run, which the tail still ends; the run
    // before it never takes the tail.
    assert_eq!(rides(&[mix, steps, mix]), [false, true]);
    // A stage without a section, a replaced file's or a game's module that
    // marks none, draws with its own pipeline: the tail draws alone, last
    // in the stack or not.
    assert_eq!(rides(&[mix, none]), [false, false]);
    assert_eq!(rides(&[none]), [false]);
    assert_eq!(rides(&[none, mix]), [false, true]);
    // An empty stack has no run to ride.
    assert!(rides(&[]).is_empty());
}

#[test]
fn a_fused_pass_is_labelled_by_its_first_slot_and_its_stages() {
    assert_eq!(
        label(0, &["bloom_composite", "vignette"]),
        "post0_bloom_composite_vignette",
        "the fold's label of before"
    );
    assert_eq!(label(2, &["pixelate", "fade"]), "post2_pixelate_fade");
}

#[test]
fn a_fused_pass_is_built_once_and_again_after_a_member_reloads() {
    let mut built = 0;
    let mut fused: Fused<(&str, &str), u32> = Fused::default();
    let mut build = |fused: &mut Fused<(&str, &str), u32>, signature, ok: bool| {
        fused
            .get_or_build(signature, || {
                built += 1;
                ok.then_some(built)
            })
            .copied()
    };
    let pair = ("bloom_composite", "vignette");
    let other = ("vignette", "fade");
    assert_eq!(
        fused.get(&pair),
        None,
        "nothing is built before its first use"
    );
    assert_eq!(build(&mut fused, pair, true), Some(1));
    assert_eq!(
        build(&mut fused, pair, true),
        Some(1),
        "one build a signature"
    );
    assert_eq!(fused.get(&pair), Some(&1));
    assert_eq!(build(&mut fused, other, true), Some(2));

    // A build that fails is held as failed: its stages draw alone, and no
    // frame builds it again.
    let broken = ("glitch", "fog");
    assert_eq!(build(&mut fused, broken, false), None);
    assert_eq!(build(&mut fused, broken, true), None, "not retried");
    assert_eq!(fused.get(&broken), None);
    assert_eq!(fused.len(), 3);

    // A member's file reloads: every pass that holds it goes, built or
    // failed, and is built again at its next use.
    fused.forget(|signature| signature.0 == "vignette" || signature.1 == "vignette");
    assert_eq!(fused.len(), 1);
    assert_eq!(build(&mut fused, pair, true), Some(4));
    fused.forget(|signature| signature.1 == "fog");
    assert_eq!(build(&mut fused, broken, true), Some(5), "a reload retries");
}
