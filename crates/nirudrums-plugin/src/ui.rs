use crate::params::{NirudrumsParams, ReverbModeParam, SequencerModeParam};
use crossbeam_queue::ArrayQueue;
use nih_plug::prelude::*;
use nih_plug_egui::{
    create_egui_editor,
    egui::{self, Align, Align2, Color32, FontId, Frame, Layout, RichText, Stroke, Vec2},
    resizable_window::ResizableWindow,
};
use nirudrums_core::{
    available_kits, factory_presets, Engine, EngineCommand, EngineSettings, Kit, Meters, Project,
    ReverbMode, SequencerMode, MAX_SONG_SLOTS, NUM_INSTRUMENTS,
};
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const INK: Color32 = Color32::from_rgb(13, 13, 14);
const PANEL: Color32 = Color32::from_rgb(21, 21, 23);
const PANEL_ALT: Color32 = Color32::from_rgb(28, 28, 31);
const BONE: Color32 = Color32::from_rgb(226, 222, 210);
const MUTED: Color32 = Color32::from_rgb(130, 126, 118);
const ACCENT: Color32 = Color32::from_rgb(180, 56, 48);
const ACCENT_DARK: Color32 = Color32::from_rgb(96, 31, 29);
const GOLD: Color32 = Color32::from_rgb(196, 162, 92);
const GRID_OFF: Color32 = Color32::from_rgb(43, 43, 47);
const GRID_ON: Color32 = Color32::from_rgb(188, 74, 60);

#[derive(Clone, Copy, Debug)]
struct SongDrag {
    source: Option<usize>,
    pattern: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Pattern,
    Kit,
    Mixer,
    Song,
    Fx,
    Settings,
}

pub struct UiState {
    tab: Tab,
    selected_instrument: usize,
    selected_step: usize,
    selected_song_slot: usize,
    paint_value: Option<u8>,
    paint_instrument: Option<usize>,
    velocity_edit: bool,
    show_help: bool,
    preset_id: String,
    undo: Vec<Project>,
    redo: Vec<Project>,
    clipboard: Option<nirudrums_core::Pattern>,
    project_path: String,
    song_folder: String,
    song_filename: String,
    preset_path: String,
    kit_path: String,
    sound_path: String,
    export_path: String,
    export_bars: u32,
    status: String,
    last_parent_check: Instant,
    parent_size: Option<(u32, u32)>,
}

impl UiState {
    fn new() -> Self {
        let base = home_dir().join("NIRUDRUMS");
        Self {
            tab: Tab::Pattern,
            selected_instrument: 0,
            selected_step: 0,
            selected_song_slot: 0,
            paint_value: None,
            paint_instrument: None,
            velocity_edit: false,
            show_help: false,
            preset_id: "init".into(),
            undo: Vec::new(),
            redo: Vec::new(),
            clipboard: None,
            project_path: base.join("Songs/mercy-80-song.json").display().to_string(),
            song_folder: base.join("Songs").display().to_string(),
            song_filename: "mercy-80-song.json".into(),
            preset_path: base.join("presets/user-preset.json").display().to_string(),
            kit_path: base.join("kits").display().to_string(),
            sound_path: base.join("samples/user-sound.wav").display().to_string(),
            export_path: base.join("mercy-80-export.wav").display().to_string(),
            export_bars: 4,
            status: "Ready. MERCY-80 factory kit loaded.".into(),
            last_parent_check: Instant::now(),
            parent_size: None,
        }
    }

    fn snapshot(&mut self, project: &Project) {
        self.undo.push(project.clone());
        if self.undo.len() > 64 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
}

pub fn create_editor(
    params: Arc<NirudrumsParams>,
    kit: Arc<RwLock<Arc<Kit>>>,
    kit_swap: Arc<ArrayQueue<Arc<Kit>>>,
    queue: Arc<ArrayQueue<EngineCommand>>,
    meters: Arc<Meters>,
    current_step: Arc<AtomicU32>,
) -> Option<Box<dyn Editor>> {
    let egui_state = params.editor_state.clone();
    create_egui_editor(
        egui_state.clone(),
        UiState::new(),
        |ctx, _state| install_theme(ctx),
        move |ctx, setter, state| {
            sync_standalone_window(ctx, state);

            let mut project_guard = params.project.write().expect("project lock poisoned");
            let project = &mut *project_guard;

            let current_kit = kit.read().clone();
            top_bar(ctx, project, &params, setter, state, &current_step);
            nav_bar(ctx, state, &current_kit, project, &params, setter);
            status_bar(ctx, state, &meters, &current_step);

            ResizableWindow::new("nirudrums-main")
                .min_size(Vec2::new(980.0, 680.0))
                .show(ctx, egui_state.as_ref(), |ui| match state.tab {
                    Tab::Pattern => {
                        pattern_tab(ui, project, &params, setter, state, &queue, &current_step)
                    }
                    Tab::Kit => kit_tab(
                        ui,
                        project,
                        &params,
                        setter,
                        state,
                        &queue,
                        &current_kit,
                        &kit,
                        &kit_swap,
                    ),
                    Tab::Mixer => mixer_tab(ui, &params, setter, &meters, state),
                    Tab::Song => song_tab(ui, project, &params, setter, state),
                    Tab::Fx => fx_tab(ui, &params, setter),
                    Tab::Settings => {
                        settings_tab(ui, project, &params, setter, state, &current_kit)
                    }
                });

            help_window(ctx, state);
            ctx.request_repaint_after(Duration::from_millis(33));
        },
    )
}

fn sync_standalone_window(ctx: &egui::Context, state: &mut UiState) {
    if state.last_parent_check.elapsed() < Duration::from_millis(750) {
        return;
    }
    state.last_parent_check = Instant::now();
    let Some((width, height)) = standalone_parent_size() else {
        return;
    };
    if state.parent_size == Some((width, height)) {
        return;
    }
    state.parent_size = Some((width, height));
    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(
        width as f32,
        height as f32,
    )));
}

fn standalone_parent_size() -> Option<(u32, u32)> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, Window};

    let executable = std::env::current_exe().ok()?;
    let executable_name = executable
        .file_name()?
        .to_string_lossy()
        .to_ascii_lowercase();
    if !executable_name.contains("nirudrums") {
        return None;
    }

    fn children(connection: &impl Connection, window: Window) -> Vec<Window> {
        connection
            .query_tree(window)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .map(|reply| reply.children)
            .unwrap_or_default()
    }

    fn name(connection: &impl Connection, window: Window) -> Option<String> {
        let reply = connection
            .get_property(false, window, AtomEnum::WM_NAME, AtomEnum::STRING, 0, 1024)
            .ok()?
            .reply()
            .ok()?;
        String::from_utf8(reply.value).ok()
    }

    let (connection, screen) = x11rb::connect(None).ok()?;
    let root = connection.setup().roots.get(screen)?.root;
    let mut found = None;
    for window in children(&connection, root) {
        if name(&connection, window).as_deref() != Some("NIRUDRUMS") {
            continue;
        }
        if !children(&connection, window)
            .iter()
            .any(|child| name(&connection, *child).as_deref() == Some("egui window"))
        {
            continue;
        }
        let geometry = connection.get_geometry(window).ok()?.reply().ok()?;
        if found.is_some() {
            return None;
        }
        found = Some((u32::from(geometry.width), u32::from(geometry.height)));
    }
    found
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn install_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(BONE);
    visuals.panel_fill = INK;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = Color32::from_rgb(9, 9, 10);
    visuals.faint_bg_color = PANEL_ALT;
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.0_f32, BONE);
    visuals.widgets.noninteractive.bg_fill = PANEL_ALT;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(48, 48, 52));
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(36, 36, 40);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(31, 31, 35);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(58, 44, 42);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.active.bg_fill = ACCENT_DARK;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.window_stroke = Stroke::new(1.0_f32, Color32::from_rgb(58, 58, 62));
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(6.0, 5.0);
    style.spacing.button_padding = Vec2::new(8.0, 4.0);
    style.spacing.slider_width = 150.0;
    style
        .text_styles
        .insert(egui::TextStyle::Heading, FontId::proportional(19.0));
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(13.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(12.0));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, FontId::monospace(12.0));
    ctx.set_style(style);
}

fn top_bar(
    ctx: &egui::Context,
    project: &mut Project,
    params: &NirudrumsParams,
    setter: &ParamSetter,
    state: &mut UiState,
    current_step: &AtomicU32,
) {
    let wants_keyboard = ctx.wants_keyboard_input();
    if ctx.input(|input| input.key_pressed(egui::Key::F1))
        || (!wants_keyboard && ctx.input(|input| input.key_pressed(egui::Key::Questionmark)))
    {
        state.show_help = !state.show_help;
    }
    if state.show_help && ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
        state.show_help = false;
    }

    if !wants_keyboard && !state.show_help {
        let modifier = ctx.input(|input| input.modifiers.command);
        let alt = ctx.input(|input| input.modifiers.alt);
        if modifier && ctx.input(|input| input.key_pressed(egui::Key::S)) {
            save_project(project, params, state);
        }
        if modifier && ctx.input(|input| input.key_pressed(egui::Key::O)) {
            load_project(project, params, setter, state);
        }
        if modifier && ctx.input(|input| input.key_pressed(egui::Key::C)) {
            state.clipboard = project.patterns.get(project.current_pattern).cloned();
            state.status = "Pattern copied.".into();
        }
        if modifier && ctx.input(|input| input.key_pressed(egui::Key::V)) {
            if let Some(copied) = state.clipboard.clone() {
                state.snapshot(project);
                if let Some(pattern) = project.patterns.get_mut(project.current_pattern) {
                    pattern.copy_from(&copied);
                }
                state.status = "Pattern pasted.".into();
            }
        }
        if ctx.input(|input| {
            input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace)
        }) {
            if state.tab == Tab::Pattern {
                state.snapshot(project);
                if let Some(pattern) = project.patterns.get_mut(project.current_pattern) {
                    pattern.set_velocity(state.selected_instrument, state.selected_step, 0);
                }
                state.status = "Step cleared.".into();
            } else if state.tab == Tab::Song && !project.song.is_empty() {
                state.snapshot(project);
                let index = state.selected_song_slot.min(project.song.len() - 1);
                project.song.remove(index);
                state.selected_song_slot = index.saturating_sub(1);
                state.status = "Song slot removed.".into();
            }
        }
        if alt {
            for (key, tab) in [
                (egui::Key::Num1, Tab::Pattern),
                (egui::Key::Num2, Tab::Kit),
                (egui::Key::Num3, Tab::Mixer),
                (egui::Key::Num4, Tab::Song),
                (egui::Key::Num5, Tab::Fx),
                (egui::Key::Num6, Tab::Settings),
            ] {
                if ctx.input(|input| input.key_pressed(key)) {
                    state.tab = tab;
                }
            }
        }
        if modifier && ctx.input(|input| input.key_pressed(egui::Key::Z)) {
            if ctx.input(|input| input.modifiers.shift) {
                if let Some(next) = state.redo.pop() {
                    state.undo.push(project.clone());
                    *project = next;
                    state.status = "Redo.".into();
                }
            } else if let Some(previous) = state.undo.pop() {
                state.redo.push(project.clone());
                *project = previous;
                state.status = "Undo.".into();
            }
        }
        if modifier && ctx.input(|input| input.key_pressed(egui::Key::Y)) {
            if let Some(next) = state.redo.pop() {
                state.undo.push(project.clone());
                *project = next;
                state.status = "Redo.".into();
            }
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Space)) {
            let playing = params.sequencer.playing.value();
            set_param(setter, &params.sequencer.playing, !playing);
        }
        for key in [
            egui::Key::Num1,
            egui::Key::Num2,
            egui::Key::Num3,
            egui::Key::Num4,
            egui::Key::Num5,
            egui::Key::Num6,
            egui::Key::Num7,
            egui::Key::Num8,
        ] {
            if ctx.input(|input| input.key_pressed(key)) {
                let index = match key {
                    egui::Key::Num1 => 0,
                    egui::Key::Num2 => 1,
                    egui::Key::Num3 => 2,
                    egui::Key::Num4 => 3,
                    egui::Key::Num5 => 4,
                    egui::Key::Num6 => 5,
                    egui::Key::Num7 => 6,
                    _ => 7,
                };
                if index < project.patterns.len() {
                    project.select_pattern(index);
                }
            }
        }
    }

    egui::TopBottomPanel::top("transport")
        .frame(
            Frame::new()
                .fill(PANEL)
                .inner_margin(egui::Margin::symmetric(12, 8))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 48, 52))),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("NIRUDRUMS").heading().strong().color(BONE));
                ui.label(RichText::new("MERCY-80").small().color(GOLD));
                ui.separator();

                let playing = params.sequencer.playing.value();
                let label = if playing { "STOP" } else { "PLAY" };
                let fill = if playing { ACCENT } else { ACCENT_DARK };
                if ui
                    .add(egui::Button::new(RichText::new(label).strong()).fill(fill))
                    .on_hover_text("Play / stop (Space)")
                    .clicked()
                {
                    set_param(setter, &params.sequencer.playing, !playing);
                }
                if ui
                    .button("HOME")
                    .on_hover_text("Stop and select pattern 1")
                    .clicked()
                {
                    set_param(setter, &params.sequencer.playing, false);
                    project.select_pattern(0);
                }
                ui.separator();

                let mut bpm = params.sequencer.bpm.value();
                let host = params.sequencer.follow_host.value();
                if ui
                    .add_enabled(
                        !host,
                        egui::DragValue::new(&mut bpm)
                            .range(20.0..=300.0)
                            .speed(0.5)
                            .suffix(" BPM"),
                    )
                    .changed()
                {
                    set_param(setter, &params.sequencer.bpm, bpm);
                    project.bpm = bpm;
                }
                if host {
                    ui.label(RichText::new("HOST SYNC").small().color(GOLD));
                }

                let mut swing = params.sequencer.swing.value();
                if ui
                    .add(
                        egui::Slider::new(&mut swing, 0.0..=0.75)
                            .text("SWING")
                            .show_value(false),
                    )
                    .changed()
                {
                    set_param(setter, &params.sequencer.swing, swing);
                    project.swing = swing;
                }
                ui.label(
                    RichText::new(format!("{:>2}%", (swing * 100.0).round()))
                        .monospace()
                        .small()
                        .color(MUTED),
                );

                ui.separator();
                let mut pattern = project.current_pattern;
                egui::ComboBox::from_id_salt("top-pattern")
                    .selected_text(pattern_name(project, pattern))
                    .width(150.0)
                    .show_ui(ui, |ui| {
                        for (index, candidate) in project.patterns.iter().enumerate() {
                            ui.selectable_value(
                                &mut pattern,
                                index,
                                format!("{:02}  {}", index + 1, candidate.name),
                            );
                        }
                    });
                if pattern != project.current_pattern {
                    project.select_pattern(pattern);
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let step = if params.sequencer.playing.value() {
                        (current_step.load(Ordering::Relaxed) + 1).to_string()
                    } else {
                        "--".into()
                    };
                    ui.label(
                        RichText::new(format!("STEP {step:>2}"))
                            .monospace()
                            .color(ACCENT),
                    );
                    ui.separator();
                    if ui
                        .button("?")
                        .on_hover_text("Quick guide (F1 / ?)")
                        .clicked()
                    {
                        state.show_help = true;
                    }
                    if ui
                        .button("SAVE")
                        .on_hover_text("Save session (Ctrl+S)")
                        .clicked()
                    {
                        save_project(project, params, state);
                    }
                    if ui
                        .button("REDO")
                        .on_hover_text("Redo (Ctrl+Shift+Z)")
                        .clicked()
                    {
                        if let Some(next) = state.redo.pop() {
                            state.undo.push(project.clone());
                            *project = next;
                            state.status = "Redo.".into();
                        }
                    }
                    if ui.button("UNDO").on_hover_text("Undo (Ctrl+Z)").clicked() {
                        if let Some(previous) = state.undo.pop() {
                            state.redo.push(project.clone());
                            *project = previous;
                            state.status = "Undo.".into();
                        }
                    }
                });
            });
        });
}

fn nav_bar(
    ctx: &egui::Context,
    state: &mut UiState,
    kit: &Kit,
    project: &mut Project,
    params: &NirudrumsParams,
    setter: &ParamSetter,
) {
    egui::SidePanel::left("nav")
        .resizable(false)
        .exact_width(184.0)
        .frame(
            Frame::new()
                .fill(PANEL)
                .inner_margin(egui::Margin::symmetric(10, 12)),
        )
        .show(ctx, |ui| {
            let tabs = [
                (Tab::Pattern, "PATTERN", "Edit patterns (Alt+1)"),
                (Tab::Kit, "KIT", "Kit and MIDI map (Alt+2)"),
                (Tab::Mixer, "MIXER", "Levels and routing (Alt+3)"),
                (Tab::Song, "SONG", "Song arrangement (Alt+4)"),
                (Tab::Fx, "FX", "Reverb and master bus (Alt+5)"),
                (
                    Tab::Settings,
                    "SETTINGS",
                    "Project, presets and export (Alt+6)",
                ),
            ];
            for (tab, label, hint) in tabs {
                let response = ui
                    .add_sized(
                        [ui.available_width(), 31.0],
                        egui::Button::new(RichText::new(label).strong()).selected(state.tab == tab),
                    )
                    .on_hover_text(hint);
                if response.clicked() {
                    state.tab = tab;
                }
            }
            ui.add_space(15.0);
            ui.label(RichText::new("KIT").small().color(MUTED));
            ui.label(RichText::new(&kit.name).strong().color(GOLD));
            ui.label(RichText::new(&kit.id).monospace().small().color(MUTED));

            ui.add_space(12.0);
            ui.label(RichText::new("PRESET").small().color(MUTED));
            let presets = factory_presets();
            let current_name = if state.preset_id == "init" {
                "Init".to_string()
            } else {
                presets
                    .iter()
                    .find(|preset| preset.id == state.preset_id)
                    .map(|preset| preset.name.clone())
                    .unwrap_or_else(|| "Custom".into())
            };
            let mut selected = state.preset_id.clone();
            egui::ComboBox::from_id_salt("nav-preset")
                .selected_text(RichText::new(current_name).small())
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut selected, "init".into(), "Init");
                    for preset in &presets {
                        ui.selectable_value(
                            &mut selected,
                            preset.id.clone(),
                            RichText::new(&preset.name).small(),
                        );
                    }
                });
            if selected != state.preset_id {
                apply_preset_by_id(&selected, project, params, setter, state, kit);
            }

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui
                    .add_sized(
                        [76.0, 24.0],
                        egui::Button::new(RichText::new("SAVE P").small()),
                    )
                    .on_hover_text("Save current sound as user preset")
                    .clicked()
                {
                    save_user_preset(project, params, state);
                }
                if ui
                    .add_sized(
                        [76.0, 24.0],
                        egui::Button::new(RichText::new("LOAD P").small()),
                    )
                    .on_hover_text("Load user preset")
                    .clicked()
                {
                    load_user_preset(project, params, setter, state);
                }
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui
                    .add_sized(
                        [48.0, 24.0],
                        egui::Button::new(RichText::new("SAVE").small()),
                    )
                    .on_hover_text("Save session (Ctrl+S)")
                    .clicked()
                {
                    save_project(project, params, state);
                }
                if ui
                    .add_sized(
                        [48.0, 24.0],
                        egui::Button::new(RichText::new("OPEN").small()),
                    )
                    .on_hover_text("Open session (Ctrl+O)")
                    .clicked()
                {
                    load_project(project, params, setter, state);
                }
                if ui
                    .add_sized(
                        [48.0, 24.0],
                        egui::Button::new(RichText::new("HELP").small()),
                    )
                    .on_hover_text("Keyboard shortcuts and workflow (?)")
                    .clicked()
                {
                    state.show_help = true;
                }
            });

            ui.add_space(10.0);
            ui.label(
                RichText::new(format!("{} voices", NUM_INSTRUMENTS * 16))
                    .small()
                    .color(MUTED),
            );
            ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                ui.label(
                    RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                        .small()
                        .color(MUTED),
                );
                ui.label(RichText::new("NIRU").small().color(ACCENT));
            });
        });
}

fn status_bar(ctx: &egui::Context, state: &UiState, meters: &Meters, current_step: &AtomicU32) {
    egui::TopBottomPanel::bottom("status")
        .frame(
            Frame::new()
                .fill(PANEL)
                .inner_margin(egui::Margin::symmetric(12, 4)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&state.status).small().color(MUTED));
                ui.separator();
                ui.label(
                    RichText::new("SPACE play  |  DRAG steps  |  1-8 pattern  |  ? help")
                        .small()
                        .color(MUTED),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (left, right) = meters.peak();
                    meter_bar(ui, "L", left);
                    meter_bar(ui, "R", right);
                    ui.label(
                        RichText::new(format!(
                            "step {:02}",
                            current_step.load(Ordering::Relaxed) + 1
                        ))
                        .monospace()
                        .small()
                        .color(MUTED),
                    );
                });
            });
        });
}

fn meter_bar(ui: &mut egui::Ui, label: &str, value: f32) {
    let normalized = (value.clamp(0.0, 1.0) * 100.0).round();
    ui.label(RichText::new(label).monospace().small().color(MUTED));
    ui.add_sized(
        [70.0, 10.0],
        egui::ProgressBar::new(value.clamp(0.0, 1.0))
            .fill(if value > 0.98 { ACCENT } else { GOLD })
            .text(RichText::new(format!("{normalized:>3}")).small()),
    );
}

fn pattern_tab(
    ui: &mut egui::Ui,
    project: &mut Project,
    params: &NirudrumsParams,
    setter: &ParamSetter,
    state: &mut UiState,
    queue: &ArrayQueue<EngineCommand>,
    current_step: &AtomicU32,
) {
    let pattern_index = project
        .current_pattern
        .min(project.patterns.len().saturating_sub(1));
    ui.horizontal(|ui| {
        ui.label(RichText::new("PATTERN").heading().color(BONE));
        let mut name = project
            .patterns
            .get(pattern_index)
            .map(|pattern| pattern.name.clone())
            .unwrap_or_default();
        if ui
            .add(egui::TextEdit::singleline(&mut name).desired_width(180.0))
            .changed()
        {
            if let Some(pattern) = project.patterns.get_mut(pattern_index) {
                pattern.name = name;
            }
        }
        ui.separator();
        if ui
            .button("NEW")
            .on_hover_text("Create a new pattern")
            .clicked()
        {
            state.snapshot(project);
            project.add_pattern();
        }
        if ui
            .button("DUPLICATE")
            .on_hover_text("Copy the current pattern")
            .clicked()
        {
            state.snapshot(project);
            project.duplicate_pattern();
        }
        if ui
            .button("CLEAR")
            .on_hover_text("Clear all steps in this pattern")
            .clicked()
        {
            state.snapshot(project);
            project.clear_pattern();
        }
        if ui
            .button("COPY")
            .on_hover_text("Copy pattern (Ctrl+C)")
            .clicked()
        {
            state.clipboard = project.patterns.get(pattern_index).cloned();
            state.status = "Pattern copied.".into();
        }
        if ui
            .add_enabled(state.clipboard.is_some(), egui::Button::new("PASTE"))
            .clicked()
        {
            if let Some(copied) = state.clipboard.clone() {
                state.snapshot(project);
                if let Some(pattern) = project.patterns.get_mut(pattern_index) {
                    pattern.copy_from(&copied);
                }
                state.status = "Pattern pasted.".into();
            }
        }

        let mut length = project
            .patterns
            .get(pattern_index)
            .map(|pattern| pattern.length)
            .unwrap_or(16);
        egui::ComboBox::from_id_salt("pattern-length")
            .selected_text(format!("{length} steps"))
            .width(100.0)
            .show_ui(ui, |ui| {
                for candidate in [8_u8, 12, 16, 24, 32, 48, 64] {
                    ui.selectable_value(&mut length, candidate, format!("{candidate} steps"));
                }
            });
        if project.patterns.get(pattern_index).map(|p| p.length) != Some(length) {
            project.set_length(length);
        }
        if ui
            .button("ADD TO SONG")
            .on_hover_text("Append this pattern to the song")
            .clicked()
        {
            project.add_song_slot(pattern_index as u8);
            state.status = "Pattern added to song.".into();
        }
    });
    ui.separator();

    if !ui.input(|input| input.pointer.primary_down()) {
        state.paint_value = None;
        state.paint_instrument = None;
    }
    let active_step = current_step.load(Ordering::Relaxed) as usize;
    let playing = params.sequencer.playing.value();
    egui::ScrollArea::both()
        .id_salt("pattern-grid-scroll")
        .drag_to_scroll(false)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Grid::new("pattern-grid")
                .spacing(Vec2::new(3.0, 3.0))
                .show(ui, |ui| {
                    ui.label("");
                    for step in 0..64 {
                        let beat = step % 4 == 0;
                        ui.label(
                            RichText::new(format!("{:02}", step + 1))
                                .monospace()
                                .small()
                                .color(if beat { GOLD } else { MUTED }),
                        );
                    }
                    ui.end_row();

                    for instrument in 0..NUM_INSTRUMENTS {
                        let selected = state.selected_instrument == instrument;
                        let name = instrument_name(instrument);
                        let label = ui.add_sized(
                            [104.0, 23.0],
                            egui::Button::new(
                                RichText::new(format!("{:02} {name}", instrument + 1))
                                    .small()
                                    .color(if selected { BONE } else { MUTED }),
                            )
                            .selected(selected)
                            .fill(if selected {
                                ACCENT_DARK
                            } else {
                                PANEL_ALT
                            }),
                        );
                        if label.clicked() {
                            state.selected_instrument = instrument;
                            trigger(queue, instrument, 1.0);
                        }
                        let length = project
                            .patterns
                            .get(pattern_index)
                            .map(|pattern| usize::from(pattern.length))
                            .unwrap_or(16);
                        for step in 0..64 {
                            let velocity = project
                                .patterns
                                .get(pattern_index)
                                .map(|pattern| pattern.velocity(instrument, step))
                                .unwrap_or(0);
                            let active = playing && step == active_step && step < length;
                            let in_pattern = step < length;
                            let beat = step % 4 == 0;
                            let fill = if active {
                                Color32::from_rgb(224, 104, 84)
                            } else if velocity > 0 {
                                if velocity >= 110 {
                                    GRID_ON
                                } else {
                                    Color32::from_rgb(126, 55, 48)
                                }
                            } else if beat {
                                Color32::from_rgb(50, 48, 45)
                            } else {
                                GRID_OFF
                            };
                            let text = if velocity > 0 {
                                RichText::new(format!("{velocity}"))
                                    .monospace()
                                    .small()
                                    .color(BONE)
                            } else {
                                RichText::new(" ")
                            };
                            let selected_step =
                                state.selected_step == step && state.selected_instrument == instrument;
                            let mut button = egui::Button::new(text)
                                .fill(fill)
                                .min_size(Vec2::new(23.0, 23.0));
                            if selected_step {
                                button = button.stroke(Stroke::new(1.5_f32, GOLD));
                            }
                            let response = ui
                                .add_enabled(in_pattern, button)
                                .on_hover_text(
                                    "Click or drag to paint. Right-click raises velocity. Delete clears the selected step.",
                                );
                            if in_pattern {
                                let pointer = ui.input(|input| input.pointer.interact_pos());
                                let primary_down = ui.input(|input| input.pointer.primary_down());
                                let hit = pointer.is_some_and(|position| response.rect.contains(position));
                                if primary_down && (hit || response.is_pointer_button_down_on()) {
                                    if state.paint_value.is_none() {
                                        state.snapshot(project);
                                        state.paint_value = Some(if velocity > 0 { 0 } else { 100 });
                                        state.paint_instrument = Some(instrument);
                                    }
                                    if hit && state.paint_instrument == Some(instrument) {
                                        state.selected_step = step;
                                        state.selected_instrument = instrument;
                                        if let Some(value) = state.paint_value {
                                            if let Some(pattern) = project.patterns.get_mut(pattern_index) {
                                                pattern.set_velocity(instrument, step, value);
                                            }
                                        }
                                    }
                                }
                                if response.clicked() {
                                    state.selected_step = step;
                                    state.selected_instrument = instrument;
                                }
                            }
                            if response.secondary_clicked() {
                                state.selected_step = step;
                                state.selected_instrument = instrument;
                                state.snapshot(project);
                                if let Some(pattern) = project.patterns.get_mut(pattern_index) {
                                    let current = pattern.velocity(instrument, step);
                                    pattern.set_velocity(
                                        instrument,
                                        step,
                                        if current >= 127 {
                                            50
                                        } else {
                                            current.saturating_add(20)
                                        },
                                    );
                                }
                            }
                        }
                        ui.end_row();
                    }
                });
            ui.add_space(12.0);
        });

    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("PADS").small().color(MUTED));
        for instrument in 0..NUM_INSTRUMENTS {
            let response = ui
                .add_sized(
                    [92.0, 30.0],
                    egui::Button::new(
                        RichText::new(instrument_name(instrument))
                            .small()
                            .color(BONE),
                    )
                    .fill(if state.selected_instrument == instrument {
                        ACCENT_DARK
                    } else {
                        PANEL_ALT
                    }),
                )
                .on_hover_text(format!("Audition {}", instrument_name(instrument)));
            if response.clicked() {
                state.selected_instrument = instrument;
                trigger(queue, instrument, 1.0);
            }
        }
    });

    ui.add_space(6.0);
    instrument_inspector(ui, params, setter, state.selected_instrument);
    velocity_lane(
        ui,
        project,
        state,
        pattern_index,
        state.selected_instrument,
        active_step,
    );
}

fn velocity_lane(
    ui: &mut egui::Ui,
    project: &mut Project,
    state: &mut UiState,
    pattern_index: usize,
    instrument: usize,
    active_step: usize,
) {
    let height = ui.available_height();
    if height < 70.0 {
        return;
    }

    ui.add_space(4.0);
    let size = Vec2::new(ui.available_width(), height - 4.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, PANEL_ALT);
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0_f32, Color32::from_rgb(48, 48, 52)),
        egui::StrokeKind::Inside,
    );
    painter.text(
        rect.left_top() + Vec2::new(10.0, 7.0),
        Align2::LEFT_TOP,
        format!(
            "VELOCITY LANE  /  CHANNEL {:02}  {}",
            instrument + 1,
            instrument_name(instrument)
        ),
        FontId::monospace(11.0),
        MUTED,
    );
    painter.text(
        rect.right_top() + Vec2::new(-10.0, 7.0),
        Align2::RIGHT_TOP,
        "CLICK / DRAG TO DRAW  |  RIGHT-CLICK ERASES",
        FontId::monospace(9.0),
        MUTED,
    );

    let chart = egui::Rect::from_min_max(
        rect.left_top() + Vec2::new(10.0, 28.0),
        rect.right_bottom() - Vec2::new(10.0, 24.0),
    );
    for level in [0.25_f32, 0.5, 0.75, 1.0] {
        let y = chart.bottom() - chart.height() * level;
        painter.line_segment(
            [egui::pos2(chart.left(), y), egui::pos2(chart.right(), y)],
            Stroke::new(1.0_f32, Color32::from_rgb(47, 47, 51)),
        );
    }
    painter.line_segment(
        [chart.left_bottom(), chart.right_bottom()],
        Stroke::new(1.0_f32, Color32::from_rgb(58, 58, 64)),
    );
    let values: Vec<u8> = (0..64)
        .map(|step| {
            project
                .patterns
                .get(pattern_index)
                .map(|pattern| pattern.velocity(instrument, step))
                .unwrap_or(0)
        })
        .collect();
    let step_width = chart.width() / 64.0;
    for step in (0..64).step_by(4) {
        let x = chart.left() + step as f32 * step_width + step_width * 0.5;
        painter.text(
            egui::pos2(x, chart.bottom() + 5.0),
            Align2::CENTER_TOP,
            format!("{:02}", step + 1),
            FontId::monospace(9.0),
            if step == active_step { ACCENT } else { MUTED },
        );
    }
    for (step, &velocity) in values.iter().enumerate() {
        let active = step == active_step;
        let x = chart.left() + step as f32 * step_width;
        let bar_height = chart.height() * (f32::from(velocity) / 127.0);
        let bar = egui::Rect::from_min_max(
            egui::pos2(x + 1.0, chart.bottom() - bar_height),
            egui::pos2(x + step_width - 1.0, chart.bottom()),
        );
        let color = if active {
            Color32::from_rgb(224, 104, 84)
        } else if velocity > 0 {
            GOLD.gamma_multiply(0.65 + 0.35 * f32::from(velocity) / 127.0)
        } else {
            Color32::from_rgb(48, 48, 52)
        };
        painter.rect_filled(bar, 1.0, color);
    }

    let pointer = ui.input(|input| input.pointer.interact_pos());
    let primary_down = ui.input(|input| input.pointer.primary_down());
    if let Some(position) = pointer.filter(|position| chart.contains(*position)) {
        if primary_down {
            if !state.velocity_edit {
                state.snapshot(project);
                state.velocity_edit = true;
            }
            let step = (((position.x - chart.left()) / step_width).floor() as usize).min(63);
            let velocity = (((chart.bottom() - position.y) / chart.height()) * 127.0)
                .round()
                .clamp(0.0, 127.0) as u8;
            state.selected_step = step;
            if let Some(pattern) = project.patterns.get_mut(pattern_index) {
                pattern.set_velocity(instrument, step, velocity);
            }
        }
        if response.secondary_clicked() {
            state.snapshot(project);
            let step = (((position.x - chart.left()) / step_width).floor() as usize).min(63);
            state.selected_step = step;
            if let Some(pattern) = project.patterns.get_mut(pattern_index) {
                pattern.set_velocity(instrument, step, 0);
            }
            state.status = "Velocity cleared.".into();
        }
    }
    if !primary_down {
        state.velocity_edit = false;
    }
}

#[allow(clippy::too_many_arguments)]
fn kit_tab(
    ui: &mut egui::Ui,
    project: &mut Project,
    params: &NirudrumsParams,
    setter: &ParamSetter,
    state: &mut UiState,
    queue: &ArrayQueue<EngineCommand>,
    current_kit: &Kit,
    kit_slot: &RwLock<Arc<Kit>>,
    kit_swap: &ArrayQueue<Arc<Kit>>,
) {
    ui.label(
        RichText::new(format!("{} KIT", current_kit.name.to_uppercase()))
            .heading()
            .color(BONE),
    );
    ui.label(RichText::new(&current_kit.description).small().color(MUTED));
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(RichText::new("SOUND REPLACEMENT").strong().color(GOLD));
        ui.label(
            RichText::new(format!(
                "Selected: CH {:02} {}",
                state.selected_instrument + 1,
                instrument_name(state.selected_instrument)
            ))
            .small()
            .color(MUTED),
        );
    });
    ui.horizontal(|ui| {
        ui.label("WAV");
        ui.add(egui::TextEdit::singleline(&mut state.sound_path).desired_width(410.0));
        if ui
            .button("REPLACE SELECTED SOUND")
            .on_hover_text("Replace the selected channel with a mono or stereo WAV")
            .clicked()
        {
            replace_sound_from_wav(
                project,
                params,
                setter,
                state,
                current_kit,
                kit_slot,
                kit_swap,
            );
        }
    });
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(RichText::new("KIT LIBRARY").strong().color(GOLD));
        let kits = available_kits();
        if kits.is_empty() {
            ui.label(
                RichText::new("No external kits found.")
                    .small()
                    .color(MUTED),
            );
        }
        for (id, name, path) in kits {
            if ui.button(name).on_hover_text(id).clicked() {
                load_kit(&path, project, params, setter, state, kit_slot, kit_swap);
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("Kit directory");
        ui.add(egui::TextEdit::singleline(&mut state.kit_path).desired_width(440.0));
        if ui.button("LOAD KIT").clicked() {
            let path = PathBuf::from(&state.kit_path);
            load_kit(&path, project, params, setter, state, kit_slot, kit_swap);
        }
    });
    ui.separator();
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("kit-grid")
            .num_columns(6)
            .spacing(Vec2::new(8.0, 5.0))
            .show(ui, |ui| {
                ui.label(RichText::new("INSTRUMENT").small().color(GOLD));
                ui.label(RichText::new("MIDI").small().color(GOLD));
                ui.label(RichText::new("VOL").small().color(GOLD));
                ui.label(RichText::new("PAN").small().color(GOLD));
                ui.label(RichText::new("TUNE").small().color(GOLD));
                ui.label(RichText::new("ACTION").small().color(GOLD));
                ui.end_row();
                for instrument in 0..NUM_INSTRUMENTS {
                    ui.label(
                        RichText::new(format!(
                            "{:02} {}",
                            instrument + 1,
                            instrument_name(instrument)
                        ))
                        .strong(),
                    );
                    let mut note = i32::from(project.midi_map[instrument]);
                    if ui
                        .add(egui::DragValue::new(&mut note).range(0..=127).speed(0.5))
                        .changed()
                    {
                        project.midi_map[instrument] = note.clamp(0, 127) as u8;
                    }
                    param_compact(
                        ui,
                        setter,
                        &params.instruments[instrument].volume_db,
                        -60.0..=12.0,
                        " dB",
                    );
                    param_compact(
                        ui,
                        setter,
                        &params.instruments[instrument].pan,
                        -1.0..=1.0,
                        "",
                    );
                    param_compact(
                        ui,
                        setter,
                        &params.instruments[instrument].pitch,
                        -24.0..=24.0,
                        " st",
                    );
                    if ui.button("AUDITION").clicked() {
                        state.selected_instrument = instrument;
                        trigger(queue, instrument, 1.0);
                    }
                    ui.end_row();
                }
            });
    });
    ui.add_space(8.0);
    instrument_inspector(ui, params, setter, state.selected_instrument);
}

fn mixer_tab(
    ui: &mut egui::Ui,
    params: &NirudrumsParams,
    setter: &ParamSetter,
    meters: &Meters,
    state: &mut UiState,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("MIXER").heading().color(BONE));
        ui.label(
            RichText::new("12 channels · responsive layout · per-channel peak meters")
                .small()
                .color(MUTED),
        );
    });
    ui.separator();

    Frame::group(ui.style())
        .fill(PANEL_ALT)
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("MASTER").small().strong().color(GOLD));
                let (left, right) = meters.peak();
                ui.label(RichText::new("L").monospace().small().color(MUTED));
                ui.add_sized(
                    [170.0, 12.0],
                    egui::ProgressBar::new(left.clamp(0.0, 1.0))
                        .fill(GOLD)
                        .text(RichText::new(level_text(left)).small()),
                );
                ui.label(RichText::new("R").monospace().small().color(MUTED));
                ui.add_sized(
                    [170.0, 12.0],
                    egui::ProgressBar::new(right.clamp(0.0, 1.0))
                        .fill(GOLD)
                        .text(RichText::new(level_text(right)).small()),
                );
                ui.label(
                    RichText::new(format!("VOL {:+.1} dB", params.master.volume_db.value()))
                        .monospace()
                        .small()
                        .color(BONE),
                );
            });
        });
    ui.add_space(6.0);

    let columns = ((ui.available_width() / 116.0).floor() as usize).clamp(2, 12);
    egui::ScrollArea::vertical()
        .id_salt("mixer-scroll")
        .show(ui, |ui| {
            egui::Grid::new("mixer-channels")
                .num_columns(columns)
                .spacing(Vec2::new(6.0, 6.0))
                .show(ui, |ui| {
                    for instrument in 0..NUM_INSTRUMENTS {
                        mixer_channel(ui, params, setter, meters, state, instrument);
                        if (instrument + 1) % columns == 0 {
                            ui.end_row();
                        }
                    }
                    if NUM_INSTRUMENTS % columns != 0 {
                        ui.end_row();
                    }
                });

            ui.add_space(10.0);
            Frame::group(ui.style())
                .fill(PANEL)
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_width(360.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("MASTER BUS").strong().color(GOLD));
                        let (left, right) = meters.peak();
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("L").monospace().small().color(MUTED));
                            ui.add_sized(
                                [140.0, 12.0],
                                egui::ProgressBar::new(left.clamp(0.0, 1.0))
                                    .fill(GOLD)
                                    .text(RichText::new(level_text(left)).small()),
                            );
                            ui.label(RichText::new("R").monospace().small().color(MUTED));
                            ui.add_sized(
                                [140.0, 12.0],
                                egui::ProgressBar::new(right.clamp(0.0, 1.0))
                                    .fill(GOLD)
                                    .text(RichText::new(level_text(right)).small()),
                            );
                        });
                        let mut master = params.master.volume_db.value();
                        if ui
                            .add(egui::Slider::new(&mut master, -60.0..=12.0).text("VOLUME"))
                            .changed()
                        {
                            set_param(setter, &params.master.volume_db, master);
                        }
                        ui.label(
                            RichText::new(format!("{master:+.1} dB"))
                                .monospace()
                                .small()
                                .color(BONE),
                        );
                        let mut drive = params.master.drive.value();
                        if ui
                            .add(egui::Slider::new(&mut drive, 0.0..=1.0).text("DRIVE"))
                            .changed()
                        {
                            set_param(setter, &params.master.drive, drive);
                        }
                        let mut limiter = params.master.limiter.value();
                        if ui.checkbox(&mut limiter, "LIMITER").changed() {
                            set_param(setter, &params.master.limiter, limiter);
                        }
                        let mut dry = params.master.dry.value();
                        if ui.checkbox(&mut dry, "DRY").changed() {
                            set_param(setter, &params.master.dry, dry);
                        }
                    });
                });
        });
}

fn mixer_channel(
    ui: &mut egui::Ui,
    params: &NirudrumsParams,
    setter: &ParamSetter,
    meters: &Meters,
    state: &mut UiState,
    instrument: usize,
) {
    let inst = &params.instruments[instrument];
    let selected = state.selected_instrument == instrument;
    let peak = meters.channel_peak(instrument);
    Frame::group(ui.style())
        .fill(if selected { ACCENT_DARK } else { PANEL_ALT })
        .inner_margin(egui::Margin::symmetric(5, 5))
        .show(ui, |ui| {
            ui.set_width(96.0);
            ui.spacing_mut().slider_width = 96.0;
            ui.vertical(|ui| {
                let name = instrument_name(instrument);
                if ui
                    .add_sized(
                        [ui.available_width(), 24.0],
                        egui::Button::new(RichText::new(name).small().strong().color(BONE))
                            .selected(selected),
                    )
                    .on_hover_text("Select channel")
                    .clicked()
                {
                    state.selected_instrument = instrument;
                }
                ui.add_sized(
                    [ui.available_width(), 12.0],
                    egui::ProgressBar::new(peak.clamp(0.0, 1.0))
                        .fill(if peak > 0.98 { ACCENT } else { GOLD })
                        .text(RichText::new(level_text(peak)).small()),
                );
                ui.add_space(2.0);
                let mut volume = inst.volume_db.value();
                ui.add_sized(
                    [ui.available_width(), 110.0],
                    egui::Slider::new(&mut volume, -60.0..=12.0)
                        .vertical()
                        .show_value(false),
                );
                if volume != inst.volume_db.value() {
                    set_param(setter, &inst.volume_db, volume);
                }
                ui.label(
                    RichText::new(format!("{volume:+.1} dB"))
                        .monospace()
                        .small()
                        .color(BONE),
                );
                ui.add_space(2.0);
                let mut pan = inst.pan.value();
                if ui
                    .add(egui::Slider::new(&mut pan, -1.0..=1.0).show_value(false))
                    .on_hover_text("Pan")
                    .changed()
                {
                    set_param(setter, &inst.pan, pan);
                }
                ui.horizontal(|ui| {
                    ui.label(RichText::new("PAN").small().color(MUTED));
                    if pan.abs() > 0.005 {
                        ui.label(RichText::new(pan_text(pan)).monospace().small().color(BONE));
                    }
                    if ui
                        .add_sized([24.0, 18.0], egui::Button::new("C").small())
                        .on_hover_text("Center pan")
                        .clicked()
                    {
                        set_param(setter, &inst.pan, 0.0);
                    }
                });
                ui.horizontal(|ui| {
                    let mut mute = inst.mute.value();
                    if ui
                        .add_sized([40.0, 19.0], egui::Button::new("M").selected(mute).small())
                        .on_hover_text("Mute")
                        .clicked()
                    {
                        mute = !mute;
                        set_param(setter, &inst.mute, mute);
                    }
                    let mut solo = inst.solo.value();
                    if ui
                        .add_sized([40.0, 19.0], egui::Button::new("S").selected(solo).small())
                        .on_hover_text("Solo")
                        .clicked()
                    {
                        solo = !solo;
                        set_param(setter, &inst.solo, solo);
                    }
                });
                let mut body = inst.body.value();
                if ui
                    .add(egui::Slider::new(&mut body, 0.0..=1.0).show_value(false))
                    .on_hover_text("Body / low-end weight")
                    .changed()
                {
                    set_param(setter, &inst.body, body);
                }
                ui.label(
                    RichText::new(format!("BODY {:>3}%", (body * 100.0).round()))
                        .monospace()
                        .small()
                        .color(MUTED),
                );
                let mut send = inst.reverb_send.value();
                if ui
                    .add(egui::Slider::new(&mut send, 0.0..=1.0).show_value(false))
                    .on_hover_text("Reverb send")
                    .changed()
                {
                    set_param(setter, &inst.reverb_send, send);
                }
                ui.label(
                    RichText::new(format!("SEND {:>3}%", (send * 100.0).round()))
                        .monospace()
                        .small()
                        .color(MUTED),
                );
            });
        });
}

fn level_text(value: f32) -> String {
    if value <= 0.0001 {
        "-inf".into()
    } else {
        format!("{:.1}", 20.0 * value.log10())
    }
}

fn pan_text(pan: f32) -> String {
    if pan.abs() < 0.005 {
        "C".into()
    } else if pan < 0.0 {
        format!("L {:>2}", (-pan * 100.0).round())
    } else {
        format!("R {:>2}", (pan * 100.0).round())
    }
}

fn drag_card(
    ui: &mut egui::Ui,
    id: egui::Id,
    drag: SongDrag,
    add: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let card = ui.scope(add);
    let response = ui
        .interact(card.response.rect, id, egui::Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::Grab)
        .on_hover_text("Drag to move; click to select");
    response.dnd_set_drag_payload(drag);
    response
}

fn song_tab(
    ui: &mut egui::Ui,
    project: &mut Project,
    params: &NirudrumsParams,
    setter: &ParamSetter,
    state: &mut UiState,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("SONG").heading().color(BONE));
        ui.label(
            RichText::new("Drag patterns into the timeline, reorder slots and set repeats.")
                .small()
                .color(MUTED),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .button("OPEN SONG")
                .on_hover_text("Open the named song file")
                .clicked()
            {
                load_song(project, params, setter, state);
            }
            if ui
                .button("SAVE SONG")
                .on_hover_text("Save the named song file")
                .clicked()
            {
                save_song(project, params, state);
            }
        });
    });
    ui.separator();

    Frame::group(ui.style())
        .fill(PANEL_ALT)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let mut song_mode = params.sequencer.song_mode.value();
                if ui
                    .checkbox(&mut song_mode, "SONG MODE")
                    .on_hover_text("Play the arrangement instead of the current pattern")
                    .changed()
                {
                    set_param(setter, &params.sequencer.song_mode, song_mode);
                    project.song_mode = song_mode;
                }
                let mut follow = params.sequencer.follow_host.value();
                if ui.checkbox(&mut follow, "FOLLOW HOST TRANSPORT").changed() {
                    set_param(setter, &params.sequencer.follow_host, follow);
                    project.follow_host = follow;
                }
                ui.separator();
                ui.label("CLOCK");
                let mut clock = params.sequencer.mode.value();
                egui::ComboBox::from_id_salt("clock-mode")
                    .selected_text(match clock {
                        SequencerModeParam::Internal => "Internal",
                        SequencerModeParam::Midi => "MIDI",
                    })
                    .width(105.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut clock, SequencerModeParam::Internal, "Internal");
                        ui.selectable_value(&mut clock, SequencerModeParam::Midi, "MIDI");
                    });
                if clock != params.sequencer.mode.value() {
                    set_param(setter, &params.sequencer.mode, clock);
                    project.sequencer_mode = match clock {
                        SequencerModeParam::Internal => SequencerMode::Internal,
                        SequencerModeParam::Midi => SequencerMode::Midi,
                    };
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .button("ADD CURRENT PATTERN")
                        .on_hover_text("Append the selected pattern to the arrangement")
                        .clicked()
                    {
                        project.add_song_slot(project.current_pattern as u8);
                        state.selected_song_slot = project.song.len().saturating_sub(1);
                        state.status = "Pattern added to song.".into();
                    }
                });
            });
            ui.label(
                RichText::new("Song loops automatically at the end.")
                    .small()
                    .color(MUTED),
            );
        });

    ui.add_space(8.0);
    Frame::group(ui.style())
        .fill(PANEL_ALT)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.label(RichText::new("SONG FILE").strong().color(GOLD));
            ui.horizontal(|ui| {
                ui.label("Title");
                ui.add(egui::TextEdit::singleline(&mut project.title).desired_width(300.0));
                ui.label(
                    RichText::new("Saved with the arrangement and pattern data.")
                        .small()
                        .color(MUTED),
                );
            });
            path_row(ui, "Folder", &mut state.song_folder);
            path_row(ui, "File", &mut state.song_filename);
            ui.horizontal(|ui| {
                if ui.button("SAVE SONG").clicked() {
                    save_song(project, params, state);
                }
                if ui.button("OPEN SONG").clicked() {
                    load_song(project, params, setter, state);
                }
                ui.label(
                    RichText::new("Existing songs in this folder:")
                        .small()
                        .color(MUTED),
                );
            });
            let mut songs: Vec<PathBuf> = std::fs::read_dir(&state.song_folder)
                .ok()
                .into_iter()
                .flatten()
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
                })
                .collect();
            songs.sort();
            if songs.is_empty() {
                ui.label(RichText::new("No song files yet.").small().color(MUTED));
            } else {
                ui.horizontal_wrapped(|ui| {
                    for path in songs.iter().take(16) {
                        let label = path
                            .file_stem()
                            .map(|stem| stem.to_string_lossy().to_string())
                            .unwrap_or_else(|| "song".into());
                        if ui
                            .small_button(label)
                            .on_hover_text(path.display().to_string())
                            .clicked()
                        {
                            load_song_path(path, project, params, setter, state);
                        }
                    }
                });
            }
        });

    ui.add_space(8.0);
    let palette: Vec<(String, u8)> = project
        .patterns
        .iter()
        .map(|pattern| (pattern.name.clone(), pattern.length))
        .collect();
    Frame::group(ui.style())
        .fill(PANEL)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.label(RichText::new("PATTERN PALETTE").strong().color(GOLD));
            ui.label(
                RichText::new("Drag a pattern card to a drop marker below, or click to select it.")
                    .small()
                    .color(MUTED),
            );
            ui.horizontal_wrapped(|ui| {
                for (index, (name, length)) in palette.iter().enumerate() {
                    let selected = project.current_pattern == index;
                    let source = drag_card(
                        ui,
                        egui::Id::new(("song-palette", index)),
                        SongDrag {
                            source: None,
                            pattern: index,
                        },
                        |ui| {
                            Frame::group(ui.style())
                                .fill(if selected { ACCENT_DARK } else { PANEL_ALT })
                                .inner_margin(egui::Margin::symmetric(8, 5))
                                .show(ui, |ui| {
                                    ui.set_width(164.0);
                                    ui.vertical(|ui| {
                                        ui.label(
                                            RichText::new(format!("{:02}  {}", index + 1, name))
                                                .small()
                                                .color(BONE),
                                        );
                                        ui.label(
                                            RichText::new(format!("{} steps  ·  drag", length))
                                                .monospace()
                                                .small()
                                                .color(MUTED),
                                        );
                                    });
                                });
                        },
                    );
                    if source.clicked() {
                        project.select_pattern(index);
                        state.status = format!("Pattern {} selected.", index + 1);
                    }
                }
            });
        });

    ui.add_space(8.0);
    let mut slots = project.song.clone();
    let mut dropped: Option<(usize, SongDrag)> = None;
    let mut clicked_slot: Option<usize> = None;
    Frame::group(ui.style())
        .fill(PANEL)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("TIMELINE").strong().color(GOLD));
                let total_steps: usize = slots
                    .iter()
                    .map(|slot| {
                        project
                            .patterns
                            .get(usize::from(slot.pattern))
                            .map(|pattern| usize::from(pattern.length) * usize::from(slot.repeats))
                            .unwrap_or(0)
                    })
                    .sum();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{:.1} bars", total_steps as f32 / 16.0))
                            .monospace()
                            .small()
                            .color(MUTED),
                    );
                });
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("ADD PATTERN").small().color(MUTED));
                let mut add_index = project.current_pattern.min(palette.len().saturating_sub(1));
                egui::ComboBox::from_id_salt("timeline-add-pattern")
                    .selected_text(
                        palette
                            .get(add_index)
                            .map(|(name, _)| name.clone())
                            .unwrap_or_default(),
                    )
                    .width(160.0)
                    .show_ui(ui, |ui| {
                        for (index, (name, _)) in palette.iter().enumerate() {
                            ui.selectable_value(&mut add_index, index, name.clone());
                        }
                    });
                if ui.button("ADD").clicked() && slots.len() < MAX_SONG_SLOTS {
                    slots.push(nirudrums_core::SongSlot {
                        pattern: add_index as u8,
                        repeats: 1,
                    });
                    state.selected_song_slot = slots.len() - 1;
                    state.status = "Pattern appended to song.".into();
                }
            });
            ui.add_space(4.0);
            egui::ScrollArea::horizontal()
                .id_salt("song-timeline")
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if slots.is_empty() {
                        let (_, payload) = ui.dnd_drop_zone::<SongDrag, _>(
                            Frame::new().inner_margin(egui::Margin::symmetric(12, 12)),
                            |ui| {
                                ui.allocate_ui_with_layout(
                                    Vec2::new(360.0, 72.0),
                                    Layout::centered_and_justified(egui::Direction::LeftToRight),
                                    |ui| {
                                        ui.label(
                                            RichText::new("DROP A PATTERN HERE")
                                                .strong()
                                                .color(MUTED),
                                        );
                                    },
                                );
                            },
                        );
                        if let Some(payload) = payload {
                            dropped = Some((0, *payload));
                        }
                    } else {
                        ui.horizontal_top(|ui| {
                            for index in 0..=slots.len() {
                                let is_end = index == slots.len();
                                let (_, payload) = ui.dnd_drop_zone::<SongDrag, _>(
                                    Frame::new().inner_margin(egui::Margin::symmetric(2, 0)),
                                    |ui| {
                                        if is_end {
                                            ui.allocate_ui_with_layout(
                                                Vec2::new(92.0, 76.0),
                                                Layout::centered_and_justified(
                                                    egui::Direction::LeftToRight,
                                                ),
                                                |ui| {
                                                    ui.label(
                                                        RichText::new("+ DROP TO ADD")
                                                            .small()
                                                            .color(MUTED),
                                                    );
                                                },
                                            );
                                        } else {
                                            ui.allocate_space(Vec2::new(10.0, 76.0));
                                        }
                                    },
                                );
                                if let Some(payload) = payload {
                                    dropped = Some((index, *payload));
                                }
                                if index == slots.len() {
                                    break;
                                }
                                let slot = slots[index];
                                let name = project
                                    .patterns
                                    .get(usize::from(slot.pattern))
                                    .map(|pattern| pattern.name.clone())
                                    .unwrap_or_else(|| "Missing".into());
                                let selected = state.selected_song_slot == index;
                                let source = drag_card(
                                    ui,
                                    egui::Id::new(("song-slot", index)),
                                    SongDrag {
                                        source: Some(index),
                                        pattern: usize::from(slot.pattern),
                                    },
                                    |ui| {
                                        Frame::group(ui.style())
                                            .fill(if selected { ACCENT_DARK } else { PANEL_ALT })
                                            .inner_margin(egui::Margin::symmetric(8, 6))
                                            .show(ui, |ui| {
                                                ui.set_width(152.0);
                                                ui.vertical(|ui| {
                                                    ui.label(
                                                        RichText::new(format!("{:02}", index + 1))
                                                            .monospace()
                                                            .small()
                                                            .color(GOLD),
                                                    );
                                                    ui.label(
                                                        RichText::new(name)
                                                            .small()
                                                            .strong()
                                                            .color(BONE),
                                                    );
                                                    ui.label(
                                                        RichText::new(format!("x{}", slot.repeats))
                                                            .monospace()
                                                            .small()
                                                            .color(MUTED),
                                                    );
                                                });
                                            });
                                    },
                                );
                                if source.clicked() {
                                    clicked_slot = Some(index);
                                }
                            }
                        });
                    }
                    if !slots.is_empty() {
                        let (_, payload) = ui.dnd_drop_zone::<SongDrag, _>(
                            Frame::new().inner_margin(egui::Margin::symmetric(8, 6)),
                            |ui| {
                                ui.allocate_ui_with_layout(
                                    Vec2::new(90.0, 76.0),
                                    Layout::centered_and_justified(egui::Direction::LeftToRight),
                                    |ui| {
                                        ui.label(
                                            RichText::new("DROP TO DELETE").small().color(ACCENT),
                                        );
                                    },
                                );
                            },
                        );
                        if let Some(payload) = payload {
                            if let Some(source) = payload.source {
                                dropped = Some((
                                    usize::MAX,
                                    SongDrag {
                                        source: Some(source),
                                        pattern: payload.pattern,
                                    },
                                ));
                            }
                        }
                    }
                });
            if slots.is_empty() {
                ui.label(
                    RichText::new("No slots yet. Drag a pattern here or use Add Current Pattern.")
                        .small()
                        .color(MUTED),
                );
            }
        });

    if let Some(index) = clicked_slot {
        state.selected_song_slot = index;
    }
    if let Some((position, drag)) = dropped {
        if position == usize::MAX {
            if let Some(source_index) = drag.source {
                if source_index < slots.len() {
                    slots.remove(source_index);
                    state.selected_song_slot = source_index.saturating_sub(1);
                    state.status = "Song slot deleted.".into();
                }
            }
        } else if let Some(source_index) = drag.source {
            if source_index < slots.len() {
                let moved = slots.remove(source_index);
                let target = if source_index < position {
                    position.saturating_sub(1)
                } else {
                    position
                }
                .min(slots.len());
                slots.insert(target, moved);
                state.selected_song_slot = target;
                state.status = "Moved song slot.".into();
            }
        } else if slots.len() < MAX_SONG_SLOTS {
            let target = position.min(slots.len());
            slots.insert(
                target,
                nirudrums_core::SongSlot {
                    pattern: drag.pattern.min(palette.len().saturating_sub(1)) as u8,
                    repeats: 1,
                },
            );
            state.selected_song_slot = target;
            state.status = "Pattern added from palette.".into();
        }
    }

    if !slots.is_empty() {
        state.selected_song_slot = state.selected_song_slot.min(slots.len() - 1);
        let index = state.selected_song_slot;
        Frame::group(ui.style())
            .fill(PANEL_ALT)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.label(RichText::new("SELECTED SLOT").strong().color(GOLD));
                ui.horizontal_wrapped(|ui| {
                    ui.label("Pattern");
                    let mut pattern = usize::from(slots[index].pattern).min(palette.len() - 1);
                    egui::ComboBox::from_id_salt("selected-slot-pattern")
                        .selected_text(
                            palette
                                .get(pattern)
                                .map(|(name, _)| name.clone())
                                .unwrap_or_default(),
                        )
                        .width(180.0)
                        .show_ui(ui, |ui| {
                            for (candidate, (name, _)) in palette.iter().enumerate() {
                                ui.selectable_value(&mut pattern, candidate, name.clone());
                            }
                        });
                    slots[index].pattern = pattern as u8;
                    ui.label("Repeats");
                    let mut repeats = i32::from(slots[index].repeats);
                    if ui
                        .add(egui::DragValue::new(&mut repeats).range(1..=32))
                        .changed()
                    {
                        slots[index].repeats = repeats.clamp(1, 32) as u8;
                    }
                    if ui.button("UP").on_hover_text("Move slot earlier").clicked() && index > 0 {
                        slots.swap(index, index - 1);
                        state.selected_song_slot = index - 1;
                        state.status = "Moved slot up.".into();
                    }
                    if ui.button("DOWN").on_hover_text("Move slot later").clicked()
                        && index + 1 < slots.len()
                    {
                        slots.swap(index, index + 1);
                        state.selected_song_slot = index + 1;
                        state.status = "Moved slot down.".into();
                    }
                    if ui
                        .button("DUPLICATE")
                        .on_hover_text("Duplicate this slot")
                        .clicked()
                        && slots.len() < MAX_SONG_SLOTS
                    {
                        let copy = slots[index];
                        slots.insert(index + 1, copy);
                        state.selected_song_slot = index + 1;
                        state.status = "Duplicated song slot.".into();
                    }
                    if ui
                        .button("REMOVE")
                        .on_hover_text("Remove this slot")
                        .clicked()
                    {
                        slots.remove(index);
                        state.selected_song_slot = index.saturating_sub(1);
                        state.status = "Song slot removed.".into();
                    }
                });
            })
            .response
            .on_hover_text("Drop zones between cards reorder; use Up/Down for precise moves.");
    }

    project.song = slots;
}

fn fx_tab(ui: &mut egui::Ui, params: &NirudrumsParams, setter: &ParamSetter) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("FX RACK").heading().color(BONE));
        ui.label(
            RichText::new(
                "Vintage processors in fixed-size modules. Scroll sideways on narrow windows.",
            )
            .small()
            .color(MUTED),
        );
    });
    ui.separator();
    egui::ScrollArea::horizontal().show(ui, |ui| {
        ui.horizontal_top(|ui| {
            fx_module(ui, "01", "REVERB", "Vintage room", |ui| {
                let mut mode = params.reverb.mode.value();
                egui::ComboBox::from_id_salt("reverb-mode")
                    .selected_text(match mode {
                        ReverbModeParam::Plate => "Plate",
                        ReverbModeParam::Room => "Room",
                        ReverbModeParam::Gated => "Gated",
                    })
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut mode, ReverbModeParam::Plate, "Plate");
                        ui.selectable_value(&mut mode, ReverbModeParam::Room, "Room");
                        ui.selectable_value(&mut mode, ReverbModeParam::Gated, "Gated");
                    });
                if mode != params.reverb.mode.value() {
                    set_param(setter, &params.reverb.mode, mode);
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    knob(ui, setter, &params.reverb.mix, "MIX", 0.0..=1.0, "", 2);
                    knob(ui, setter, &params.reverb.decay, "DECAY", 0.0..=1.0, "", 2);
                    knob(
                        ui,
                        setter,
                        &params.reverb.pre_delay_ms,
                        "PRE",
                        0.0..=200.0,
                        " ms",
                        0,
                    );
                });
                ui.horizontal(|ui| {
                    knob(ui, setter, &params.reverb.damping, "DAMP", 0.0..=1.0, "", 2);
                    knob(ui, setter, &params.reverb.width, "WIDTH", 0.0..=1.0, "", 2);
                    knob(
                        ui,
                        setter,
                        &params.reverb.gate_ms,
                        "GATE",
                        0.0..=2000.0,
                        " ms",
                        0,
                    );
                });
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Plate: dense and bright. Room: open. Gated: hard 80s stop.")
                        .small()
                        .color(MUTED),
                );
            });
            fx_module(ui, "02", "MASTER BUS", "Output stage", |ui| {
                ui.horizontal(|ui| {
                    knob(
                        ui,
                        setter,
                        &params.master.volume_db,
                        "VOLUME",
                        -60.0..=12.0,
                        " dB",
                        1,
                    );
                    knob(ui, setter, &params.master.drive, "DRIVE", 0.0..=1.0, "", 2);
                });
                ui.add_space(8.0);
                let mut limiter = params.master.limiter.value();
                if ui
                    .checkbox(&mut limiter, "LIMITER")
                    .on_hover_text("Soft output protection")
                    .changed()
                {
                    set_param(setter, &params.master.limiter, limiter);
                }
                let mut dry = params.master.dry.value();
                if ui
                    .checkbox(&mut dry, "DRY BYPASS")
                    .on_hover_text("Bypass drive, limiter and reverb")
                    .changed()
                {
                    set_param(setter, &params.master.dry, dry);
                }
                ui.add_space(6.0);
                ui.label(
                    RichText::new("DRY removes all bus processing. LIMITER protects the output.")
                        .small()
                        .color(MUTED),
                );
            });
            fx_module(ui, "03", "CHANNEL SENDS", "Reverb matrix", |ui| {
                for instrument in 0..NUM_INSTRUMENTS {
                    let inst = &params.instruments[instrument];
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(instrument_name(instrument))
                                .small()
                                .color(MUTED),
                        );
                        let mut send = inst.reverb_send.value();
                        if ui
                            .add_sized(
                                [120.0, 18.0],
                                egui::Slider::new(&mut send, 0.0..=1.0).show_value(false),
                            )
                            .changed()
                        {
                            set_param(setter, &inst.reverb_send, send);
                        }
                        ui.label(
                            RichText::new(format!("{:>3}%", (send * 100.0).round()))
                                .monospace()
                                .small()
                                .color(BONE),
                        );
                    });
                }
            });
        });
    });
}

fn fx_module(
    ui: &mut egui::Ui,
    number: &str,
    title: &str,
    subtitle: &str,
    add: impl FnOnce(&mut egui::Ui),
) {
    Frame::group(ui.style())
        .fill(PANEL_ALT)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(330.0);
            ui.set_min_height(300.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(number).monospace().small().color(ACCENT));
                    ui.label(RichText::new(title).strong().color(GOLD));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(subtitle).small().color(MUTED));
                    });
                });
                ui.add_space(4.0);
                add(ui);
            });
        });
}

fn settings_tab(
    ui: &mut egui::Ui,
    project: &mut Project,
    params: &NirudrumsParams,
    setter: &ParamSetter,
    state: &mut UiState,
    kit: &Kit,
) {
    ui.label(RichText::new("SETTINGS").heading().color(BONE));
    ui.label(
        RichText::new("Kit, presets, sessions, MIDI map and offline export.")
            .small()
            .color(MUTED),
    );
    ui.separator();
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.columns(2, |columns| {
            section(&mut columns[0], "KIT", |ui| {
                ui.label(RichText::new(format!("{} / {}", kit.name, kit.id)).strong().color(GOLD));
                ui.label(RichText::new(format!("Version {}", kit.version)).small().color(MUTED));
                ui.label(RichText::new(&kit.description).small().color(MUTED));
            });
            section(&mut columns[1], "USER PRESET", |ui| {
                path_row(ui, "Path", &mut state.preset_path);
                ui.horizontal(|ui| {
                    if ui.button("SAVE PRESET").clicked() {
                        save_user_preset(project, params, state);
                    }
                    if ui.button("LOAD PRESET").clicked() {
                        load_user_preset(project, params, setter, state);
                    }
                });
            });
            section(&mut columns[0], "MIDI MAP", |ui| {
                egui::Grid::new("midi-map")
                    .num_columns(4)
                    .spacing(Vec2::new(10.0, 3.0))
                    .show(ui, |ui| {
                        for instrument in 0..NUM_INSTRUMENTS {
                            ui.label(
                                RichText::new(instrument_name(instrument))
                                    .small()
                                    .color(MUTED),
                            );
                            let mut note = i32::from(project.midi_map[instrument]);
                            if ui
                                .add(egui::DragValue::new(&mut note).range(0..=127).speed(0.5))
                                .changed()
                            {
                                project.midi_map[instrument] = note.clamp(0, 127) as u8;
                            }
                            if instrument % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });
            });
            section(&mut columns[1], "SESSION / SONG", |ui| {
                path_row(ui, "Path", &mut state.project_path);
                ui.horizontal(|ui| {
                    if ui.button("SAVE SESSION").clicked() {
                        save_project(project, params, state);
                    }
                    if ui.button("OPEN SESSION").clicked() {
                        load_project(project, params, setter, state);
                    }
                });
                ui.label(
                    RichText::new("Projects contain patterns, song slots, MIDI map, mixer and FX.")
                        .small()
                        .color(MUTED),
                );
            });
            section(&mut columns[0], "FACTORY PRESETS", |ui| {
                ui.horizontal_wrapped(|ui| {
                    for preset in factory_presets() {
                        let name = preset.name.clone();
                        if ui.button(name).clicked() {
                            apply_preset_by_id(&preset.id, project, params, setter, state, kit);
                        }
                    }
                });
            });
            section(&mut columns[1], "WAV EXPORT", |ui| {
                path_row(ui, "Output", &mut state.export_path);
                ui.horizontal(|ui| {
                    ui.label("Bars");
                    ui.add(egui::DragValue::new(&mut state.export_bars).range(1..=64));
                    if ui.button("EXPORT WAV").clicked() {
                        let path = PathBuf::from(&state.export_path);
                        if let Some(parent) = path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        let mut export_project = project.clone();
                        sync_project_from_params(&mut export_project, params);
                        let mut settings = EngineSettings::from_project(&export_project);
                        settings.playing = true;
                        settings.sequencer_mode = SequencerMode::Internal;
                        match Engine::render_to_wav(
                            &path,
                            Arc::new(kit.clone()),
                            &export_project,
                            &settings,
                            state.export_bars,
                        ) {
                            Ok(()) => state.status = format!("Exported {}", path.display()),
                            Err(error) => state.status = error,
                        }
                    }
                });
            });
            section(&mut columns[0], "ABOUT", |ui| {
                ui.label(
                    RichText::new(
                        "NIRUDRUMS 0.0.1 — concept, design, and creative direction by Nicklas Rudolfsson.",
                    )
                    .small()
                    .color(GOLD),
                );
                ui.label(
                    RichText::new(
                        "MERCY-80 includes CC0-licensed one-shot source material. See project kit source notes for provenance.",
                    )
                    .small()
                    .color(MUTED),
                );
            });
        });
    });
}

fn path_row(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(
            egui::TextEdit::singleline(value)
                .desired_width((ui.available_width() - 8.0).max(160.0)),
        );
    });
}

fn section(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    Frame::group(ui.style())
        .fill(PANEL_ALT)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong().color(GOLD));
            ui.add_space(4.0);
            add(ui);
        });
    ui.add_space(8.0);
}

#[allow(clippy::too_many_arguments)]
fn load_project(
    project: &mut Project,
    params: &NirudrumsParams,
    setter: &ParamSetter,
    state: &mut UiState,
) {
    let path = PathBuf::from(&state.project_path);
    match Project::load(&path) {
        Ok(loaded) => {
            *project = loaded;
            apply_project_to_params(project, params, setter);
            state.preset_id = "custom".into();
            state.status = format!("Opened {}", path.display());
        }
        Err(error) => state.status = error,
    }
}

fn save_user_preset(project: &mut Project, params: &NirudrumsParams, state: &mut UiState) {
    sync_project_from_params(project, params);
    let preset = nirudrums_core::Preset::from_project(
        project,
        format!("user-{}", project.patterns.len()),
        "User Preset",
    );
    let path = PathBuf::from(&state.preset_path);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match preset.save(&path) {
        Ok(()) => state.status = format!("Preset saved {}", path.display()),
        Err(error) => state.status = error,
    }
}

fn load_user_preset(
    project: &mut Project,
    params: &NirudrumsParams,
    setter: &ParamSetter,
    state: &mut UiState,
) {
    let path = PathBuf::from(&state.preset_path);
    match nirudrums_core::Preset::load(&path) {
        Ok(preset) => {
            preset.apply(project);
            apply_project_to_params(project, params, setter);
            state.preset_id = "custom".into();
            state.status = format!("Preset loaded {}", path.display());
        }
        Err(error) => state.status = error,
    }
}

fn song_path(state: &mut UiState) -> PathBuf {
    let mut filename = state.song_filename.trim().to_string();
    if filename.is_empty() {
        filename = "untitled-song.json".into();
    }
    if !filename.to_ascii_lowercase().ends_with(".json") {
        filename.push_str(".json");
    }
    state.song_filename = filename.clone();
    PathBuf::from(&state.song_folder).join(filename)
}

fn save_song(project: &mut Project, params: &NirudrumsParams, state: &mut UiState) {
    sync_project_from_params(project, params);
    let path = song_path(state);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match project.save(&path) {
        Ok(()) => {
            state.project_path = path.display().to_string();
            state.status = format!("Saved song {}", path.display());
        }
        Err(error) => state.status = error,
    }
}

fn load_song(
    project: &mut Project,
    params: &NirudrumsParams,
    setter: &ParamSetter,
    state: &mut UiState,
) {
    let path = song_path(state);
    match Project::load(&path) {
        Ok(loaded) => {
            *project = loaded;
            apply_project_to_params(project, params, setter);
            state.preset_id = "custom".into();
            state.project_path = path.display().to_string();
            state.status = format!("Opened song {}", path.display());
        }
        Err(error) => state.status = error,
    }
}

fn load_song_path(
    path: &Path,
    project: &mut Project,
    params: &NirudrumsParams,
    setter: &ParamSetter,
    state: &mut UiState,
) {
    match Project::load(path) {
        Ok(loaded) => {
            *project = loaded;
            apply_project_to_params(project, params, setter);
            state.preset_id = "custom".into();
            state.project_path = path.display().to_string();
            if let Some(parent) = path.parent() {
                state.song_folder = parent.display().to_string();
            }
            if let Some(name) = path.file_name() {
                state.song_filename = name.to_string_lossy().to_string();
            }
            state.status = format!("Opened song {}", path.display());
        }
        Err(error) => state.status = error,
    }
}

fn save_project(project: &mut Project, params: &NirudrumsParams, state: &mut UiState) {
    sync_project_from_params(project, params);
    let path = PathBuf::from(&state.project_path);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match project.save(&path) {
        Ok(()) => state.status = format!("Saved {}", path.display()),
        Err(error) => state.status = error,
    }
}

fn apply_preset_by_id(
    id: &str,
    project: &mut Project,
    params: &NirudrumsParams,
    setter: &ParamSetter,
    state: &mut UiState,
    kit: &Kit,
) {
    if id == "init" {
        state.snapshot(project);
        let defaults = kit.default_project();
        project.kit = defaults.kit;
        project.instruments = defaults.instruments;
        project.midi_map = defaults.midi_map;
        project.master = defaults.master;
        project.reverb = defaults.reverb;
        apply_project_to_params(project, params, setter);
        state.preset_id = "init".into();
        state.status = "Init kit settings.".into();
        return;
    }

    let Some(preset) = factory_presets().into_iter().find(|preset| preset.id == id) else {
        return;
    };
    state.snapshot(project);
    preset.apply(project);
    apply_project_to_params(project, params, setter);
    state.preset_id = preset.id.clone();
    state.status = format!("Preset: {}", preset.name);
}

fn help_window(ctx: &egui::Context, state: &mut UiState) {
    if !state.show_help {
        return;
    }
    let mut open = state.show_help;
    egui::Window::new("Quick guide")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .default_width(540.0)
        .show(ctx, |ui| {
            ui.label(RichText::new("TRANSPORT").strong().color(GOLD));
            ui.label("Space: play / stop.");
            ui.label("Host sync: enable FOLLOW HOST in Song or Settings depending on the host.");
            ui.add_space(8.0);
            ui.label(RichText::new("PATTERN").strong().color(GOLD));
            ui.label("Click or drag across steps to paint. Drag a step from an active cell to erase.");
            ui.label("Right-click a step to raise velocity. Drag the velocity lane to draw exact values.");
            ui.label("1-8: select pattern.  Ctrl+C / Ctrl+V: copy / paste pattern.");
            ui.add_space(8.0);
            ui.label(RichText::new("EDITING").strong().color(GOLD));
            ui.label("Ctrl+Z: undo.  Ctrl+Shift+Z or Ctrl+Y: redo.  Delete: clear selected step.");
            ui.label("Alt+1..6: switch views.  ?: open this guide.  Esc: close.");
            ui.add_space(8.0);
            ui.label(RichText::new("PROJECT").strong().color(GOLD));
            ui.label("Ctrl+S: save session.  Save / load, presets and WAV export are in Settings.");
            ui.label("The plugin saves its full state with the host project.");
            ui.add_space(12.0);
            if ui.button("CLOSE").clicked() {
                state.show_help = false;
            }
        });
    if !open {
        state.show_help = false;
    }
}

#[allow(clippy::too_many_arguments)]
fn replace_sound_from_wav(
    project: &mut Project,
    params: &NirudrumsParams,
    setter: &ParamSetter,
    state: &mut UiState,
    current_kit: &Kit,
    kit_slot: &RwLock<Arc<Kit>>,
    kit_swap: &ArrayQueue<Arc<Kit>>,
) {
    let path = PathBuf::from(&state.sound_path);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            state.status = format!("{}: {error}", path.display());
            return;
        }
    };
    let data = match nirudrums_core::decode_wav(&bytes) {
        Ok(data) => Arc::new(data),
        Err(error) => {
            state.status = error;
            return;
        }
    };
    let instrument = state.selected_instrument.min(NUM_INSTRUMENTS - 1);
    let mut kit = current_kit.clone();
    if let Some(target) = kit.instruments.get_mut(instrument) {
        target.layers = vec![
            nirudrums_core::VelocityLayer {
                velocity_min: 0,
                velocity_max: 42,
                sample: data.clone(),
            },
            nirudrums_core::VelocityLayer {
                velocity_min: 43,
                velocity_max: 85,
                sample: data.clone(),
            },
            nirudrums_core::VelocityLayer {
                velocity_min: 86,
                velocity_max: 127,
                sample: data,
            },
        ];
    } else {
        state.status = "Selected instrument is not available in this kit.".into();
        return;
    }
    let kit = Arc::new(kit);
    *kit_slot.write() = kit.clone();
    let _ = kit_swap.push(kit);
    project.kit = current_kit.id.clone();
    apply_project_to_params(project, params, setter);
    state.status = format!(
        "Replaced {} with {}",
        instrument_name(instrument),
        path.display()
    );
}

fn load_kit(
    path: &Path,
    project: &mut Project,
    params: &NirudrumsParams,
    setter: &ParamSetter,
    state: &mut UiState,
    kit_slot: &RwLock<Arc<Kit>>,
    kit_swap: &ArrayQueue<Arc<Kit>>,
) {
    match Kit::load_from_dir(path) {
        Ok(kit) => {
            let kit = Arc::new(kit);
            let defaults = kit.default_project();
            project.kit = kit.id.clone();
            project.instruments = defaults.instruments;
            project.midi_map = defaults.midi_map;
            *kit_slot.write() = kit.clone();
            let _ = kit_swap.push(kit);
            apply_project_to_params(project, params, setter);
            state.preset_id = "init".into();
            state.status = format!("Loaded kit {}", path.display());
        }
        Err(error) => state.status = error,
    }
}

fn instrument_inspector(
    ui: &mut egui::Ui,
    params: &NirudrumsParams,
    setter: &ParamSetter,
    instrument: usize,
) {
    Frame::group(ui.style())
        .fill(PANEL_ALT)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!(
                        "CHANNEL {:02} / {}",
                        instrument + 1,
                        instrument_name(instrument)
                    ))
                    .strong()
                    .color(GOLD),
                );
                ui.separator();
                let inst = &params.instruments[instrument];
                param_label(ui, "VOL");
                param_compact(ui, setter, &inst.volume_db, -60.0..=12.0, " dB");
                param_label(ui, "PAN");
                let mut pan = inst.pan.value();
                if ui
                    .add(egui::Slider::new(&mut pan, -1.0..=1.0).show_value(false))
                    .on_hover_text("Pan")
                    .changed()
                {
                    set_param(setter, &inst.pan, pan);
                }
                if pan.abs() > 0.005 {
                    ui.label(RichText::new(pan_text(pan)).monospace().small().color(BONE));
                }
                if ui
                    .add_sized([22.0, 18.0], egui::Button::new("C").small())
                    .on_hover_text("Center pan")
                    .clicked()
                {
                    set_param(setter, &inst.pan, 0.0);
                }
                param_label(ui, "TUNE");
                param_compact(ui, setter, &inst.pitch, -24.0..=24.0, " st");
                param_label(ui, "DECAY");
                param_compact(ui, setter, &inst.decay, 0.1..=4.0, "x");
                param_label(ui, "BODY");
                param_compact(ui, setter, &inst.body, 0.0..=1.0, "");
                param_label(ui, "DRIVE");
                param_compact(ui, setter, &inst.drive, 0.0..=1.0, "");
                param_label(ui, "SEND");
                param_compact(ui, setter, &inst.reverb_send, 0.0..=1.0, "");
                let mut mute = inst.mute.value();
                if ui.checkbox(&mut mute, "MUTE").changed() {
                    set_param(setter, &inst.mute, mute);
                }
                let mut solo = inst.solo.value();
                if ui.checkbox(&mut solo, "SOLO").changed() {
                    set_param(setter, &inst.solo, solo);
                }
            });
        });
}

fn param_label(ui: &mut egui::Ui, label: &str) {
    ui.label(RichText::new(label).small().color(MUTED));
}

fn knob(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    param: &FloatParam,
    label: &str,
    range: std::ops::RangeInclusive<f32>,
    suffix: &str,
    decimals: usize,
) {
    let mut value = param.value();
    let min = *range.start();
    let max = *range.end();
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(88.0, 104.0), egui::Sense::click_and_drag());
    if response.dragged() {
        let delta = ui.input(|input| input.pointer.delta().y);
        let span = (max - min).max(0.0001);
        value = (value - delta * span / 180.0).clamp(min, max);
        set_param(setter, param, value);
    }
    if response.hovered() {
        let scroll = ui.input(|input| input.raw_scroll_delta.y);
        if scroll.abs() > 0.01 {
            let span = (max - min).max(0.0001);
            value = (value + scroll * span / 1200.0).clamp(min, max);
            set_param(setter, param, value);
        }
    }

    let center = egui::pos2(rect.center().x, rect.top() + 41.0);
    let radius = 25.0;
    let start = std::f32::consts::PI * 0.75;
    let sweep = std::f32::consts::PI * 1.5;
    let t = ((value - min) / (max - min).max(0.0001)).clamp(0.0, 1.0);
    let painter = ui.painter_at(rect);
    painter.circle_filled(center, radius + 5.0, Color32::from_rgb(18, 18, 20));
    painter.circle_filled(center, radius, Color32::from_rgb(30, 30, 34));
    painter.circle_stroke(
        center,
        radius + 5.0,
        Stroke::new(1.0_f32, Color32::from_rgb(55, 55, 60)),
    );
    for step in 0..=12 {
        let angle = start + sweep * step as f32 / 12.0;
        let inner = center + Vec2::new(angle.cos(), angle.sin()) * (radius + 1.0);
        let outer = center + Vec2::new(angle.cos(), angle.sin()) * (radius + 4.0);
        painter.line_segment(
            [inner, outer],
            Stroke::new(1.0_f32, if step as f32 / 12.0 <= t { GOLD } else { MUTED }),
        );
    }
    let mut points = Vec::new();
    for step in 0..=24 {
        let angle = start + sweep * step as f32 / 24.0;
        if step as f32 / 24.0 <= t {
            points.push(center + Vec2::new(angle.cos(), angle.sin()) * (radius + 1.0));
        }
    }
    if points.len() > 1 {
        for window in points.windows(2) {
            painter.line_segment([window[0], window[1]], Stroke::new(2.0_f32, ACCENT));
        }
    }
    let pointer =
        center + Vec2::new((start + sweep * t).cos(), (start + sweep * t).sin()) * (radius - 4.0);
    painter.line_segment([center, pointer], Stroke::new(2.0_f32, BONE));
    painter.text(
        egui::pos2(center.x, rect.top() + 7.0),
        Align2::CENTER_TOP,
        label,
        FontId::monospace(9.0),
        MUTED,
    );
    painter.text(
        egui::pos2(center.x, rect.bottom() - 2.0),
        Align2::CENTER_BOTTOM,
        format!("{value:.decimals$}{suffix}"),
        FontId::monospace(10.0),
        BONE,
    );
    response.on_hover_text(format!("{label}: drag or scroll to adjust"));
}

fn param_compact(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    param: &FloatParam,
    range: std::ops::RangeInclusive<f32>,
    suffix: &str,
) {
    let mut value = param.value();
    if ui
        .add(
            egui::DragValue::new(&mut value)
                .range(range)
                .speed(0.01)
                .suffix(suffix),
        )
        .changed()
    {
        set_param(setter, param, value);
    }
}

fn set_param<P: Param>(setter: &ParamSetter, param: &P, value: P::Plain) {
    setter.begin_set_parameter(param);
    setter.set_parameter(param, value);
    setter.end_set_parameter(param);
}

fn trigger(queue: &ArrayQueue<EngineCommand>, instrument: usize, velocity: f32) {
    let note = nirudrums_core::DEFAULT_NOTES
        .get(instrument)
        .copied()
        .unwrap_or_else(|| 36 + instrument as u8);
    let _ = queue.push(EngineCommand::Trigger { note, velocity });
}

fn instrument_name(instrument: usize) -> &'static str {
    match instrument {
        0 => "KICK",
        1 => "SNARE",
        2 => "CLOSED HAT",
        3 => "OPEN HAT",
        4 => "CRASH 1",
        5 => "CRASH 2",
        6 => "RIDE",
        7 => "HIGH TOM",
        8 => "MID TOM",
        9 => "LOW TOM",
        10 => "CLAP",
        11 => "RIMSHOT",
        _ => "PERC",
    }
}

fn pattern_name(project: &Project, index: usize) -> String {
    project
        .patterns
        .get(index)
        .map(|pattern| format!("{:02} {}", index + 1, pattern.name))
        .unwrap_or_else(|| "Pattern".into())
}

fn apply_project_to_params(project: &Project, params: &NirudrumsParams, setter: &ParamSetter) {
    for (index, settings) in project.instruments.iter().enumerate() {
        let inst = &params.instruments[index];
        set_param(setter, &inst.volume_db, settings.volume_db);
        set_param(setter, &inst.pan, settings.pan);
        set_param(setter, &inst.pitch, settings.pitch);
        set_param(setter, &inst.decay, settings.decay);
        set_param(setter, &inst.body, settings.body);
        set_param(setter, &inst.velocity, settings.velocity);
        set_param(setter, &inst.drive, settings.drive);
        set_param(setter, &inst.reverb_send, settings.reverb_send);
        set_param(setter, &inst.mute, settings.mute);
        set_param(setter, &inst.solo, settings.solo);
    }
    set_param(setter, &params.master.volume_db, project.master.volume_db);
    set_param(setter, &params.master.drive, project.master.drive);
    set_param(setter, &params.master.limiter, project.master.limiter);
    set_param(setter, &params.master.dry, project.master.dry);
    set_param(
        setter,
        &params.reverb.mode,
        match project.reverb.mode {
            ReverbMode::Plate => ReverbModeParam::Plate,
            ReverbMode::Room => ReverbModeParam::Room,
            ReverbMode::Gated => ReverbModeParam::Gated,
        },
    );
    set_param(setter, &params.reverb.mix, project.reverb.mix);
    set_param(setter, &params.reverb.decay, project.reverb.decay);
    set_param(
        setter,
        &params.reverb.pre_delay_ms,
        project.reverb.pre_delay_ms,
    );
    set_param(setter, &params.reverb.damping, project.reverb.damping);
    set_param(setter, &params.reverb.width, project.reverb.width);
    set_param(setter, &params.reverb.gate_ms, project.reverb.gate_ms);
    set_param(setter, &params.sequencer.bpm, project.bpm);
    set_param(setter, &params.sequencer.swing, project.swing);
    set_param(
        setter,
        &params.sequencer.mode,
        match project.sequencer_mode {
            SequencerMode::Internal => SequencerModeParam::Internal,
            SequencerMode::Midi => SequencerModeParam::Midi,
        },
    );
    set_param(setter, &params.sequencer.follow_host, project.follow_host);
    set_param(setter, &params.sequencer.song_mode, project.song_mode);
}

fn sync_project_from_params(project: &mut Project, params: &NirudrumsParams) {
    for (index, settings) in project.instruments.iter_mut().enumerate() {
        *settings = params.instruments[index].settings();
    }
    project.master = params.master.settings();
    project.reverb = params.reverb.settings();
    project.bpm = params.sequencer.bpm.value();
    project.swing = params.sequencer.swing.value();
    project.song_mode = params.sequencer.song_mode.value();
    project.follow_host = params.sequencer.follow_host.value();
    project.sequencer_mode = match params.sequencer.mode.value() {
        SequencerModeParam::Internal => SequencerMode::Internal,
        SequencerModeParam::Midi => SequencerMode::Midi,
    };
}
