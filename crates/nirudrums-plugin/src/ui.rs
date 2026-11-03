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
    undo: Vec<Project>,
    redo: Vec<Project>,
    clipboard: Option<nirudrums_core::Pattern>,
    project_path: String,
    preset_path: String,
    kit_path: String,
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
            undo: Vec::new(),
            redo: Vec::new(),
            clipboard: None,
            project_path: base.join("mercy-80-session.json").display().to_string(),
            preset_path: base.join("presets/user-preset.json").display().to_string(),
            kit_path: base.join("kits").display().to_string(),
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
            nav_bar(ctx, state, &current_kit);
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
                    Tab::Mixer => mixer_tab(ui, &params, setter, &meters),
                    Tab::Song => song_tab(ui, project, &params, setter, state),
                    Tab::Fx => fx_tab(ui, &params, setter),
                    Tab::Settings => {
                        settings_tab(ui, project, &params, setter, state, &current_kit)
                    }
                });

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
    if !ctx.wants_keyboard_input() {
        let modifier = ctx.input(|input| input.modifiers.command);
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
                    .clicked()
                {
                    set_param(setter, &params.sequencer.playing, !playing);
                }
                if ui.button("HOME").clicked() {
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
                    if ui.button("REDO").clicked() {
                        if let Some(next) = state.redo.pop() {
                            state.undo.push(project.clone());
                            *project = next;
                            state.status = "Redo.".into();
                        }
                    }
                    if ui.button("UNDO").clicked() {
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

fn nav_bar(ctx: &egui::Context, state: &mut UiState, kit: &Kit) {
    egui::SidePanel::left("nav")
        .resizable(false)
        .exact_width(154.0)
        .frame(
            Frame::new()
                .fill(PANEL)
                .inner_margin(egui::Margin::symmetric(10, 12)),
        )
        .show(ctx, |ui| {
            let tabs = [
                (Tab::Pattern, "PATTERN"),
                (Tab::Kit, "KIT"),
                (Tab::Mixer, "MIXER"),
                (Tab::Song, "SONG"),
                (Tab::Fx, "FX"),
                (Tab::Settings, "SETTINGS"),
            ];
            for (tab, label) in tabs {
                let response = ui.add_sized(
                    [ui.available_width(), 32.0],
                    egui::Button::new(RichText::new(label).strong()).selected(state.tab == tab),
                );
                if response.clicked() {
                    state.tab = tab;
                }
            }
            ui.add_space(18.0);
            ui.label(RichText::new("KIT").small().color(MUTED));
            ui.label(RichText::new(&kit.name).strong().color(GOLD));
            ui.label(RichText::new(&kit.id).monospace().small().color(MUTED));
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
                    RichText::new("SPACE play  |  1-8 pattern  |  CTRL+Z undo")
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
        if ui.button("NEW").clicked() {
            state.snapshot(project);
            project.add_pattern();
        }
        if ui.button("DUPLICATE").clicked() {
            state.snapshot(project);
            project.duplicate_pattern();
        }
        if ui.button("CLEAR").clicked() {
            state.snapshot(project);
            project.clear_pattern();
        }
        if ui.button("COPY").clicked() {
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
        if ui.button("ADD TO SONG").clicked() {
            project.add_song_slot(pattern_index as u8);
            state.status = "Pattern added to song.".into();
        }
    });
    ui.separator();

    let active_step = current_step.load(Ordering::Relaxed) as usize;
    let playing = params.sequencer.playing.value();
    egui::ScrollArea::both()
        .id_salt("pattern-grid-scroll")
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
                            let response = ui
                                .add_enabled(
                                    in_pattern,
                                    egui::Button::new(text)
                                        .fill(fill)
                                        .min_size(Vec2::new(23.0, 23.0)),
                                )
                                .on_hover_text("Click to toggle. Right-click to raise velocity.");
                            if response.clicked() {
                                state.snapshot(project);
                                if let Some(pattern) = project.patterns.get_mut(pattern_index) {
                                    pattern.toggle(instrument, step);
                                }
                            }
                            if response.secondary_clicked() {
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
            let response = ui.add_sized(
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
            );
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
        pattern_index,
        state.selected_instrument,
        active_step,
    );
}

fn velocity_lane(
    ui: &mut egui::Ui,
    project: &Project,
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
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
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
    let pattern = project.patterns.get(pattern_index);
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
    for step in 0..64 {
        let velocity = pattern
            .map(|pattern| pattern.velocity(instrument, step))
            .unwrap_or(0);
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

fn mixer_tab(ui: &mut egui::Ui, params: &NirudrumsParams, setter: &ParamSetter, meters: &Meters) {
    ui.label(RichText::new("MIXER").heading().color(BONE));
    ui.label(
        RichText::new("Twelve drum channels, stereo bus, reverb send and master limiter.")
            .small()
            .color(MUTED),
    );
    ui.separator();
    egui::ScrollArea::horizontal().show(ui, |ui| {
        ui.horizontal_top(|ui| {
            for instrument in 0..NUM_INSTRUMENTS {
                ui.vertical(|ui| {
                    ui.set_width(86.0);
                    ui.label(
                        RichText::new(instrument_name(instrument))
                            .small()
                            .strong()
                            .color(if instrument == 0 { GOLD } else { BONE }),
                    );
                    let inst = &params.instruments[instrument];
                    let mut volume = inst.volume_db.value();
                    if ui
                        .add(
                            egui::Slider::new(&mut volume, -60.0..=12.0)
                                .vertical()
                                .show_value(false),
                        )
                        .changed()
                    {
                        set_param(setter, &inst.volume_db, volume);
                    }
                    ui.label(
                        RichText::new(format!("{volume:>6.1}"))
                            .monospace()
                            .small()
                            .color(MUTED),
                    );
                    let mut pan = inst.pan.value();
                    if ui
                        .add(egui::Slider::new(&mut pan, -1.0..=1.0).show_value(false))
                        .changed()
                    {
                        set_param(setter, &inst.pan, pan);
                    }
                    ui.horizontal(|ui| {
                        let mut mute = inst.mute.value();
                        if ui.checkbox(&mut mute, "M").changed() {
                            set_param(setter, &inst.mute, mute);
                        }
                        let mut solo = inst.solo.value();
                        if ui.checkbox(&mut solo, "S").changed() {
                            set_param(setter, &inst.solo, solo);
                        }
                    });
                    let mut send = inst.reverb_send.value();
                    if ui
                        .add(egui::Slider::new(&mut send, 0.0..=1.0).show_value(false))
                        .changed()
                    {
                        set_param(setter, &inst.reverb_send, send);
                    }
                    ui.label(RichText::new("SEND").small().color(MUTED));
                    let mut body = inst.body.value();
                    if ui
                        .add(egui::Slider::new(&mut body, 0.0..=1.0).show_value(false))
                        .changed()
                    {
                        set_param(setter, &inst.body, body);
                    }
                    ui.label(RichText::new("BODY").small().color(MUTED));
                });
            }
            ui.separator();
            ui.vertical(|ui| {
                ui.set_width(126.0);
                ui.label(RichText::new("MASTER").strong().color(GOLD));
                let (left, right) = meters.peak();
                ui.add(
                    egui::ProgressBar::new(left.clamp(0.0, 1.0))
                        .fill(GOLD)
                        .text(RichText::new("L").small()),
                );
                ui.add(
                    egui::ProgressBar::new(right.clamp(0.0, 1.0))
                        .fill(GOLD)
                        .text(RichText::new("R").small()),
                );
                let mut master = params.master.volume_db.value();
                if ui
                    .add(egui::Slider::new(&mut master, -60.0..=12.0).text("VOL"))
                    .changed()
                {
                    set_param(setter, &params.master.volume_db, master);
                }
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

fn song_tab(
    ui: &mut egui::Ui,
    project: &mut Project,
    params: &NirudrumsParams,
    setter: &ParamSetter,
    state: &mut UiState,
) {
    ui.label(RichText::new("SONG").heading().color(BONE));
    ui.label(
        RichText::new("Arrange pattern slots for a full performance. Repeats are pattern lengths.")
            .small()
            .color(MUTED),
    );
    ui.separator();
    let mut song_mode = params.sequencer.song_mode.value();
    if ui.checkbox(&mut song_mode, "SONG MODE").changed() {
        set_param(setter, &params.sequencer.song_mode, song_mode);
        project.song_mode = song_mode;
    }
    let mut follow = params.sequencer.follow_host.value();
    if ui.checkbox(&mut follow, "FOLLOW HOST TRANSPORT").changed() {
        set_param(setter, &params.sequencer.follow_host, follow);
        project.follow_host = follow;
    }
    ui.horizontal(|ui| {
        ui.label("CLOCK");
        let mut clock = params.sequencer.mode.value();
        egui::ComboBox::from_id_salt("clock-mode")
            .selected_text(match clock {
                SequencerModeParam::Internal => "Internal",
                SequencerModeParam::Midi => "MIDI",
            })
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
    });
    if ui.button("ADD CURRENT PATTERN").clicked() {
        project.add_song_slot(project.current_pattern as u8);
        state.status = "Song slot added.".into();
    }
    ui.add_space(8.0);
    egui::ScrollArea::vertical().show(ui, |ui| {
        if project.song.is_empty() {
            ui.label(RichText::new("No song slots yet.").color(MUTED));
        }
        let pattern_names: Vec<String> = project
            .patterns
            .iter()
            .enumerate()
            .map(|(index, pattern)| format!("{:02} {}", index + 1, pattern.name))
            .collect();
        let mut remove = None;
        for (index, slot) in project.song.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{:02}", index + 1))
                        .monospace()
                        .color(GOLD),
                );
                egui::ComboBox::from_id_salt(("song-pattern", index))
                    .selected_text(
                        pattern_names
                            .get(usize::from(slot.pattern))
                            .cloned()
                            .unwrap_or_default(),
                    )
                    .width(180.0)
                    .show_ui(ui, |ui| {
                        for (candidate, name) in pattern_names.iter().enumerate() {
                            ui.selectable_value(&mut slot.pattern, candidate as u8, name.clone());
                        }
                    });
                ui.label(RichText::new("x").small().color(MUTED));
                let mut repeats = i32::from(slot.repeats);
                if ui
                    .add(egui::DragValue::new(&mut repeats).range(1..=32))
                    .changed()
                {
                    slot.repeats = repeats.clamp(1, 32) as u8;
                }
                if ui.button("REMOVE").clicked() {
                    remove = Some(index);
                }
            });
        }
        if let Some(index) = remove {
            project.remove_song_slot(index);
            state.status = "Song slot removed.".into();
        }
        if project.song.len() >= MAX_SONG_SLOTS {
            ui.label(
                RichText::new("Song slot limit reached.")
                    .small()
                    .color(ACCENT),
            );
        }
    });
}

fn fx_tab(ui: &mut egui::Ui, params: &NirudrumsParams, setter: &ParamSetter) {
    ui.label(RichText::new("FX").heading().color(BONE));
    ui.separator();
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new("REVERB").strong().color(GOLD));
            let mut mode = params.reverb.mode.value();
            egui::ComboBox::from_id_salt("reverb-mode")
                .selected_text(match mode {
                    ReverbModeParam::Plate => "Plate",
                    ReverbModeParam::Room => "Room",
                    ReverbModeParam::Gated => "Gated",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut mode, ReverbModeParam::Plate, "Plate");
                    ui.selectable_value(&mut mode, ReverbModeParam::Room, "Room");
                    ui.selectable_value(&mut mode, ReverbModeParam::Gated, "Gated");
                });
            if mode != params.reverb.mode.value() {
                set_param(setter, &params.reverb.mode, mode);
            }
            param_slider(ui, setter, &params.reverb.mix, "MIX", 0.0..=1.0);
            param_slider(ui, setter, &params.reverb.decay, "DECAY", 0.0..=1.0);
            param_slider(
                ui,
                setter,
                &params.reverb.pre_delay_ms,
                "PRE-DELAY",
                0.0..=200.0,
            );
            param_slider(ui, setter, &params.reverb.damping, "DAMPING", 0.0..=1.0);
            param_slider(ui, setter, &params.reverb.width, "WIDTH", 0.0..=1.0);
            param_slider(ui, setter, &params.reverb.gate_ms, "GATE", 0.0..=2000.0);
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.label(RichText::new("MASTER BUS").strong().color(GOLD));
            param_slider(
                ui,
                setter,
                &params.master.volume_db,
                "MASTER VOLUME",
                -60.0..=12.0,
            );
            param_slider(ui, setter, &params.master.drive, "DRIVE", 0.0..=1.0);
            let mut limiter = params.master.limiter.value();
            if ui.checkbox(&mut limiter, "LIMITER").changed() {
                set_param(setter, &params.master.limiter, limiter);
            }
            let mut dry = params.master.dry.value();
            if ui.checkbox(&mut dry, "DRY BYPASS").changed() {
                set_param(setter, &params.master.dry, dry);
            }
            ui.add_space(12.0);
            ui.label(
                RichText::new(
                    "Plate: bright, dense. Room: short and open. Gated: 1970s hard stop.",
                )
                .small()
                .color(MUTED),
            );
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
    ui.separator();
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.label(RichText::new("KIT").strong().color(GOLD));
        ui.label(format!("{} / {} / {}", kit.name, kit.id, kit.version));
        ui.label(
            RichText::new(&kit.description)
                .small()
                .color(MUTED),
        );
        ui.add_space(10.0);

        ui.label(RichText::new("MIDI MAP").strong().color(GOLD));
        egui::Grid::new("midi-map")
            .num_columns(4)
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
        ui.add_space(10.0);

        ui.label(RichText::new("PRESETS").strong().color(GOLD));
        ui.horizontal_wrapped(|ui| {
            for preset in factory_presets() {
                let name = preset.name.clone();
                if ui.button(name).clicked() {
                    preset.apply(project);
                    apply_project_to_params(project, params, setter);
                    state.status = format!("Preset loaded: {}", preset.id);
                }
            }
        });
        ui.add_space(10.0);

        ui.label(RichText::new("USER PRESET").strong().color(GOLD));
        ui.horizontal(|ui| {
            ui.label("Path");
            ui.add(egui::TextEdit::singleline(&mut state.preset_path).desired_width(460.0));
        });
        ui.horizontal(|ui| {
            if ui.button("SAVE PRESET").clicked() {
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
            if ui.button("LOAD PRESET").clicked() {
                let path = PathBuf::from(&state.preset_path);
                match nirudrums_core::Preset::load(&path) {
                    Ok(preset) => {
                        preset.apply(project);
                        apply_project_to_params(project, params, setter);
                        state.status = format!("Preset loaded {}", path.display());
                    }
                    Err(error) => state.status = error,
                }
            }
        });
        ui.add_space(10.0);

        ui.label(RichText::new("SESSION").strong().color(GOLD));
        ui.horizontal(|ui| {
            ui.label("Path");
            ui.add(egui::TextEdit::singleline(&mut state.project_path).desired_width(460.0));
        });
        ui.horizontal(|ui| {
            if ui.button("SAVE SESSION").clicked() {
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
            if ui.button("LOAD SESSION").clicked() {
                let path = PathBuf::from(&state.project_path);
                match Project::load(&path) {
                    Ok(loaded) => {
                        *project = loaded;
                        apply_project_to_params(project, params, setter);
                        state.status = format!("Loaded {}", path.display());
                    }
                    Err(error) => state.status = error,
                }
            }
        });
        ui.add_space(10.0);

        ui.label(RichText::new("WAV EXPORT").strong().color(GOLD));
        ui.horizontal(|ui| {
            ui.label("Output");
            ui.add(egui::TextEdit::singleline(&mut state.export_path).desired_width(460.0));
        });
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
        ui.add_space(12.0);
        ui.label(RichText::new("NIRUDRUMS").strong().color(BONE));
        ui.label(
            RichText::new(
                "NIRUDRUMS 0.0.1 — concept, design, and creative direction by Nicklas Rudolfsson.",
            )
            .small()
            .color(GOLD),
        );
        ui.label(
            RichText::new(
                "MERCY-80 includes CC0-licensed one-shot source material. Seeproject kit source notes for provenance.",
            )
            .small()
            .color(MUTED),
        );
    });
}

#[allow(clippy::too_many_arguments)]
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
                param_compact(ui, setter, &inst.pan, -1.0..=1.0, "");
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

fn param_slider(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    param: &FloatParam,
    label: &str,
    range: std::ops::RangeInclusive<f32>,
) -> bool {
    let mut value = param.value();
    let response = ui.add(egui::Slider::new(&mut value, range).text(label));
    if response.changed() {
        set_param(setter, param, value);
    }
    response.changed()
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
