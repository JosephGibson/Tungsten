//! Screen files as states (redo 7.20): a [`DataScreen`] builds a
//! `ui_screens` file in its `on_enter`, adopts the root as a
//! [`ModalScreen`], runs each control's `on` action and refreshes the
//! file's bindings each frame. [`open`](super::open) pushes one by the
//! file's ID.

use tungsten_core::ui::{
    KeyedAction, ScreenAction, ScreenSetting, UiAction, UiBindings, UiEvent, UiRouter, UiTree,
    UiValue, WidgetId, screen_build,
};
use tungsten_core::{AudioBus, DisplayState, EventQueue, Settings, World};

use super::{ModalScreen, open, pop};
use crate::settings::{ShippedConfig, apply_settings, shipped_volume};
use crate::state::{GameState, StateContext, StateId, StateStack};

/// A `ui_screens` file as a modal screen: built in `on_enter`, its root
/// adopted as a [`ModalScreen`] (slid in, owned, shown while covered), the
/// file's actions run, its bindings refreshed each frame. A button's and a
/// choice row's action runs at its activation, a toggle's and a slider's at
/// its commit; `pop` closes the screen, `push` opens another by ID or name,
/// `action` calls the game's registered action and `setting` writes
/// `Settings` and applies them. A control that writes a setting shows the
/// setting as set, else as it runs, unless a value binding names another.
pub struct DataScreen {
    id: StateId,
    screen: Option<ModalScreen>,
    /// The controls that show the setting they write.
    settings: Vec<(WidgetId, ScreenSetting)>,
}

impl DataScreen {
    /// The data screen of `ui_screens` file `id`: a name from
    /// [`UiScreenRegistry::intern`](tungsten_core::ui::UiScreenRegistry::intern)
    /// or a literal, its state ID. [`open`](super::open) makes one.
    #[must_use]
    pub fn new(id: StateId) -> Self {
        Self {
            id,
            screen: None,
            settings: Vec::new(),
        }
    }

    /// Writes each setting control's setting, the captured control's left
    /// alone, compare-on-write.
    fn show_settings(&self, world: &mut World) {
        let captured = world
            .get_resource::<UiRouter>()
            .and_then(UiRouter::captured);
        for &(widget, setting) in &self.settings {
            if captured == Some(widget) {
                continue;
            }
            let value = setting_value(world, setting);
            let tree = world.resource_mut::<UiTree>();
            if let Some(on) = value.as_bool() {
                tree.set_toggled(widget, on);
            } else if let Some(number) = value.as_number() {
                tree.set_slider_value(widget, number);
            }
        }
    }
}

impl GameState for DataScreen {
    fn id(&self) -> StateId {
        self.id
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let world = &mut *ctx.world;
        let Some(built) = screen_build::build(world, self.id) else {
            world.resource_mut::<StateStack>().request_pop();
            return;
        };
        let screen = ModalScreen::adopt(world, built.root, built.card);
        self.settings = screen_build::actions(world, built.root)
            .into_iter()
            .filter_map(|(widget, keyed)| match keyed.action {
                ScreenAction::Setting(setting) if !keyed.value_bound => Some((widget, setting)),
                _ => None,
            })
            .collect();
        self.show_settings(world);
        screen.resume(world);
        self.screen = Some(screen);
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn on_resume(&mut self, ctx: &mut StateContext) {
        if let Some(screen) = &self.screen {
            screen.resume(ctx.world);
        }
    }

    fn update(&mut self, world: &mut World) {
        let Some(mut screen) = self.screen else {
            return;
        };
        if screen.ready(world) {
            let activated = run_actions(world, screen.root);
            screen.opener = activated.or(screen.opener);
        }
        self.screen = Some(screen);
        screen_build::refresh(world, screen.root);
        self.show_settings(world);
    }
}

/// Runs the action of each control on `root` used at this routing, one
/// event a control kind: a button's and a choice row's at its activation,
/// a toggle's and a slider's at its commit. Returns the last control
/// activated, the opener of a screen its action opens.
fn run_actions(world: &mut World, root: WidgetId) -> Option<WidgetId> {
    let used: Vec<(WidgetId, Option<UiValue>)> = {
        let tree = world.resource::<UiTree>();
        let events = world.resource::<EventQueue<UiEvent>>();
        events
            .iter_current()
            .filter_map(|event| match *event {
                UiEvent::Activated { widget, .. } => Some((widget, None)),
                UiEvent::ValueCommitted { widget, value, .. } => Some((widget, Some(value))),
                _ => None,
            })
            .filter(|(widget, _)| tree.root_of(*widget) == Some(root))
            .collect()
    };
    let mut activated = None;
    for (widget, value) in used {
        if value.is_none() {
            activated = Some(widget);
        }
        let Some(keyed) = screen_build::action(world, root, widget) else {
            continue;
        };
        if keyed.on_commit == value.is_some() {
            run(world, &keyed, value);
        }
    }
    activated
}

/// Runs `keyed`'s action with the control's `value`.
fn run(world: &mut World, keyed: &KeyedAction, value: Option<UiValue>) {
    match &keyed.action {
        ScreenAction::Pop => {
            pop(world);
        }
        ScreenAction::Push(id) => {
            open(world, id);
        }
        ScreenAction::Named(name) => {
            let action = world
                .get_resource::<UiBindings>()
                .and_then(|bindings| bindings.action_named(name));
            let Some(action) = action else {
                log::warn!("ui screen action '{name}' is not registered; nothing runs");
                return;
            };
            let call = UiAction {
                key: keyed.key.clone(),
                row: keyed.row.clone(),
                value,
            };
            action(world, call);
        }
        ScreenAction::Setting(setting) => {
            if let Some(value) = value {
                write_setting(world, *setting, value);
            }
        }
    }
}

/// Writes `setting` from a control's `value` and applies the settings.
fn write_setting(world: &mut World, setting: ScreenSetting, value: UiValue) {
    let Some(settings) = world.get_resource_mut::<Settings>() else {
        return;
    };
    match (setting, value.as_bool(), value.as_number()) {
        (ScreenSetting::Vsync, Some(on), _) => settings.set_vsync(Some(on)),
        (ScreenSetting::MasterVolume, _, Some(volume)) => settings.set_master_volume(Some(volume)),
        (ScreenSetting::MusicVolume, _, Some(volume)) => {
            settings.set_bus_volume(AudioBus::Music, Some(volume));
        }
        (ScreenSetting::EffectsVolume, _, Some(volume)) => {
            settings.set_bus_volume(AudioBus::Effects, Some(volume));
        }
        (ScreenSetting::UiScale, _, Some(scale)) => settings.set_ui_scale(Some(scale)),
        (ScreenSetting::TextScale, _, Some(scale)) => settings.set_text_scale(Some(scale)),
        _ => return,
    }
    apply_settings(world);
}

/// `setting` as set, else as it runs: the display's vsync, the shipped
/// volumes, the shipped UI scale and a text scale of 1.
fn setting_value(world: &World, setting: ScreenSetting) -> UiValue {
    let settings = world.get_resource::<Settings>();
    let volume = |set: Option<f32>, bus: Option<AudioBus>| {
        UiValue::Number(set.unwrap_or_else(|| shipped_volume(world, bus)))
    };
    match setting {
        ScreenSetting::Vsync => {
            let live = world
                .get_resource::<DisplayState>()
                .is_some_and(|display| display.vsync);
            UiValue::Bool(settings.and_then(Settings::vsync).unwrap_or(live))
        }
        ScreenSetting::MasterVolume => volume(settings.and_then(Settings::master_volume), None),
        ScreenSetting::MusicVolume => {
            let set = settings.and_then(|settings| settings.bus_volume(AudioBus::Music));
            volume(set, Some(AudioBus::Music))
        }
        ScreenSetting::EffectsVolume => {
            let set = settings.and_then(|settings| settings.bus_volume(AudioBus::Effects));
            volume(set, Some(AudioBus::Effects))
        }
        ScreenSetting::UiScale => {
            let shipped = world
                .get_resource::<ShippedConfig>()
                .and_then(|ShippedConfig(config)| config.ui.scale);
            let set = settings.and_then(Settings::ui_scale);
            UiValue::Number(set.or(shipped).unwrap_or(1.0))
        }
        ScreenSetting::TextScale => {
            UiValue::Number(settings.and_then(Settings::text_scale).unwrap_or(1.0))
        }
    }
}
