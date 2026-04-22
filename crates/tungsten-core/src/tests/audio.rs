use super::*;

#[test]
fn drain_returns_all_commands_and_empties_queue() {
    let mut cmds = AudioCommands::new();
    cmds.play(AudioHandle(0));
    cmds.stop(AudioHandle(1));
    cmds.stop_all();
    let drained = cmds.drain();
    assert_eq!(drained.len(), 3);
    assert!(cmds.drain().is_empty());
}

#[test]
fn drain_on_empty_returns_empty() {
    let mut cmds = AudioCommands::new();
    assert!(cmds.drain().is_empty());
}

#[test]
fn play_command_has_correct_defaults() {
    let mut cmds = AudioCommands::new();
    cmds.play(AudioHandle(5));
    let drained = cmds.drain();
    match &drained[0] {
        AudioCommand::Play {
            handle,
            volume,
            looping,
        } => {
            assert_eq!(*handle, AudioHandle(5));
            assert!((volume - 1.0).abs() < 1e-6);
            assert!(!looping);
        }
        _ => panic!("Expected Play command"),
    }
}

#[test]
fn play_looping_sets_looping_true() {
    let mut cmds = AudioCommands::new();
    cmds.play_looping(AudioHandle(3));
    let drained = cmds.drain();
    match &drained[0] {
        AudioCommand::Play { looping, .. } => assert!(*looping),
        _ => panic!("Expected Play command"),
    }
}

#[test]
fn set_master_volume_queues_correctly() {
    let mut cmds = AudioCommands::new();
    cmds.set_master_volume(0.5);
    let drained = cmds.drain();
    assert!(matches!(drained[0], AudioCommand::SetMasterVolume(v) if (v - 0.5).abs() < 1e-6));
}

#[test]
fn stop_all_queues_correctly() {
    let mut cmds = AudioCommands::new();
    cmds.stop_all();
    let drained = cmds.drain();
    assert!(matches!(drained[0], AudioCommand::StopAll));
}
