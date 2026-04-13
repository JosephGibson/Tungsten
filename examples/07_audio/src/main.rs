use tungsten::asset_loader;
use tungsten::core::{
    AudioCommands, AudioHandle, Config, InputState, KeyCode, ResolvedManifest, SoundRegistry, World,
};
use tungsten::render::TextSection;
use tungsten::App;

/// Persists audio playback state across frames.
struct AudioState {
    sfx_handle: AudioHandle,
    music_handle: AudioHandle,
    music_playing: bool,
    master_volume: f32,
    sfx_count: u32,
}

fn audio_input_system(world: &mut World) {
    let just_pressed_space;
    let just_pressed_m;
    let just_pressed_1;
    let just_pressed_2;
    let just_pressed_3;
    let just_pressed_s;

    {
        let input = match world.get_resource::<InputState>() {
            Some(i) => i,
            None => return,
        };
        just_pressed_space = input.just_pressed(KeyCode::Space);
        just_pressed_m = input.just_pressed(KeyCode::KeyM);
        just_pressed_1 = input.just_pressed(KeyCode::Digit1);
        just_pressed_2 = input.just_pressed(KeyCode::Digit2);
        just_pressed_3 = input.just_pressed(KeyCode::Digit3);
        just_pressed_s = input.just_pressed(KeyCode::KeyS);
    }

    let state_ref = world.get_resource::<AudioState>();
    if state_ref.is_none() {
        return;
    }

    // Collect commands to send, then apply state changes.
    let mut play_sfx = false;
    let mut toggle_music = false;
    let mut new_volume: Option<f32> = None;
    let mut stop_all = false;

    if just_pressed_space {
        play_sfx = true;
    }
    if just_pressed_m {
        toggle_music = true;
    }
    if just_pressed_1 {
        new_volume = Some(0.2);
    }
    if just_pressed_2 {
        new_volume = Some(0.5);
    }
    if just_pressed_3 {
        new_volume = Some(1.0);
    }
    if just_pressed_s {
        stop_all = true;
    }

    // Apply to AudioState resource.
    {
        let state = world.get_resource_mut::<AudioState>().unwrap();
        if let Some(v) = new_volume {
            state.master_volume = v;
        }
        if just_pressed_space {
            state.sfx_count += 1;
        }
        if stop_all {
            state.music_playing = false;
        }
        if toggle_music {
            state.music_playing = !state.music_playing;
        }
    }

    // Send audio commands.
    let (sfx_handle, music_handle, music_was_playing, master_vol);
    {
        let state = world.get_resource::<AudioState>().unwrap();
        sfx_handle = state.sfx_handle;
        music_handle = state.music_handle;
        master_vol = state.master_volume;
        // After toggle: state.music_playing reflects the NEW desired state.
        music_was_playing = state.music_playing;
    }

    let cmds = world.get_resource_mut::<AudioCommands>().unwrap();

    if let Some(v) = new_volume {
        cmds.set_master_volume(v);
        let _ = v; // used above
    }
    if stop_all {
        cmds.stop_all();
    }
    if play_sfx {
        cmds.play(sfx_handle);
    }
    if toggle_music {
        if music_was_playing {
            // Just toggled ON — start music.
            cmds.play_with(music_handle, master_vol, true);
        } else {
            // Just toggled OFF — stop music.
            cmds.stop(music_handle);
        }
    }
}

fn extract_text(world: &World) -> Vec<TextSection> {
    let state = match world.get_resource::<AudioState>() {
        Some(s) => s,
        None => return vec![],
    };

    let music_status = if state.music_playing {
        "playing"
    } else {
        "stopped"
    };
    let volume_pct = (state.master_volume * 100.0).round() as u32;

    vec![
        TextSection {
            content: "Tungsten Audio Demo".into(),
            font_id: "sans_bold".into(),
            font_size: 36.0,
            line_height: 44.0,
            color: [255, 255, 255, 255],
            position: [40.0, 30.0],
            bounds: None,
        },
        TextSection {
            content: "[Space]   Play sound effect\n\
                      [M]       Toggle background music\n\
                      [1]       Volume: Low (20%)\n\
                      [2]       Volume: Medium (50%)\n\
                      [3]       Volume: Full (100%)\n\
                      [S]       Stop all sounds"
                .into(),
            font_id: "mono".into(),
            font_size: 15.0,
            line_height: 24.0,
            color: [180, 180, 180, 255],
            position: [40.0, 110.0],
            bounds: None,
        },
        TextSection {
            content: format!(
                "Music:         {music_status}\n\
                 Master volume: {volume_pct}%\n\
                 SFX played:    {}",
                state.sfx_count
            ),
            font_id: "mono".into(),
            font_size: 16.0,
            line_height: 26.0,
            color: [80, 220, 120, 255],
            position: [40.0, 320.0],
            bounds: None,
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

        // Resolve sound handles from the registry populated by load_sounds().
        let (sfx_handle, music_handle) = {
            let reg = world
                .get_resource::<SoundRegistry>()
                .expect("SoundRegistry missing");
            let sfx = reg
                .get_by_id("sfx_blip")
                .expect("sfx_blip not found in manifest");
            let music = reg
                .get_by_id("music_main")
                .expect("music_main not found in manifest");
            (sfx, music)
        };

        world.insert_resource(AudioState {
            sfx_handle,
            music_handle,
            music_playing: false,
            master_volume: 0.5,
            sfx_count: 0,
        });

        // Set initial master volume.
        if let Some(cmds) = world.get_resource_mut::<AudioCommands>() {
            cmds.set_master_volume(0.5);
        }
    });

    app.add_system(audio_input_system);
    app.set_extract_text(extract_text);

    app.run()
}
