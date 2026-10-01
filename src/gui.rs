use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use walkdir::WalkDir;

use eframe::egui::{self, Color32, Margin, RichText, ScrollArea, Stroke, Vec2};

use crate::drives::{get_available_drives, DriveInfo, FormatFileSystem};
use crate::favorites::{load_favorites, save_favorites, FavoritesList};
use crate::installer::{
    CopyMode, InstallConfig, InstallSummary, InstallerEngine, InstallerEvent,
    PlatformInstallConfig,
};
use crate::launcher_profiles::{LauncherProfile, ProfileId, PROFILES};
use crate::platforms::{find_platform_by_dir_name, PlatformInfo, PLATFORMS};

#[derive(Clone)]
pub struct PlatformUiState {
    pub platform: PlatformInfo,
    pub enabled: bool,
    pub found_dir: Option<PathBuf>,
    pub rom_files: Vec<String>,
    pub mode: CopyMode,
    pub favorites: Option<FavoritesList>,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum ActiveTab {
    Dashboard,
    Consoles,
    Favorites,
    Settings,
    SdCard,
    Profile,
    Platforms,
    Artwork,
    Install,
}

// ----------------------------------------------------------------------------
// Theme Helpers & Cards
// ----------------------------------------------------------------------------

fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::from_rgb(13, 19, 28))
        .stroke(Stroke::new(1.0, Color32::from_rgb(26, 37, 54)))
        .corner_radius(8)
        .inner_margin(Margin::same(12))
}

fn card_frame_selected() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::from_rgb(17, 28, 42))
        .stroke(Stroke::new(1.2, Color32::from_rgb(0, 229, 255)))
        .corner_radius(8)
        .inner_margin(Margin::same(12))
}

#[allow(dead_code)]
fn card_frame_protected() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::from_rgb(31, 17, 21))
        .stroke(Stroke::new(1.2, Color32::from_rgb(239, 68, 68)))
        .corner_radius(8)
        .inner_margin(Margin::same(12))
}

fn render_badge(ui: &mut egui::Ui, text: &str, bg: Color32, border: Color32, fg: Color32) {
    egui::Frame::new()
        .fill(bg)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(12)
        .inner_margin(Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(10.0).color(fg).strong());
        });
}

fn badge_cyan(ui: &mut egui::Ui, text: &str) {
    render_badge(
        ui,
        text,
        Color32::from_rgb(0, 36, 48),
        Color32::from_rgb(0, 180, 210),
        Color32::from_rgb(0, 229, 255),
    );
}

fn badge_green(ui: &mut egui::Ui, text: &str) {
    render_badge(
        ui,
        text,
        Color32::from_rgb(10, 34, 22),
        Color32::from_rgb(16, 185, 129),
        Color32::from_rgb(16, 185, 129),
    );
}

fn badge_purple(ui: &mut egui::Ui, text: &str) {
    render_badge(
        ui,
        text,
        Color32::from_rgb(28, 20, 48),
        Color32::from_rgb(139, 92, 246),
        Color32::from_rgb(168, 130, 255),
    );
}

fn badge_amber(ui: &mut egui::Ui, text: &str) {
    render_badge(
        ui,
        text,
        Color32::from_rgb(42, 28, 10),
        Color32::from_rgb(245, 158, 11),
        Color32::from_rgb(245, 158, 11),
    );
}

fn badge_red(ui: &mut egui::Ui, text: &str) {
    render_badge(
        ui,
        text,
        Color32::from_rgb(40, 15, 18),
        Color32::from_rgb(239, 68, 68),
        Color32::from_rgb(239, 68, 68),
    );
}

fn render_card_header(ui: &mut egui::Ui, title: &str, badge_fn: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(12.5).strong().color(Color32::WHITE));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            badge_fn(ui);
        });
    });
    ui.add_space(2.0);
    ui.separator();
    ui.add_space(6.0);
}

// Custom iOS/Windows 11 styled Toggle Switch with Neon Cyan accent
pub fn toggle_switch(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let desired_size = Vec2::new(34.0, 18.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, ""));

    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool_responsive(response.id, *on);
        let corner_radius = rect.height() / 2.0;
        let bg_color = if *on {
            Color32::from_rgb(0, 229, 255) // Neon cyan
        } else {
            Color32::from_rgb(28, 38, 54) // Dark slate
        };
        ui.painter().rect_filled(rect, corner_radius, bg_color);

        let circle_x = egui::lerp((rect.left() + corner_radius)..=(rect.right() - corner_radius), how_on);
        let center = egui::pos2(circle_x, rect.center().y);
        let knob_color = if *on {
            Color32::from_rgb(10, 14, 20)
        } else {
            Color32::from_rgb(140, 160, 185)
        };
        ui.painter().circle_filled(center, corner_radius - 2.0, knob_color);
    }
    response
}

// Glowing Sleek Neon Cyan Progress Bar
fn render_neon_progress_bar(ui: &mut egui::Ui, label: &str, fraction: f32, extra_text: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(10.5).strong().color(Color32::from_gray(200)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(extra_text).size(10.5).strong().color(Color32::from_rgb(0, 229, 255)));
        });
    });
    ui.add_space(2.0);
    let height = 12.0;
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let radius = height / 2.0;
        // Background track
        ui.painter().rect_filled(rect, radius, Color32::from_rgb(16, 24, 36));
        // Fill
        let clamped = fraction.clamp(0.0, 1.0);
        if clamped > 0.001 {
            let fill_width = (rect.width() * clamped).max(height);
            let fill_rect = egui::Rect::from_min_size(rect.min, Vec2::new(fill_width, height));
            ui.painter().rect_filled(fill_rect, radius, Color32::from_rgb(0, 229, 255));
        }
    }
    ui.add_space(5.0);
}

// Glowing Curated Favorites Pill Button on Console rows
fn curated_favorites_button(ui: &mut egui::Ui) -> bool {
    let frame = egui::Frame::new()
        .fill(Color32::from_rgb(0, 34, 46))
        .stroke(Stroke::new(1.0, Color32::from_rgb(0, 229, 255)))
        .corner_radius(12)
        .inner_margin(Margin::symmetric(8, 3));

    let inner = frame.show(ui, |ui| {
        ui.label(
            RichText::new("⭐ CURATED FAVORITES")
                .size(9.5)
                .strong()
                .color(Color32::from_rgb(0, 229, 255)),
        );
    });

    let id = inner.response.id.with("curate_favs_btn");
    ui.interact(inner.response.rect, id, egui::Sense::click()).clicked()
}

fn get_game_icon(rom_name: &str) -> &'static str {
    let lower = rom_name.to_lowercase();
    if lower.contains("pokemon") {
        "⚡"
    } else if lower.contains("mario kart") || lower.contains("racing") || lower.contains("need for speed") {
        "🏎️"
    } else if lower.contains("mario") {
        "🍄"
    } else if lower.contains("zelda") {
        "🛡️"
    } else if lower.contains("castlevania") {
        "🦇"
    } else if lower.contains("dragon quest") || lower.contains("final fantasy") {
        "🗡️"
    } else if lower.contains("metroid") || lower.contains("star") {
        "🚀"
    } else if lower.contains("sonic") {
        "🦔"
    } else if lower.contains("kirby") {
        "⭐"
    } else {
        "🎮"
    }
}

// ----------------------------------------------------------------------------
// Application State
// ----------------------------------------------------------------------------

pub struct RetroCardMakerApp {
    // Navigation & Layout
    pub active_tab: ActiveTab,
    pub wizard_focused_step: usize,

    // Step 1: SD Card & Format
    pub drives: Vec<DriveInfo>,
    pub selected_drive_idx: usize,
    pub subfolder_name: String,
    pub do_format: bool,
    pub format_fs: FormatFileSystem,
    pub volume_label: String,
    pub format_confirmed: bool,
    pub wipe_and_repartition: bool,

    // Step 2: Launcher & Device Profile
    pub selected_profile_id: ProfileId,

    // Step 3: Source, Platforms & Selection
    pub source_path: String,
    pub platform_states: Vec<PlatformUiState>,
    pub selected_platform_idx: usize,
    pub rom_search_query: String,

    // Step 4: Artwork
    pub download_art: bool,

    // Step 5: Installer Execution & Progress
    pub is_running: bool,
    pub current_phase: String,
    pub progress_current: usize,
    pub progress_total: usize,
    pub current_item: String,
    pub logs: Vec<String>,
    pub install_summary: Option<InstallSummary>,
    pub install_error: Option<String>,
    pub cancel_flag: Arc<AtomicBool>,
    pub event_rx: Option<Receiver<InstallerEvent>>,

    // Modal & Full-Screen: Favorites Editor
    pub editing_platform_idx: Option<usize>,
    pub editing_selected_games: HashSet<String>,
    pub editing_custom_patterns: Vec<String>,
    pub editing_search_query: String,
    pub editing_show_selected_only: bool,
    pub new_favorite_pattern: String,
    pub favorites_save_notification: Option<String>,
    pub favorites_preview_game: Option<String>,
}

impl RetroCardMakerApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let drives = get_available_drives();
        let default_profile = ProfileId::AnbernicRgDsLauncher;
        let profile = LauncherProfile::get_by_id(default_profile);

        let mut app = Self {
            active_tab: ActiveTab::Dashboard,
            wizard_focused_step: 1,

            drives,
            selected_drive_idx: 0,
            subfolder_name: profile.recommended_sd_subfolder.to_string(),
            do_format: false,
            format_fs: FormatFileSystem::ExFat,
            volume_label: "RETRO".to_string(),
            format_confirmed: false,
            wipe_and_repartition: false,

            selected_profile_id: default_profile,

            source_path: String::new(),
            platform_states: PLATFORMS
                .iter()
                .map(|p| PlatformUiState {
                    platform: p.clone(),
                    enabled: true,
                    found_dir: None,
                    rom_files: Vec::new(),
                    mode: CopyMode::AllRoms,
                    favorites: None,
                })
                .collect(),
            selected_platform_idx: 0,
            rom_search_query: String::new(),

            download_art: true,

            is_running: false,
            current_phase: "Ready".to_string(),
            progress_current: 0,
            progress_total: 0,
            current_item: String::new(),
            logs: vec![
                "[INFO] Initializing Retro CardMaker v0.2.1 Core Engine...".to_string(),
                "[INFO] Hardware drive scanner online. Removable protection active.".to_string(),
            ],
            install_summary: None,
            install_error: None,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            event_rx: None,

            editing_platform_idx: None,
            editing_selected_games: HashSet::new(),
            editing_custom_patterns: Vec::new(),
            editing_search_query: String::new(),
            editing_show_selected_only: false,
            new_favorite_pattern: String::new(),
            favorites_save_notification: None,
            favorites_preview_game: None,
        };

        // Apply Refined Compact Desktop AAA Dark Theme
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(9, 12, 16);
        visuals.window_fill = Color32::from_rgb(13, 18, 25);
        visuals.window_stroke = Stroke::new(1.0, Color32::from_rgb(45, 58, 78));
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(15, 20, 28);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(28, 36, 50));
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(18, 24, 34);
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(32, 42, 58));
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(25, 34, 48);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0, 229, 255));
        visuals.widgets.active.bg_fill = Color32::from_rgb(30, 42, 60);
        visuals.selection.bg_fill = Color32::from_rgb(0, 160, 210);
        _cc.egui_ctx.set_visuals(visuals);

        // Auto-select first removable drive
        if let Some(pos) = app.drives.iter().position(|d| d.is_removable) {
            app.selected_drive_idx = pos;
        }

        // Auto-discover C:\Users\Andreas\OneDrive\Roms if present
        let default_user_path = "C:\\Users\\Andreas\\OneDrive\\Roms";
        if Path::new(default_user_path).exists() {
            app.source_path = default_user_path.to_string();
            app.scan_source_folder();
        }

        // Auto-select first platform that has found ROMs
        if let Some(pos) = app.platform_states.iter().position(|p| !p.rom_files.is_empty()) {
            app.selected_platform_idx = pos;
        }

        app
    }

    pub fn refresh_drives(&mut self) {
        self.drives = get_available_drives();
        if self.selected_drive_idx >= self.drives.len() && !self.drives.is_empty() {
            self.selected_drive_idx = 0;
        }
        self.logs.push(format!("[DRIVE] Scanned {} storage volumes.", self.drives.len()));
    }

    pub fn scan_source_folder(&mut self) {
        let root = Path::new(&self.source_path);
        if !root.exists() || !root.is_dir() {
            return;
        }

        let subdirs: Vec<PathBuf> = match std::fs::read_dir(root) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect(),
            Err(_) => Vec::new(),
        };

        for p_state in &mut self.platform_states {
            p_state.found_dir = None;
            p_state.rom_files.clear();
            p_state.favorites = None;

            let mut matched_candidate = None;
            for subdir in &subdirs {
                if let Some(dir_name) = subdir.file_name().and_then(|s| s.to_str()) {
                    if let Some(matched_p) = find_platform_by_dir_name(dir_name) {
                        if matched_p.id == p_state.platform.id {
                            matched_candidate = Some(subdir.clone());
                            break;
                        }
                    }
                }
            }

            if matched_candidate.is_none() {
                for alias in p_state.platform.folder_aliases {
                    let candidate = root.join(alias);
                    if candidate.is_dir() {
                        matched_candidate = Some(candidate);
                        break;
                    }
                }
            }

            if let Some(candidate) = matched_candidate {
                p_state.found_dir = Some(candidate.clone());
                p_state.favorites = load_favorites(&candidate);

                for entry in WalkDir::new(&candidate)
                    .max_depth(3)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if entry.file_type().is_file() {
                        let path = entry.path();
                        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                            let lower = name.to_lowercase();
                            if p_state
                                .platform
                                .extensions
                                .iter()
                                .any(|&ext| lower.ends_with(ext))
                            {
                                p_state.rom_files.push(name.to_string());
                            }
                        }
                    }
                }
                p_state.rom_files.sort();
                p_state.rom_files.dedup();
            }
        }

        let found_count = self.platform_states.iter().filter(|p| p.found_dir.is_some()).count();
        let total_roms: usize = self.platform_states.iter().map(|p| p.rom_files.len()).sum();
        self.logs.push(format!("[ROMS] Source library scanned: {} systems matched, {} ROMs loaded.", found_count, total_roms));
    }

    pub fn open_favorites_editor(&mut self, p_idx: usize) {
        if p_idx >= self.platform_states.len() {
            return;
        }
        self.editing_platform_idx = Some(p_idx);
        self.editing_selected_games.clear();
        self.editing_custom_patterns.clear();
        self.editing_search_query.clear();
        self.editing_show_selected_only = false;
        self.favorites_save_notification = None;

        let p_state = &self.platform_states[p_idx];
        if let Some(ref favs) = p_state.favorites {
            for g in &favs.games {
                self.editing_selected_games.insert(g.clone());
            }
            for pat in &favs.patterns {
                self.editing_custom_patterns.push(pat.clone());
            }
        } else {
            let default_favs = FavoritesList::from_default_platform(&p_state.platform);
            for g in &default_favs.games {
                self.editing_selected_games.insert(g.clone());
            }
            for pat in &default_favs.patterns {
                self.editing_custom_patterns.push(pat.clone());
            }
        }

        self.favorites_preview_game = p_state.rom_files.first().cloned();
    }

    pub fn start_install(&mut self) {
        if self.drives.is_empty() {
            return;
        }

        let drive = &self.drives[self.selected_drive_idx];
        if self.do_format && drive.is_system {
            self.install_error = Some("Cannot format system drive C:!".to_string());
            return;
        }
        if self.do_format && !drive.is_removable {
            self.install_error = Some("Safety violation: Formatting is strictly restricted to removable USB and SD-card drives.".to_string());
            return;
        }

        let clean_drive = drive.letter.trim_end_matches('\\').trim_end_matches('/');
        let dest_path = if self.subfolder_name.trim().is_empty() {
            PathBuf::from(format!("{}\\", clean_drive))
        } else {
            PathBuf::from(format!("{}\\{}", clean_drive, self.subfolder_name.trim()))
        };

        let mut platform_configs = Vec::new();
        for p in &self.platform_states {
            if p.enabled {
                if let Some(ref dir) = p.found_dir {
                    platform_configs.push(PlatformInstallConfig {
                        platform_id: p.platform.id.to_string(),
                        source_dir: dir.clone(),
                        mode: p.mode,
                        favorites: p.favorites.clone(),
                    });
                }
            }
        }

        if platform_configs.is_empty() {
            self.install_error = Some("No platforms selected or no ROMs found!".to_string());
            return;
        }

        let format_opt = if self.do_format {
            Some(self.format_fs)
        } else {
            None
        };

        let config = InstallConfig {
            drive_letter: clean_drive.to_string(),
            destination_path: dest_path,
            format_option: format_opt,
            wipe_and_repartition: self.wipe_and_repartition && self.do_format,
            volume_label: self.volume_label.clone(),
            profile_id: self.selected_profile_id,
            platforms: platform_configs,
            download_art: self.download_art,
        };

        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = cancel_flag.clone();
        let (tx, rx): (Sender<InstallerEvent>, Receiver<InstallerEvent>) = mpsc::channel();
        self.event_rx = Some(rx);

        self.is_running = true;
        self.install_summary = None;
        self.install_error = None;
        self.logs.clear();
        self.logs.push(format!("[INIT] Starting QuickInstaller for drive {}...", clean_drive));

        std::thread::spawn(move || {
            InstallerEngine::run(config, cancel_flag, tx);
        });
    }

    pub fn poll_events(&mut self) {
        if let Some(ref rx) = self.event_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    InstallerEvent::Phase(phase) => {
                        self.current_phase = phase.clone();
                        self.logs.push(format!("[Phase] {}", phase));
                    }
                    InstallerEvent::Log(msg) => {
                        self.logs.push(msg);
                    }
                    InstallerEvent::Progress {
                        current,
                        total,
                        item,
                    } => {
                        self.progress_current = current;
                        self.progress_total = total;
                        self.current_item = item;
                    }
                    InstallerEvent::Success(summary) => {
                        self.is_running = false;
                        self.install_summary = Some(summary);
                        self.logs.push("=== QuickInstaller Succeeded! ===".to_string());
                    }
                    InstallerEvent::Failed(err) => {
                        self.is_running = false;
                        self.install_error = Some(err.clone());
                        self.logs.push(format!("ERROR: {}", err));
                    }
                }
            }
        }
    }
}

// ----------------------------------------------------------------------------
// eframe App UI implementation
// ----------------------------------------------------------------------------

impl eframe::App for RetroCardMakerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_events();
        if self.is_running {
            ui.ctx().request_repaint();
        }

        // 1. Top Windows 11 App Header with Navigation Tabs (Matching Mockup)
        self.render_top_header(ui);

        // 2. 5-Step Chevron Ribbon (Matching cardmaker_ui_mockup_1790889832082.jpg)
        self.render_step_ribbon(ui);

        ui.add_space(6.0);

        // 3. Main Workspace Area
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                match self.active_tab {
                    ActiveTab::Dashboard => self.render_dashboard_cockpit(ui),
                    ActiveTab::Consoles | ActiveTab::Platforms => self.render_consoles_tab_view(ui),
                    ActiveTab::Favorites => self.render_favorites_tab_view(ui),
                    ActiveTab::Settings => self.render_settings_tab_view(ui),
                    ActiveTab::SdCard => self.render_dashboard_cockpit(ui),
                    ActiveTab::Profile => self.render_dashboard_cockpit(ui),
                    ActiveTab::Artwork => self.render_dashboard_cockpit(ui),
                    ActiveTab::Install => self.render_dashboard_cockpit(ui),
                }
            });

        // 4. Curated Favorites Modal Dialog (Matching cardmaker_favorites_modal_1790889844749.jpg)
        self.render_favorites_modal(ui);
    }
}

// ----------------------------------------------------------------------------
// Top Header & Step Ribbon Renderers
// ----------------------------------------------------------------------------

impl RetroCardMakerApp {
    fn render_top_header(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                // App Logo Icon
                egui::Frame::new()
                    .fill(Color32::from_rgb(0, 160, 210))
                    .corner_radius(6)
                    .inner_margin(Margin::symmetric(6, 4))
                    .show(ui, |ui| {
                        ui.label(RichText::new("🎮").size(14.0).color(Color32::WHITE));
                    });
                ui.add_space(4.0);

                ui.label(
                    RichText::new("Retro CardMaker")
                        .size(15.0)
                        .strong()
                        .color(Color32::WHITE),
                );

                render_badge(
                    ui,
                    "Windows 11",
                    Color32::from_rgb(18, 24, 34),
                    Color32::from_rgb(45, 58, 78),
                    Color32::from_gray(180),
                );

                ui.separator();
                ui.add_space(6.0);

                // Navigation Tabs: [ ⊞ Dashboard ] [ 🖵 Consoles ] [ ⭐ Favorites ] [ ⚙ Settings ]
                let tabs = [
                    (ActiveTab::Dashboard, "⊞ Dashboard"),
                    (ActiveTab::Consoles, "🖵 Consoles"),
                    (ActiveTab::Favorites, "⭐ Favorites"),
                    (ActiveTab::Settings, "⚙ Settings"),
                ];

                for (tab, title) in tabs {
                    let is_active = self.active_tab == tab;
                    let frame = if is_active {
                        egui::Frame::new()
                            .fill(Color32::from_rgb(20, 32, 48))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(0, 229, 255)))
                            .corner_radius(6)
                            .inner_margin(Margin::symmetric(10, 4))
                    } else {
                        egui::Frame::new()
                            .fill(Color32::from_rgb(12, 16, 24))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(26, 36, 50)))
                            .corner_radius(6)
                            .inner_margin(Margin::symmetric(10, 4))
                    };

                    let inner = frame.show(ui, |ui| {
                        let text_color = if is_active {
                            Color32::from_rgb(0, 229, 255)
                        } else {
                            Color32::from_gray(170)
                        };
                        ui.label(RichText::new(title).size(11.5).strong().color(text_color));
                    });

                    let id = inner.response.id.with(format!("top_tab_{:?}", tab));
                    if ui.interact(inner.response.rect, id, egui::Sense::click()).clicked() {
                        self.active_tab = tab;
                    }
                    ui.add_space(2.0);
                }

                // Right-aligned status pill
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let total_favs: usize = self
                        .platform_states
                        .iter()
                        .map(|p| p.favorites.as_ref().map(|f| f.games.len()).unwrap_or(0))
                        .sum();
                    render_badge(
                        ui,
                        &format!("👤 User • {} Favs ⌵", total_favs),
                        Color32::from_rgb(22, 18, 38),
                        Color32::from_rgb(139, 92, 246),
                        Color32::from_rgb(180, 140, 255),
                    );

                    if let Some(drive) = self.drives.get(self.selected_drive_idx) {
                        let rem_tag = if drive.is_removable { "Removable" } else { "Fixed" };
                        badge_cyan(ui, &format!("Target: {} ({} {})", drive.letter, drive.total_gb_str(), rem_tag));
                    }
                });
            });
            ui.add_space(4.0);
            ui.separator();
        });
    }

    fn render_step_ribbon(&mut self, ui: &mut egui::Ui) {
        ui.add_space(2.0);
        ui.columns(5, |cols| {
            let steps = [
                ("1", "❶ SD CARD & FORMAT", ActiveTab::Dashboard),
                ("2", "❷ DEVICE PROFILE", ActiveTab::Dashboard),
                ("3", "❸ ROMS & FAVORITES", ActiveTab::Dashboard),
                ("4", "❹ BOXART SCRAPING", ActiveTab::Dashboard),
                ("5", "❺ QUICKINSTALL", ActiveTab::Dashboard),
            ];

            for (i, (_num, title, target_tab)) in steps.iter().enumerate() {
                let col = &mut cols[i];
                let is_active = self.wizard_focused_step == (i + 1);

                let frame = if is_active {
                    egui::Frame::new()
                        .fill(Color32::from_rgb(0, 229, 255))
                        .corner_radius(12)
                        .inner_margin(Margin::symmetric(10, 6))
                } else {
                    egui::Frame::new()
                        .fill(Color32::from_rgb(14, 20, 30))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(28, 40, 58)))
                        .corner_radius(12)
                        .inner_margin(Margin::symmetric(10, 6))
                };

                let inner = frame.show(col, |ui| {
                    ui.centered_and_justified(|ui| {
                        let text_color = if is_active {
                            Color32::BLACK
                        } else {
                            Color32::from_gray(200)
                        };
                        ui.label(RichText::new(*title).size(10.5).strong().color(text_color));
                    });
                });

                let id = inner.response.id.with(format!("ribbon_step_{}", i));
                if col.interact(inner.response.rect, id, egui::Sense::click()).clicked() {
                    self.wizard_focused_step = i + 1;
                    self.active_tab = *target_tab;
                }
            }
        });
        ui.add_space(4.0);
        ui.separator();
    }
}

// ----------------------------------------------------------------------------
// Master Studio Dashboard Cockpit (3 Tiers, Matching cardmaker_ui_mockup_1790889832082.jpg)
// ----------------------------------------------------------------------------

impl RetroCardMakerApp {
    fn render_dashboard_cockpit(&mut self, ui: &mut egui::Ui) {
        // TIER 1: SD Card & Format (Left) + Device Profile (Right)
        ui.columns(2, |cols| {
            self.render_card_sd_format(&mut cols[0]);
            self.render_card_device_profile(&mut cols[1]);
        });

        ui.add_space(8.0);

        // TIER 2: Consoles Selection (Left) + Live ROMs & Boxart Preview (Right)
        ui.columns(2, |cols| {
            self.render_card_consoles_selection(&mut cols[0]);
            self.render_card_roms_preview(&mut cols[1]);
        });

        ui.add_space(8.0);

        // TIER 3: QuickInstaller Cockpit (Left) + Live Terminal (Right)
        ui.columns(2, |cols| {
            self.render_card_installer_cockpit(&mut cols[0]);
            self.render_card_terminal_console(&mut cols[1]);
        });

        ui.add_space(8.0);
    }

    // ------------------------------------------------------------------------
    // Card 1: 1. SD Card & Format
    // ------------------------------------------------------------------------
    fn render_card_sd_format(&mut self, ui: &mut egui::Ui) {
        card_frame().show(ui, |ui| {
            let selected_drive = self.drives.get(self.selected_drive_idx).cloned();
            let is_removable = selected_drive.as_ref().map(|d| d.is_removable).unwrap_or(false);
            let is_system = selected_drive.as_ref().map(|d| d.is_system).unwrap_or(false);
            let has_hidden = selected_drive.as_ref().map(|d| d.has_hidden_partitions).unwrap_or(false);

            render_card_header(ui, "1. SD Card & Format", |ui| {
                if is_removable {
                    badge_cyan(ui, "💾 CONNECTED");
                } else if is_system {
                    badge_red(ui, "🛡️ SYSTEM DRIVE");
                } else if self.drives.is_empty() {
                    badge_red(ui, "❌ NO DRIVE");
                } else {
                    badge_amber(ui, "⚠️ FIXED DISK");
                }
            });

            // Target drive dropdown combo
            ui.label(RichText::new("Target Storage Drive:").size(10.5).color(Color32::from_gray(160)));
            ui.horizontal(|ui| {
                let drive_display = if let Some(ref d) = selected_drive {
                    let label_str = if d.label.is_empty() { "NO NAME" } else { &d.label };
                    let type_str = if d.is_removable { "Removable" } else if d.is_system { "System" } else { "Fixed" };
                    format!("{}: {} {} ({})", d.letter.trim_end_matches('\\'), d.total_gb_str(), type_str, label_str)
                } else {
                    "No drive detected".to_string()
                };

                egui::ComboBox::from_id_salt("dashboard_drive_combo")
                    .width(ui.available_width() - 85.0)
                    .selected_text(RichText::new(&drive_display).size(11.5).strong().color(Color32::WHITE))
                    .show_ui(ui, |ui| {
                        for (idx, drive) in self.drives.iter().enumerate() {
                            let label_str = if drive.label.is_empty() { "NO NAME" } else { &drive.label };
                            let type_str = if drive.is_removable { "Removable" } else if drive.is_system { "System" } else { "Fixed" };
                            let text = format!("{}: {} {} ({}) - {}", drive.letter.trim_end_matches('\\'), drive.total_gb_str(), type_str, label_str, drive.file_system);
                            ui.selectable_value(&mut self.selected_drive_idx, idx, text);
                        }
                    });

                if ui.button(RichText::new("🔄 Refresh").size(10.5)).clicked() {
                    self.refresh_drives();
                }
            });

            ui.add_space(8.0);

            // Row 2: Current Filesystem & Safety Lock on left, Format toggles on right
            ui.columns(2, |fcols| {
                // Left Subcolumn
                let left_ui = &mut fcols[0];
                left_ui.vertical(|ui| {
                    ui.label(RichText::new("Current file system").size(10.0).color(Color32::from_gray(140)));
                    let fs_text = selected_drive.as_ref().map(|d| d.file_system.as_str()).unwrap_or("Unknown");
                    ui.label(RichText::new(fs_text).size(14.0).strong().color(Color32::WHITE));
                    ui.add_space(4.0);

                    // Safety Lock Badge
                    egui::Frame::new()
                        .fill(Color32::from_rgb(0, 32, 42))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(0, 229, 255)))
                        .corner_radius(12)
                        .inner_margin(Margin::symmetric(8, 4))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("🛡️ SAFETY LOCK").size(10.0).strong().color(Color32::from_rgb(0, 229, 255)));
                            });
                        });
                });

                // Right Subcolumn: Format toggles
                let right_ui = &mut fcols[1];
                right_ui.vertical(|ui| {
                    if !is_removable || is_system {
                        ui.colored_label(Color32::from_rgb(239, 68, 68), "Formatting is strictly locked for internal and system drives.");
                    } else {
                        // Format as FAT32
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Format as FAT32").size(11.0).color(Color32::from_gray(210)));
                            let mut fat32_on = self.do_format && self.format_fs == FormatFileSystem::Fat32;
                            if toggle_switch(ui, &mut fat32_on).changed() {
                                if fat32_on {
                                    self.do_format = true;
                                    self.format_fs = FormatFileSystem::Fat32;
                                } else if self.format_fs == FormatFileSystem::Fat32 {
                                    self.do_format = false;
                                }
                            }
                        });

                        ui.add_space(4.0);

                        // Format as exFAT
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Format as exFAT").size(11.0).color(Color32::from_gray(210)));
                            let mut exfat_on = self.do_format && self.format_fs == FormatFileSystem::ExFat;
                            if toggle_switch(ui, &mut exfat_on).changed() {
                                if exfat_on {
                                    self.do_format = true;
                                    self.format_fs = FormatFileSystem::ExFat;
                                } else if self.format_fs == FormatFileSystem::ExFat {
                                    self.do_format = false;
                                }
                            }
                        });
                    }
                });
            });

            // If drive has hidden/foreign partitions, show Diskpart MBR clean wipe
            if self.do_format && is_removable && !is_system {
                ui.add_space(6.0);
                if has_hidden {
                    let phys_str = selected_drive.as_ref().map(|d| d.physical_gb_str()).unwrap_or_default();
                    let vol_str = selected_drive.as_ref().map(|d| d.total_gb_str()).unwrap_or_default();
                    egui::Frame::new()
                        .fill(Color32::from_rgb(38, 26, 10))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(245, 158, 11)))
                        .corner_radius(6)
                        .inner_margin(Margin::same(6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("⚠️").size(12.0));
                                ui.label(RichText::new(format!("Hidden partitions detected: Volume is {} but card is {}.", vol_str, phys_str)).size(10.0).color(Color32::from_rgb(255, 200, 100)));
                            });
                        });
                    ui.add_space(4.0);
                }

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Full Repartition & Clean Wipe (Diskpart MBR)").size(10.5).color(Color32::WHITE));
                    toggle_switch(ui, &mut self.wipe_and_repartition);
                    if has_hidden {
                        badge_amber(ui, "Recommended");
                    }
                });

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Volume Label:").size(10.5).color(Color32::from_gray(160)));
                    ui.text_edit_singleline(&mut self.volume_label);
                    ui.checkbox(&mut self.format_confirmed, RichText::new("Confirm Erase").size(10.0).color(Color32::from_rgb(255, 180, 100)));
                });
            }
        });
    }

    // ------------------------------------------------------------------------
    // Card 2: 2. Device Profile
    // ------------------------------------------------------------------------
    fn render_card_device_profile(&mut self, ui: &mut egui::Ui) {
        card_frame().show(ui, |ui| {
            let active_profile = LauncherProfile::get_by_id(self.selected_profile_id);
            render_card_header(ui, "2. Device Profile", |ui| {
                badge_purple(ui, &format!("🕹️ {}", active_profile.name));
            });

            // Horizontal Profile Cards Grid
            ScrollArea::horizontal().show(ui, |ui| {
                ui.horizontal(|ui| {
                    for profile in PROFILES.iter() {
                        let is_sel = self.selected_profile_id == profile.id;
                        let frame = if is_sel {
                            card_frame_selected()
                        } else {
                            egui::Frame::new()
                                .fill(Color32::from_rgb(10, 15, 22))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(26, 36, 52)))
                                .corner_radius(8)
                                .inner_margin(Margin::same(8))
                        };

                        let icon = match profile.id {
                            ProfileId::AnbernicRgDsLauncher => "📱",
                            ProfileId::EmulationStationEsDe => "🖵",
                            ProfileId::RetroArch => "👾",
                            ProfileId::AnbernicOlderGarlicOs => "🧄",
                            ProfileId::MiyooMiniOnionOs => "🧅",
                            ProfileId::Daijisho => "📱",
                            ProfileId::Pegasus => "🎠",
                            ProfileId::BatoceraArkOs => "🦇",
                            ProfileId::Custom => "⚙️",
                        };

                        let short_name = match profile.id {
                            ProfileId::AnbernicRgDsLauncher => "ANBERNIC RG DS",
                            ProfileId::EmulationStationEsDe => "EMULATIONSTATION",
                            ProfileId::RetroArch => "RETROARCH",
                            ProfileId::AnbernicOlderGarlicOs => "GARLICOS",
                            ProfileId::MiyooMiniOnionOs => "MIYOO ONION",
                            ProfileId::Daijisho => "DAIJISHO",
                            ProfileId::Pegasus => "PEGASUS",
                            ProfileId::BatoceraArkOs => "BATOCERA",
                            ProfileId::Custom => "CUSTOM",
                        };

                        let inner = frame.show(ui, |ui| {
                            ui.set_width(116.0);
                            ui.set_height(82.0);
                            ui.vertical_centered(|ui| {
                                // Radio dot indicator on top-right
                                ui.horizontal(|ui| {
                                    ui.add_space(80.0);
                                    if is_sel {
                                        ui.label(RichText::new("🔘").size(10.0).color(Color32::from_rgb(0, 229, 255)));
                                    } else {
                                        ui.label(RichText::new("⚪").size(10.0).color(Color32::from_gray(80)));
                                    }
                                });

                                ui.label(RichText::new(icon).size(22.0));
                                ui.add_space(2.0);
                                ui.label(
                                    RichText::new(short_name)
                                        .size(9.5)
                                        .strong()
                                        .color(if is_sel { Color32::WHITE } else { Color32::from_gray(180) }),
                                );
                                ui.label(
                                    RichText::new(format!("{}/", profile.recommended_sd_subfolder))
                                        .size(8.5)
                                        .color(Color32::from_rgb(0, 229, 255)),
                                );
                            });
                        });

                        let id = inner.response.id.with(format!("prof_card_{:?}", profile.id));
                        if ui.interact(inner.response.rect, id, egui::Sense::click()).clicked() {
                            self.selected_profile_id = profile.id;
                            self.subfolder_name = profile.recommended_sd_subfolder.to_string();
                        }
                        ui.add_space(6.0);
                    }
                });
            });
        });
    }

    // ------------------------------------------------------------------------
    // Card 3: 3. Consoles Selection
    // ------------------------------------------------------------------------
    fn render_card_consoles_selection(&mut self, ui: &mut egui::Ui) {
        card_frame().show(ui, |ui| {
            render_card_header(ui, "3. Consoles Selection", |ui| {
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("All").size(10.0)).clicked() {
                        for p in &mut self.platform_states {
                            p.enabled = true;
                        }
                    }
                    if ui.button(RichText::new("None").size(10.0)).clicked() {
                        for p in &mut self.platform_states {
                            p.enabled = false;
                        }
                    }
                });
            });

            // Scrollable List of detected consoles
            ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
                for idx in 0..self.platform_states.len() {
                    let is_selected_for_preview = self.selected_platform_idx == idx;
                    let p_state = &mut self.platform_states[idx];
                    let is_found = p_state.found_dir.is_some();
                    let rom_count = p_state.rom_files.len();

                    let frame = if is_selected_for_preview {
                        egui::Frame::new()
                            .fill(Color32::from_rgb(16, 26, 38))
                            .stroke(Stroke::new(1.2, Color32::from_rgb(0, 229, 255)))
                            .corner_radius(6)
                            .inner_margin(Margin::symmetric(8, 6))
                    } else {
                        egui::Frame::new()
                            .fill(Color32::from_rgb(10, 14, 20))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(24, 32, 44)))
                            .corner_radius(6)
                            .inner_margin(Margin::symmetric(8, 6))
                    };

                    let p_icon = match p_state.platform.id {
                        "nds" => "📱",
                        "gba" => "🕹️",
                        "gb" | "gbc" => "📟",
                        "snes" | "nes" => "🎮",
                        "n64" => "🕹️",
                        "psx" | "ps2" => "💿",
                        "psp" => "🎮",
                        "megadrive" | "genesis" => "⚡",
                        "dreamcast" => "🌀",
                        "saturn" => "🪐",
                        "master_system" => "🕹️",
                        "gamegear" => "📟",
                        "gamecube" | "wii" => "💿",
                        _ => "🕹️",
                    };

                    let mut clicked_curate = false;

                    let inner = frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut p_state.enabled, "");
                            ui.label(RichText::new(p_icon).size(15.0));

                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(p_state.platform.name)
                                        .size(11.5)
                                        .strong()
                                        .color(if is_found { Color32::WHITE } else { Color32::from_gray(130) }),
                                );
                                ui.label(
                                    RichText::new(format!("{} ROMs", rom_count))
                                        .size(10.0)
                                        .color(if rom_count > 0 { Color32::from_rgb(0, 229, 255) } else { Color32::from_gray(120) }),
                                );
                            });

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if curated_favorites_button(ui) {
                                    clicked_curate = true;
                                }
                            });
                        });
                    });

                    if clicked_curate {
                        self.open_favorites_editor(idx);
                    }

                    let row_id = inner.response.id.with(format!("console_row_{}", idx));
                    if ui.interact(inner.response.rect, row_id, egui::Sense::click()).clicked() {
                        self.selected_platform_idx = idx;
                    }

                    ui.add_space(4.0);
                }
            });
        });
    }

    // ------------------------------------------------------------------------
    // Card 4: 4. Live ROMs & Boxart Preview
    // ------------------------------------------------------------------------
    fn render_card_roms_preview(&mut self, ui: &mut egui::Ui) {
        card_frame().show(ui, |ui| {
            let active_plat = self.platform_states.get(self.selected_platform_idx).cloned();
            let plat_title = active_plat
                .as_ref()
                .map(|p| format!("{} ({} ROMs)", p.platform.name, p.rom_files.len()))
                .unwrap_or_else(|| "ROMs & Boxart Preview".to_string());

            render_card_header(ui, "4. Live ROMs & Boxart Preview", |ui| {
                badge_cyan(ui, &plat_title);
            });

            // Subheader: Search box + Boxart Sync toggle
            ui.horizontal(|ui| {
                ui.label(RichText::new("🔍").size(12.0));
                ui.text_edit_singleline(&mut self.rom_search_query);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("Sync Boxart").size(10.5).color(Color32::from_gray(200)));
                    toggle_switch(ui, &mut self.download_art);
                    badge_green(ui, "Libretro CDN");
                });
            });

            ui.add_space(6.0);

            // Games Table
            ScrollArea::vertical().max_height(210.0).show(ui, |ui| {
                if let Some(ref p_state) = active_plat {
                    let search_lower = self.rom_search_query.to_lowercase();
                    let top_list = p_state.platform.default_favorites;
                    let fav_set = p_state.favorites.as_ref().map(|f| f.games.iter().collect::<HashSet<_>>());

                    let mut shown = 0;
                    for rom in &p_state.rom_files {
                        if !search_lower.is_empty() && !rom.to_lowercase().contains(&search_lower) {
                            continue;
                        }
                        shown += 1;
                        let is_top = top_list.iter().any(|&c| rom.to_lowercase().contains(&c.to_lowercase()));
                        let is_fav = fav_set.as_ref().map(|s| s.contains(rom)).unwrap_or(false);

                        egui::Frame::new()
                            .fill(Color32::from_rgb(10, 14, 20))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(22, 30, 42)))
                            .corner_radius(4)
                            .inner_margin(Margin::symmetric(6, 4))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    // Thumbnail / Image Placeholder
                                    egui::Frame::new()
                                        .fill(Color32::from_rgb(16, 22, 32))
                                        .stroke(Stroke::new(1.0, Color32::from_rgb(0, 180, 210)))
                                        .corner_radius(4)
                                        .inner_margin(Margin::same(4))
                                        .show(ui, |ui| {
                                            ui.label(RichText::new("🖼️").size(12.0));
                                        });

                                    ui.vertical(|ui| {
                                        ui.label(RichText::new(rom).size(11.0).strong().color(Color32::WHITE));
                                        ui.label(RichText::new(format!("Platform: {}", p_state.platform.name)).size(9.5).color(Color32::from_gray(140)));
                                    });

                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if is_top {
                                            badge_green(ui, "Top Classic");
                                        } else if is_fav {
                                            badge_purple(ui, "Curated");
                                        } else {
                                            badge_cyan(ui, "ROM");
                                        }
                                    });
                                });
                            });
                        ui.add_space(2.0);
                    }

                    if shown == 0 {
                        ui.label(RichText::new("No ROMs match the filter or folder empty.").size(10.5).color(Color32::GRAY));
                    }
                } else {
                    ui.label(RichText::new("No platform selected.").size(10.5).color(Color32::GRAY));
                }
            });
        });
    }

    // ------------------------------------------------------------------------
    // Card 5: 5. QuickInstaller Cockpit
    // ------------------------------------------------------------------------
    fn render_card_installer_cockpit(&mut self, ui: &mut egui::Ui) {
        card_frame().show(ui, |ui| {
            render_card_header(ui, "5. QuickInstaller Cockpit", |ui| {
                if self.is_running {
                    badge_amber(ui, "🚀 EXECUTING");
                } else if self.install_summary.is_some() {
                    badge_green(ui, "✓ COMPLETE");
                } else {
                    badge_green(ui, "READY");
                }
            });

            // Progress Bar 1: PLATFORM SYNC
            let sync_frac = if self.platform_states.is_empty() {
                0.0
            } else {
                let enabled_count = self.platform_states.iter().filter(|p| p.enabled && p.found_dir.is_some()).count();
                if enabled_count == 0 { 0.0 } else { 1.0 }
            };
            let sync_text = format!("{:.0}%", sync_frac * 100.0);
            render_neon_progress_bar(ui, "PLATFORM SYNC", sync_frac, &sync_text);

            // Progress Bar 2: FILE COPY
            let copy_frac = if self.progress_total > 0 {
                self.progress_current as f32 / self.progress_total as f32
            } else {
                0.0
            };
            let copy_text = if self.progress_total > 0 {
                format!("{}/{} ({:.0}%)", self.progress_current, self.progress_total, copy_frac * 100.0)
            } else {
                "0/0 (0%)".to_string()
            };
            render_neon_progress_bar(ui, "FILE COPY", copy_frac, &copy_text);

            let item_label = if self.current_item.is_empty() {
                if self.is_running { "Copying game assets..." } else { "System idling. Ready for quickinstall." }
            } else {
                &self.current_item
            };
            ui.label(RichText::new(item_label).size(10.0).color(Color32::from_gray(150)));

            ui.add_space(8.0);

            // Giant QuickInstall Button
            if !self.is_running {
                let can_start = !self.drives.is_empty()
                    && (!self.do_format || self.format_confirmed)
                    && self.platform_states.iter().any(|p| p.enabled && p.found_dir.is_some());

                let target_letter = self.drives.get(self.selected_drive_idx).map(|d| d.letter.as_str()).unwrap_or("E:");

                let start_btn = egui::Button::new(
                    RichText::new(format!("🚀 START QUICKINSTALL ({})", target_letter))
                        .size(13.0)
                        .strong()
                        .color(Color32::BLACK),
                )
                .fill(Color32::from_rgb(16, 185, 129));

                if ui.add_sized([ui.available_width(), 36.0], start_btn).clicked() && can_start {
                    self.start_install();
                }

                if !can_start {
                    ui.add_space(2.0);
                    ui.label(RichText::new("Confirm erase or verify removable drive to launch.").size(9.5).color(Color32::from_rgb(255, 180, 100)));
                }
            } else {
                let cancel_btn = egui::Button::new(
                    RichText::new("🛑 CANCEL QUICKINSTALL")
                        .size(13.0)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(Color32::from_rgb(239, 68, 68));

                if ui.add_sized([ui.available_width(), 36.0], cancel_btn).clicked() {
                    self.cancel_flag.store(true, Ordering::Relaxed);
                    self.logs.push("[CANCEL] Cancellation requested by user.".to_string());
                }
            }
        });
    }

    // ------------------------------------------------------------------------
    // Card 6: Terminal Console (terminal.log)
    // ------------------------------------------------------------------------
    fn render_card_terminal_console(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(Color32::from_rgb(6, 9, 14))
            .stroke(Stroke::new(1.0, Color32::from_rgb(26, 36, 50)))
            .corner_radius(8)
            .inner_margin(Margin::same(0))
            .show(ui, |ui| {
                // Header with macOS/Linux colored dots
                egui::Frame::new()
                    .fill(Color32::from_rgb(12, 17, 26))
                    .corner_radius(8)
                    .inner_margin(Margin::symmetric(10, 6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("🔴").size(8.0));
                            ui.label(RichText::new("🟡").size(8.0));
                            ui.label(RichText::new("🟢").size(8.0));
                            ui.add_space(4.0);
                            ui.label(RichText::new("terminal.log").monospace().size(10.5).color(Color32::from_gray(180)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                badge_cyan(ui, "Execution Stream");
                            });
                        });
                    });

                // Scrollable Console Body
                egui::Frame::new()
                    .fill(Color32::from_rgb(6, 9, 14))
                    .inner_margin(Margin::same(8))
                    .show(ui, |ui| {
                        ScrollArea::vertical()
                            .max_height(145.0)
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                for line in &self.logs {
                                    let color = if line.contains("ERROR") || line.contains("Failed") {
                                        Color32::from_rgb(255, 100, 100)
                                    } else if line.contains("Phase") || line.contains("Succeeded") {
                                        Color32::from_rgb(0, 229, 255)
                                    } else if line.contains("ROMS") || line.contains("DRIVE") {
                                        Color32::from_rgb(168, 130, 255)
                                    } else {
                                        Color32::from_rgb(180, 205, 235)
                                    };
                                    ui.label(RichText::new(line).monospace().size(9.5).color(color));
                                }
                            });
                    });
            });
    }
}

// ----------------------------------------------------------------------------
// Curated Favorites Modal Dialog (Matching cardmaker_favorites_modal_1790889844749.jpg)
// ----------------------------------------------------------------------------

impl RetroCardMakerApp {
    fn render_favorites_modal(&mut self, ui: &mut egui::Ui) {
        let Some(p_idx) = self.editing_platform_idx else {
            return;
        };

        let p_state = &self.platform_states[p_idx];
        let p_name = p_state.platform.name;
        let rom_count = p_state.rom_files.len();
        let mut close_modal = false;
        let mut do_save = false;

        egui::Window::new(format!("Curate Favorites: {} ({}) - {} ROMs", p_name, p_state.platform.id.to_uppercase(), rom_count))
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(880.0, 560.0))
            .show(ui.ctx(), |ui| {
                // Top Header Action Bar
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("⭐ Select Recommended Top Classics").size(11.0).strong()).clicked() {
                        let top = FavoritesList::from_default_platform(&p_state.platform);
                        for item in top.games {
                            self.editing_selected_games.insert(item);
                        }
                    }

                    if ui.button(RichText::new("Select All").size(10.5)).clicked() {
                        for r in &p_state.rom_files {
                            self.editing_selected_games.insert(r.clone());
                        }
                    }
                    if ui.button(RichText::new("Clear").size(10.5)).clicked() {
                        self.editing_selected_games.clear();
                    }

                    ui.add_space(8.0);
                    ui.label(RichText::new("🔍").size(11.0));
                    ui.text_edit_singleline(&mut self.editing_search_query);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        badge_purple(ui, &format!("{} of {} Curated", self.editing_selected_games.len(), rom_count));
                    });
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // Two-Column Grid: Left (~55%) Games List, Right (~45%) Live Artwork Preview & Patterns
                ui.columns(2, |mcols| {
                    // Left Column: Games Table
                    let ui = &mut mcols[0];
                    ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                        let search_lower = self.editing_search_query.to_lowercase();
                        let top_list = p_state.platform.default_favorites;

                        let mut shown = 0;
                        for rom in &p_state.rom_files {
                            let is_checked = self.editing_selected_games.contains(rom);
                            if !search_lower.is_empty() && !rom.to_lowercase().contains(&search_lower) {
                                continue;
                            }
                            shown += 1;
                            let is_top = top_list.iter().any(|&c| rom.to_lowercase().contains(&c.to_lowercase()));
                            let icon = get_game_icon(rom);

                            let frame = if is_checked {
                                egui::Frame::new()
                                    .fill(Color32::from_rgb(14, 25, 36))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(0, 229, 255)))
                                    .corner_radius(6)
                                    .inner_margin(Margin::symmetric(6, 4))
                            } else {
                                egui::Frame::new()
                                    .fill(Color32::from_rgb(10, 14, 20))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(22, 30, 42)))
                                    .corner_radius(6)
                                    .inner_margin(Margin::symmetric(6, 4))
                            };

                            let inner = frame.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let mut chk = is_checked;
                                    if ui.checkbox(&mut chk, "").changed() {
                                        if chk {
                                            self.editing_selected_games.insert(rom.clone());
                                        } else {
                                            self.editing_selected_games.remove(rom);
                                        }
                                    }

                                    ui.label(RichText::new(icon).size(13.0));
                                    ui.label(RichText::new(rom).size(11.0).color(if is_checked { Color32::WHITE } else { Color32::from_gray(170) }));

                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if is_top {
                                            badge_green(ui, "★ Top Classic");
                                        }
                                    });
                                });
                            });

                            let id = inner.response.id.with(format!("modal_game_{}", rom));
                            if ui.interact(inner.response.rect, id, egui::Sense::click()).clicked() {
                                self.favorites_preview_game = Some(rom.clone());
                            }
                            ui.add_space(2.0);
                        }

                        if shown == 0 {
                            ui.label(RichText::new("No games found.").size(10.5).color(Color32::GRAY));
                        }
                    });

                    // Right Column: Live Libretro Preview Card & Pattern Rules
                    let ui = &mut mcols[1];
                    ui.label(RichText::new("Live preview").size(11.0).strong().color(Color32::WHITE));
                    ui.add_space(2.0);

                    // Artwork Preview Card
                    egui::Frame::new()
                        .fill(Color32::from_rgb(10, 15, 22))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(26, 38, 54)))
                        .corner_radius(8)
                        .inner_margin(Margin::same(12))
                        .show(ui, |ui| {
                            let preview_title = self.favorites_preview_game.clone().unwrap_or_else(|| "Chrono Trigger".to_string());

                            ui.horizontal(|ui| {
                                // Cover art placeholder with Libretro branding
                                egui::Frame::new()
                                    .fill(Color32::from_rgb(18, 30, 44))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(0, 229, 255)))
                                    .corner_radius(6)
                                    .inner_margin(Margin::same(16))
                                    .show(ui, |ui| {
                                        ui.vertical_centered(|ui| {
                                            ui.label(RichText::new("LIBRETRO").size(9.0).strong().color(Color32::from_rgb(0, 229, 255)));
                                            ui.label(RichText::new("🖼️").size(28.0));
                                            ui.label(RichText::new("COVER").size(8.0).color(Color32::from_gray(160)));
                                        });
                                    });

                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&preview_title).size(12.5).strong().color(Color32::WHITE));
                                    ui.label(RichText::new(format!("Platform: {}", p_state.platform.name)).size(10.0).color(Color32::from_gray(150)));
                                    ui.label(RichText::new("Publisher: Verified Dump").size(9.5).color(Color32::from_gray(140)));
                                    ui.label(RichText::new("CDN: Libretro Thumbnails").size(9.5).color(Color32::from_rgb(16, 185, 129)));
                                });
                            });
                        });

                    ui.add_space(8.0);

                    // Keyword Pattern Rules Card
                    egui::Frame::new()
                        .fill(Color32::from_rgb(10, 15, 22))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(26, 38, 54)))
                        .corner_radius(8)
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            ui.label(RichText::new("Keyword Pattern Rules:").size(10.5).strong().color(Color32::from_rgb(0, 229, 255)));
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut self.new_favorite_pattern);
                                if ui.button(RichText::new("+ Add").size(10.5)).clicked() && !self.new_favorite_pattern.trim().is_empty() {
                                    let pat = self.new_favorite_pattern.trim().to_string();
                                    if !self.editing_custom_patterns.contains(&pat) {
                                        self.editing_custom_patterns.push(pat);
                                    }
                                    self.new_favorite_pattern.clear();
                                }
                            });

                            ui.add_space(4.0);
                            ui.horizontal_wrapped(|ui| {
                                let mut to_remove = None;
                                for (p_i, pat) in self.editing_custom_patterns.iter().enumerate() {
                                    badge_cyan(ui, pat);
                                    if ui.button(RichText::new("✕").size(8.5)).clicked() {
                                        to_remove = Some(p_i);
                                    }
                                }
                                if let Some(r) = to_remove {
                                    self.editing_custom_patterns.remove(r);
                                }
                            });
                        });
                });

                if let Some(ref note) = self.favorites_save_notification {
                    ui.add_space(4.0);
                    ui.colored_label(Color32::from_rgb(80, 240, 120), note);
                }

                ui.add_space(8.0);
                ui.separator();

                // Modal Footer with Lime Green Save Button
                ui.horizontal(|ui| {
                    let target_folder = p_state
                        .found_dir
                        .as_ref()
                        .map(|d| d.display().to_string())
                        .unwrap_or_else(|| "Not found".to_string());
                    ui.label(RichText::new(format!("Destination: {}\\{}", target_folder, "favorites.json")).size(9.5).color(Color32::from_gray(140)));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let save_btn = egui::Button::new(
                            RichText::new("Save favorites.json")
                                .size(11.5)
                                .strong()
                                .color(Color32::BLACK),
                        )
                        .fill(Color32::from_rgb(132, 204, 22)); // Glowing Lime Green

                        if ui.add(save_btn).clicked() {
                            do_save = true;
                        }

                        ui.add_space(4.0);
                        ui.label(RichText::new("Sync Boxart").size(10.5).color(Color32::from_gray(200)));
                        toggle_switch(ui, &mut self.download_art);

                        if ui.button(RichText::new("Close").size(11.0)).clicked() {
                            close_modal = true;
                        }
                    });
                });
            });

        if do_save {
            if let Some(ref dir) = self.platform_states[p_idx].found_dir {
                let mut games_list: Vec<String> = self.editing_selected_games.iter().cloned().collect();
                games_list.sort();

                let favs = FavoritesList {
                    platform: self.platform_states[p_idx].platform.id.to_string(),
                    title: format!("{} Curated Favorites", self.platform_states[p_idx].platform.name),
                    games: games_list,
                    patterns: self.editing_custom_patterns.clone(),
                };

                match save_favorites(dir, &favs) {
                    Ok(_) => {
                        self.favorites_save_notification = Some(format!("✓ Successfully saved favorites.json with {} games!", favs.games.len()));
                        self.platform_states[p_idx].favorites = Some(favs);
                    }
                    Err(e) => {
                        self.favorites_save_notification = Some(format!("Error saving favorites: {}", e));
                    }
                }
            } else {
                self.favorites_save_notification = Some("Error: Source folder not found.".to_string());
            }
        }

        if close_modal {
            self.editing_platform_idx = None;
            self.editing_selected_games.clear();
            self.editing_custom_patterns.clear();
            self.favorites_save_notification = None;
        }
    }
}

// ----------------------------------------------------------------------------
// Full-Screen Favorites Tab View (Matching cardmaker_compact_ui_1790890233113.jpg)
// ----------------------------------------------------------------------------

impl RetroCardMakerApp {
    fn render_favorites_tab_view(&mut self, ui: &mut egui::Ui) {
        card_frame().show(ui, |ui| {
            let active_idx = self.selected_platform_idx;
            let active_plat = self.platform_states.get(active_idx).cloned();
            let plat_name = active_plat.as_ref().map(|p| p.platform.name).unwrap_or("Nintendo DS");
            let plat_code = active_plat.as_ref().map(|p| p.platform.id.to_uppercase()).unwrap_or_else(|| "NDS".to_string());

            render_card_header(ui, &format!("Curate Favorites: {} ({})", plat_name, plat_code), |ui| {
                ui.horizontal(|ui| {
                    for (i, p) in self.platform_states.iter().enumerate().take(8) {
                        let is_cur = self.selected_platform_idx == i;
                        let btn = egui::Button::new(RichText::new(p.platform.id.to_uppercase()).size(9.5).color(if is_cur { Color32::WHITE } else { Color32::from_gray(160) }))
                            .fill(if is_cur { Color32::from_rgb(0, 160, 210) } else { Color32::from_rgb(18, 24, 34) });
                        if ui.add(btn).clicked() {
                            self.selected_platform_idx = i;
                        }
                    }
                });
            });

            // Two Columns: Left (~60%) Games List, Right (~40%) Live JSON Code Preview
            ui.columns(2, |fcols| {
                let left_ui = &mut fcols[0];
                left_ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔍").size(12.0));
                        ui.text_edit_singleline(&mut self.rom_search_query);

                        badge_cyan(ui, "Genre: RPG ✕");
                        badge_purple(ui, "Format: Verified ✕");
                    });

                    ui.add_space(6.0);

                    // Games list table
                    ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                        if let Some(p_state) = self.platform_states.get_mut(active_idx) {
                            let top_list = p_state.platform.default_favorites;
                            let search_lower = self.rom_search_query.to_lowercase();

                            for rom in &p_state.rom_files {
                                if !search_lower.is_empty() && !rom.to_lowercase().contains(&search_lower) {
                                    continue;
                                }

                                let is_top = top_list.iter().any(|&c| rom.to_lowercase().contains(&c.to_lowercase()));
                                let icon = get_game_icon(rom);

                                egui::Frame::new()
                                    .fill(Color32::from_rgb(12, 17, 24))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(24, 34, 48)))
                                    .corner_radius(6)
                                    .inner_margin(Margin::symmetric(8, 6))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            let mut checked = is_top;
                                            ui.checkbox(&mut checked, "");
                                            ui.label(RichText::new(icon).size(14.0));
                                            ui.vertical(|ui| {
                                                ui.label(RichText::new(rom).size(11.0).strong().color(Color32::WHITE));
                                                ui.label(RichText::new(format!("System: {}", p_state.platform.name)).size(9.0).color(Color32::from_gray(140)));
                                            });

                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if is_top {
                                                    badge_green(ui, "Top Classic");
                                                }
                                                ui.label(RichText::new("[48 MB]").size(9.5).color(Color32::from_gray(150)));
                                            });
                                        });
                                    });
                                ui.add_space(2.0);
                            }
                        }
                    });
                });

                // Right Column: favorites.json Code Preview
                let right_ui = &mut fcols[1];
                right_ui.vertical(|ui| {
                    card_frame().show(ui, |ui| {
                        ui.label(RichText::new("favorites.json Code Preview").size(11.5).strong().color(Color32::WHITE));
                        ui.add_space(4.0);

                        let p_info = &self.platform_states[active_idx].platform;
                        let sample_fav = FavoritesList::from_default_platform(p_info);
                        if let Ok(mut json_code) = serde_json::to_string_pretty(&sample_fav) {
                            ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut json_code)
                                        .font(egui::TextStyle::Monospace)
                                        .desired_rows(18)
                                        .desired_width(f32::INFINITY),
                                );
                            });
                        }
                    });
                });
            });

            ui.add_space(8.0);
            ui.separator();

            // Bottom Footer
            ui.horizontal(|ui| {
                ui.label(RichText::new("18 of 120 selected").size(11.0).strong().color(Color32::from_rgb(0, 229, 255)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let save_btn = egui::Button::new(RichText::new("Save favorites.json").size(11.5).strong().color(Color32::BLACK))
                        .fill(Color32::from_rgb(16, 185, 129));
                    if ui.add(save_btn).clicked() {
                        self.open_favorites_editor(active_idx);
                    }
                    if ui.button(RichText::new("🔄 Refresh List").size(11.0)).clicked() {
                        self.scan_source_folder();
                    }
                });
            });
        });
    }

    // ------------------------------------------------------------------------
    // Consoles Manager Tab View
    // ------------------------------------------------------------------------
    fn render_consoles_tab_view(&mut self, ui: &mut egui::Ui) {
        card_frame().show(ui, |ui| {
            render_card_header(ui, "Consoles & Directory Architecture Manager", |ui| {
                badge_purple(ui, &format!("{} Supported Systems", self.platform_states.len()));
            });

            // Source Directory Bar
            ui.horizontal(|ui| {
                ui.label(RichText::new("Source Library Root:").size(11.0).strong().color(Color32::WHITE));
                ui.text_edit_singleline(&mut self.source_path);
                if ui.button(RichText::new("📂 Browse...").size(11.0)).clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.source_path = folder.to_string_lossy().to_string();
                        self.scan_source_folder();
                    }
                }
                if ui.button(RichText::new("🔄 Rescan").size(11.0)).clicked() {
                    self.scan_source_folder();
                }
            });

            ui.add_space(8.0);

            // Table of Platforms
            ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
                let mut open_idx = None;
                for idx in 0..self.platform_states.len() {
                    let p_state = &mut self.platform_states[idx];
                    let is_found = p_state.found_dir.is_some();
                    let plat_name = p_state.platform.name;
                    let plat_id_upper = p_state.platform.id.to_uppercase();
                    let rom_count = p_state.rom_files.len();
                    let path_str = p_state.found_dir.as_ref().map(|d| d.display().to_string()).unwrap_or_else(|| "Directory not found in source library".to_string());

                    egui::Frame::new()
                        .fill(Color32::from_rgb(11, 16, 24))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(24, 34, 48)))
                        .corner_radius(6)
                        .inner_margin(Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut p_state.enabled, "");
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(plat_name).size(12.0).strong().color(Color32::WHITE));
                                        badge_cyan(ui, &plat_id_upper);
                                    });

                                    ui.label(RichText::new(path_str).size(10.0).color(if is_found { Color32::from_gray(160) } else { Color32::from_gray(110) }));
                                });

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if curated_favorites_button(ui) {
                                        open_idx = Some(idx);
                                    }

                                    badge_green(ui, &format!("{} ROMs", rom_count));
                                });
                            });
                        });
                    ui.add_space(4.0);
                }
                if let Some(idx) = open_idx {
                    self.open_favorites_editor(idx);
                }
            });
        });
    }

    // ------------------------------------------------------------------------
    // Settings Tab View
    // ------------------------------------------------------------------------
    fn render_settings_tab_view(&mut self, ui: &mut egui::Ui) {
        card_frame().show(ui, |ui| {
            render_card_header(ui, "Application & Storage Safety Settings", |ui| {
                badge_cyan(ui, "Configuration");
            });

            ui.columns(2, |cols| {
                // Column 1: Storage Protection
                cols[0].vertical(|ui| {
                    ui.label(RichText::new("Storage Safety Safeguards").size(12.0).strong().color(Color32::WHITE));
                    ui.add_space(4.0);
                    ui.label(RichText::new("• Formatting is strictly restricted to removable USB and SD cards.\n• System drive C: and internal fixed SSDs (NVMe) are hardware locked.\n• Diskpart clean wipe restores 100% capacity from trapped Linux/ext4 partitions.").size(10.0).color(Color32::from_gray(160)));
                });

                // Column 2: Libretro CDN Scraper
                cols[1].vertical(|ui| {
                    ui.label(RichText::new("Libretro Artwork CDN Engine").size(12.0).strong().color(Color32::WHITE));
                    ui.add_space(4.0);
                    ui.label(RichText::new("• Base URL: https://raw.githubusercontent.com/libretro-thumbnails/libretro-thumbnails/master/\n• Automatic name sanitization (&, :, /, \\ converted to _).\n• Zero API key requirement with high-speed parallel asset syncing.").size(10.0).color(Color32::from_gray(160)));
                });
            });

            ui.add_space(16.0);
            ui.separator();
            ui.label(RichText::new("Retro CardMaker v0.2.1 • Native Windows x64 Build • Pair programming Antigravity").size(10.0).color(Color32::from_gray(140)));
        });
    }
}
