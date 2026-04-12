use tungsten::asset_loader;
use tungsten::core::{Config, DeltaTime, ResolvedManifest, World};
use tungsten::render::TextSection;
use tungsten::{App, WindowSize};

fn extract_text(world: &World) -> Vec<TextSection> {
    let dt = world.get_resource::<DeltaTime>().unwrap();
    let ws = world
        .get_resource::<WindowSize>()
        .copied()
        .unwrap_or(WindowSize {
            width: 1280,
            height: 720,
        });
    let fps = if dt.seconds() > 0.0 {
        (1.0 / dt.seconds()).round() as u32
    } else {
        0
    };

    // HUD: fixed-width column from the right; `bounds` clips/layout within that column.
    const HUD_COL_WIDTH: f32 = 200.0;
    const HUD_MARGIN_RIGHT: f32 = 12.0;
    let hud_left = (ws.width as f32 - HUD_COL_WIDTH - HUD_MARGIN_RIGHT).max(8.0);

    vec![
        TextSection {
            content: "Tungsten Engine".into(),
            font_id: "sans_bold".into(),
            font_size: 48.0,
            line_height: 56.0,
            color: [255, 255, 255, 255],
            position: [40.0, 30.0],
            bounds: None,
        },
        TextSection {
            content: "Text rendering powered by glyphon + cosmic-text.\n\
                      Fonts are loaded from the asset manifest by ID,\n\
                      never by file path."
                .into(),
            font_id: "sans".into(),
            font_size: 20.0,
            line_height: 28.0,
            color: [200, 200, 200, 255],
            position: [40.0, 100.0],
            bounds: None,
        },
        TextSection {
            content: "This is Inter (sans-serif) at 20px.".into(),
            font_id: "sans".into(),
            font_size: 20.0,
            line_height: 28.0,
            color: [120, 200, 255, 255],
            position: [40.0, 220.0],
            bounds: None,
        },
        TextSection {
            content: "This is Inter Bold at 20px.".into(),
            font_id: "sans_bold".into(),
            font_size: 20.0,
            line_height: 28.0,
            color: [120, 200, 255, 255],
            position: [40.0, 256.0],
            bounds: None,
        },
        TextSection {
            content: "This is JetBrains Mono at 18px.".into(),
            font_id: "mono".into(),
            font_size: 18.0,
            line_height: 26.0,
            color: [180, 255, 180, 255],
            position: [40.0, 292.0],
            bounds: None,
        },
        TextSection {
            content: "fn main() {\n    println!(\"Hello, Tungsten!\");\n}".into(),
            font_id: "mono".into(),
            font_size: 16.0,
            line_height: 24.0,
            color: [220, 180, 255, 255],
            position: [40.0, 340.0],
            bounds: None,
        },
        TextSection {
            content: "Sizes: ".into(),
            font_id: "sans".into(),
            font_size: 14.0,
            line_height: 20.0,
            color: [180, 180, 180, 255],
            position: [40.0, 440.0],
            bounds: None,
        },
        TextSection {
            content: "14px".into(),
            font_id: "sans".into(),
            font_size: 14.0,
            line_height: 20.0,
            color: [255, 200, 100, 255],
            position: [90.0, 440.0],
            bounds: None,
        },
        TextSection {
            content: "20px".into(),
            font_id: "sans".into(),
            font_size: 20.0,
            line_height: 28.0,
            color: [255, 200, 100, 255],
            position: [135.0, 436.0],
            bounds: None,
        },
        TextSection {
            content: "32px".into(),
            font_id: "sans".into(),
            font_size: 32.0,
            line_height: 40.0,
            color: [255, 200, 100, 255],
            position: [190.0, 428.0],
            bounds: None,
        },
        TextSection {
            content: "48px".into(),
            font_id: "sans".into(),
            font_size: 48.0,
            line_height: 56.0,
            color: [255, 200, 100, 255],
            position: [260.0, 416.0],
            bounds: None,
        },
        // Debug overlay: FPS + frame time (one block, clipped to the HUD column)
        TextSection {
            content: format!("FPS: {fps}\ndt: {:.2}ms", dt.seconds() * 1000.0),
            font_id: "mono".into(),
            font_size: 15.0,
            line_height: 22.0,
            color: [0, 230, 80, 255],
            position: [hud_left, 10.0],
            bounds: Some([HUD_COL_WIDTH, 52.0]),
        },
    ]
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config = Config::load("tungsten.json")?;
    let mut app = App::new(config);

    app.on_startup(|world, renderer| {
        let manifest =
            ResolvedManifest::load("assets/manifest.json").expect("Failed to load manifest");
        asset_loader::load_all(&manifest, world, renderer).expect("Failed to load assets");
    });

    app.set_extract_text(extract_text);

    app.run()
}
