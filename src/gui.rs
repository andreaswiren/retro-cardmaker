use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use walkdir::WalkDir;

use eframe::egui::{self, Color32, Margin, RichText, ScrollArea, Stroke, Vec2};

use crate::art_scraper::ArtType;
use crate::dedup::{filter_roms_1g1r, RegionPreference};
use crate::drives::{get_available_drives, is_elevated, relaunch_as_admin, DriveInfo, FormatFileSystem};
use crate::favorites::{load_favorites, save_favorites, FavoritesList};
use crate::installer::{
    ArtLocationMode, CopyMode, InstallConfig, InstallSummary, InstallerEngine, InstallerEvent,
    InstallerExecutionMode, PlatformInstallConfig,
};
use crate::launcher_profiles::{LauncherProfile, ProfileId, PROFILES};
use crate::platforms::{filter_primary_rom_files, find_platform_by_dir_name, PlatformInfo, PLATFORMS};

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
// Custom Fonts & ClearType Windows 11 Anti-Aliasing Setup
// ----------------------------------------------------------------------------

pub fn setup_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    #[cfg(windows)]
    {
        let font_dir = std::path::Path::new("C:\\Windows\\Fonts");

        // 1. Primary Proportional Font: Segoe UI (Clean, clear anti-aliasing on Windows)
        let segoe_path = font_dir.join("segoeui.ttf");
        if segoe_path.exists() {
            if let Ok(data) = std::fs::read(&segoe_path) {
                fonts.font_data.insert(
                    "SegoeUI".to_owned(),
                    std::sync::Arc::new(egui::FontData::from_owned(data)),
                );
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, "SegoeUI".to_owned());
            }
        }

        // 2. Bold variant: Segoe UI Bold
        let segoe_bold = font_dir.join("segoeuib.ttf");
        if segoe_bold.exists() {
            if let Ok(data) = std::fs::read(&segoe_bold) {
                fonts.font_data.insert(
                    "SegoeUI-Bold".to_owned(),
                    std::sync::Arc::new(egui::FontData::from_owned(data)),
                );
            }
        }

        // 3. Emoji & Symbol Font: Segoe UI Emoji
        let emoji_path = font_dir.join("seguiemj.ttf");
        if emoji_path.exists() {
            if let Ok(data) = std::fs::read(&emoji_path) {
                fonts.font_data.insert(
                    "SegoeUIEmoji".to_owned(),
                    std::sync::Arc::new(egui::FontData::from_owned(data)),
                );
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .push("SegoeUIEmoji".to_owned());
            }
        }

        // 4. Windows 11 Fluent Icons / Segoe MDL2 Assets
        let mdl2_path = font_dir.join("segmdl2.ttf");
        if mdl2_path.exists() {
            if let Ok(data) = std::fs::read(&mdl2_path) {
                fonts.font_data.insert(
                    "SegoeMDL2".to_owned(),
                    std::sync::Arc::new(egui::FontData::from_owned(data)),
                );
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .push("SegoeMDL2".to_owned());
            }
        }
    }

    ctx.set_fonts(fonts);
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

#[allow(dead_code)]
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
        .inner_margin(Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.add(
                egui::Label::new(RichText::new(text).size(11.5).color(fg).strong())
                    .wrap_mode(egui::TextWrapMode::Extend),
            );
        });
}

fn pattern_chip(ui: &mut egui::Ui, tag: &str) -> bool {
    let btn = egui::Button::new(
        RichText::new(format!("{}  ×", tag))
            .size(11.5)
            .strong()
            .color(Color32::from_rgb(0, 229, 255)),
    )
    .fill(Color32::from_rgb(0, 32, 48))
    .stroke(Stroke::new(1.0, Color32::from_rgb(0, 180, 210)))
    .corner_radius(12);

    ui.add(btn)
        .on_hover_text("Click to remove keyword pattern")
        .clicked()
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
        ui.label(RichText::new(title).size(14.5).strong().color(Color32::WHITE));
        let avail = (ui.available_width() - 8.0).max(10.0);
        ui.allocate_ui(Vec2::new(avail, 24.0), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                badge_fn(ui);
            });
        });
    });
    ui.add_space(3.0);
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
        ui.label(RichText::new(label).size(12.5).strong().color(Color32::from_gray(210)));
        let avail = (ui.available_width() - 8.0).max(10.0);
        ui.allocate_ui(Vec2::new(avail, 20.0), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(extra_text).size(12.0).strong().color(Color32::from_rgb(0, 229, 255)));
            });
        });
    });
    ui.add_space(3.0);
    let height = 14.0;
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let radius = height / 2.0;
        ui.painter().rect_filled(rect, radius, Color32::from_rgb(16, 24, 36));
        let clamped = fraction.clamp(0.0, 1.0);
        if clamped > 0.001 {
            let fill_width = (rect.width() * clamped).max(height);
            let fill_rect = egui::Rect::from_min_size(rect.min, Vec2::new(fill_width, height));
            ui.painter().rect_filled(fill_rect, radius, Color32::from_rgb(0, 229, 255));
        }
    }
    ui.add_space(6.0);
}

// Real Button widget with vibrant cyan border and hover styling
fn curated_favorites_button(ui: &mut egui::Ui) -> bool {
    let btn = egui::Button::new(
        RichText::new("⭐ Curate Favorites")
            .size(11.5)
            .strong()
            .color(Color32::from_rgb(0, 229, 255)),
    )
    .fill(Color32::from_rgb(0, 36, 52))
    .stroke(Stroke::new(1.2, Color32::from_rgb(0, 229, 255)))
    .corner_radius(12);

    ui.add(btn).clicked()
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
    pub deduplicate_1g1r: bool,
    pub region_preference: RegionPreference,
    pub exclude_betas: bool,

    // Step 4: Artwork & Parallel Workers
    pub download_art: bool,
    pub art_types: HashSet<ArtType>,
    pub art_location_mode: ArtLocationMode,
    pub art_subfolder_name: String,
    pub copy_threads: usize,
    pub art_threads: usize,
    pub copy_roms_first: bool,

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

    // Headless / Testing Screenshot Support
    pub screenshot_path: Option<String>,
    pub frame_count: usize,
}

impl RetroCardMakerApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        screenshot_path: Option<String>,
        screenshot_tab: Option<String>,
        screenshot_modal: bool,
        screenshot_step: Option<usize>,
    ) -> Self {
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
            deduplicate_1g1r: true,
            region_preference: RegionPreference::UsaFirst,
            exclude_betas: true,

            download_art: true,
            art_types: [ArtType::Boxart].into_iter().collect(),
            art_location_mode: ArtLocationMode::RomSourceSubfolder,
            art_subfolder_name: "Imgs".to_string(),
            copy_threads: 4,
            art_threads: 6,
            copy_roms_first: true,

            is_running: false,
            current_phase: "Ready".to_string(),
            progress_current: 0,
            progress_total: 0,
            current_item: String::new(),
            logs: vec![
                format!("[INFO] Initializing Retro CardMaker v{} Core Engine...", env!("CARGO_PKG_VERSION")),
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

            screenshot_path,
            frame_count: 0,
        };

        // Apply Windows 11 Fluent Segoe UI & Emoji Fonts with ClearType Anti-Aliasing
        setup_custom_fonts(&_cc.egui_ctx);

        let mut style = (*_cc.egui_ctx.style_of(egui::Theme::Dark)).clone();
        style.text_styles = [
            (egui::TextStyle::Heading, egui::FontId::new(18.0, egui::FontFamily::Proportional)),
            (egui::TextStyle::Body, egui::FontId::new(13.5, egui::FontFamily::Proportional)),
            (egui::TextStyle::Monospace, egui::FontId::new(12.5, egui::FontFamily::Monospace)),
            (egui::TextStyle::Button, egui::FontId::new(13.0, egui::FontFamily::Proportional)),
            (egui::TextStyle::Small, egui::FontId::new(11.5, egui::FontFamily::Proportional)),
        ].into();

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
        style.visuals = visuals;

        _cc.egui_ctx.set_style_of(egui::Theme::Dark, style.clone());
        _cc.egui_ctx.set_style_of(egui::Theme::Light, style);

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

        if let Some(ref tab_name) = screenshot_tab {
            match tab_name.to_lowercase().as_str() {
                "favorites" => app.active_tab = ActiveTab::Favorites,
                "consoles" | "platforms" => app.active_tab = ActiveTab::Consoles,
                "settings" => app.active_tab = ActiveTab::Settings,
                _ => app.active_tab = ActiveTab::Dashboard,
            }
        }

        if let Some(step) = screenshot_step {
            app.wizard_focused_step = step.clamp(1, 5);
            app.active_tab = ActiveTab::Dashboard;
        }

        if screenshot_modal {
            app.open_favorites_editor(app.selected_platform_idx);
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

                let mut raw_files = Vec::new();
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
                                raw_files.push(name.to_string());
                            }
                        }
                    }
                }
                p_state.rom_files = filter_primary_rom_files(p_state.platform.id, &raw_files);
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
            // Clean initial state: only select games that actually exist in the platform
            let top_list = p_state.platform.default_favorites;
            for rom in &p_state.rom_files {
                if top_list.iter().any(|&c| rom.to_lowercase().contains(&c.to_lowercase())) {
                    self.editing_selected_games.insert(rom.clone());
                }
            }
            // Add concise franchise pattern tags rather than 15 full game titles
            self.editing_custom_patterns.push("*Mario*".to_string());
            self.editing_custom_patterns.push("*Zelda*".to_string());
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
        if self.do_format && !is_elevated() {
            self.install_error = Some("Administrator privileges required to format. Please relaunch as Administrator.".to_string());
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

        let effective_art_types: Vec<ArtType> = if self.art_types.is_empty() {
            vec![ArtType::Boxart]
        } else {
            self.art_types.iter().copied().collect()
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
            art_types: effective_art_types.clone(),
            art_location_mode: self.art_location_mode,
            art_subfolder_name: self.art_subfolder_name.clone(),
            region_preference: if self.deduplicate_1g1r { self.region_preference } else { RegionPreference::None },
            exclude_betas: self.exclude_betas && self.deduplicate_1g1r,
            copy_threads: self.copy_threads,
            art_threads: self.art_threads,
            copy_roms_first: self.copy_roms_first,
            execution_mode: InstallerExecutionMode::FullInstall,
        };

        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = cancel_flag.clone();
        let (tx, rx): (Sender<InstallerEvent>, Receiver<InstallerEvent>) = mpsc::channel();
        self.event_rx = Some(rx);

        self.is_running = true;
        self.install_summary = None;
        self.install_error = None;
        self.logs.clear();
        self.logs.push(format!("[INIT] Starting QuickInstaller for drive {} ({} copy workers, {} art workers)...", clean_drive, self.copy_threads, self.art_threads));

        std::thread::spawn(move || {
            InstallerEngine::run(config, cancel_flag, tx);
        });
    }

    pub fn start_sync_art_only(&mut self) {
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

        let (clean_drive, dest_path) = if let Some(drive) = self.drives.get(self.selected_drive_idx) {
            let clean = drive.letter.trim_end_matches('\\').trim_end_matches('/');
            let dest = if self.subfolder_name.trim().is_empty() {
                PathBuf::from(format!("{}\\", clean))
            } else {
                PathBuf::from(format!("{}\\{}", clean, self.subfolder_name.trim()))
            };
            (clean.to_string(), dest)
        } else {
            ("".to_string(), PathBuf::from("."))
        };

        let effective_art_types: Vec<ArtType> = if self.art_types.is_empty() {
            vec![ArtType::Boxart]
        } else {
            self.art_types.iter().copied().collect()
        };

        let config = InstallConfig {
            drive_letter: clean_drive,
            destination_path: dest_path,
            format_option: None,
            wipe_and_repartition: false,
            volume_label: self.volume_label.clone(),
            profile_id: self.selected_profile_id,
            platforms: platform_configs,
            download_art: true,
            art_types: effective_art_types,
            art_location_mode: self.art_location_mode,
            art_subfolder_name: self.art_subfolder_name.clone(),
            region_preference: if self.deduplicate_1g1r { self.region_preference } else { RegionPreference::None },
            exclude_betas: self.exclude_betas && self.deduplicate_1g1r,
            copy_threads: self.copy_threads,
            art_threads: self.art_threads,
            copy_roms_first: true,
            execution_mode: InstallerExecutionMode::ArtOnly,
        };

        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = cancel_flag.clone();
        let (tx, rx): (Sender<InstallerEvent>, Receiver<InstallerEvent>) = mpsc::channel();
        self.event_rx = Some(rx);

        self.is_running = true;
        self.install_summary = None;
        self.install_error = None;
        self.logs.clear();
        self.logs.push(format!("[INIT] Starting Standalone Boxart Sync ({:?}, {} threads)...", self.art_location_mode, self.art_threads));

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

        // Headless screenshot handling
        self.frame_count += 1;
        if let Some(ref path) = self.screenshot_path {
            ui.ctx().request_repaint();
            if self.frame_count == 3 {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            let save_path = path.clone();
            ui.input(|i| {
                for event in &i.raw.events {
                    if let egui::Event::Screenshot { image, .. } = event {
                        if let Err(e) = image::save_buffer(
                            &save_path,
                            image.as_raw(),
                            image.size[0] as u32,
                            image.size[1] as u32,
                            image::ExtendedColorType::Rgba8,
                        ) {
                            eprintln!("Failed to save screenshot: {}", e);
                            std::process::exit(1);
                        } else {
                            println!("SUCCESS: Screenshot saved to {}", save_path);
                            std::process::exit(0);
                        }
                    }
                }
            });
            if self.frame_count > 120 {
                eprintln!("Screenshot timeout reached at frame 120, closing.");
                std::process::exit(1);
            }
        }

        // 1. Padded Application Window Container
        egui::Frame::new()
            .inner_margin(Margin::symmetric(14, 10))
            .show(ui, |ui| {
                // Top Windows 11 App Header with Real Styled Tab Buttons
                self.render_top_header(ui);

                // 5-Step Chevron Ribbon with Real Buttons
                self.render_step_ribbon(ui);

                ui.add_space(6.0);

                // Main Workspace Area
                ScrollArea::vertical()
                    .id_salt("main_workspace_scroll")
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
            });

        // 2. Curated Favorites Modal Dialog
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
                    .inner_margin(Margin::symmetric(7, 5))
                    .show(ui, |ui| {
                        ui.label(RichText::new("🎮").size(16.0).color(Color32::WHITE));
                    });
                ui.add_space(4.0);

                ui.label(
                    RichText::new("Retro CardMaker")
                        .size(17.0)
                        .strong()
                        .color(Color32::WHITE),
                );

                render_badge(
                    ui,
                    "Windows 11",
                    Color32::from_rgb(18, 24, 34),
                    Color32::from_rgb(45, 58, 78),
                    Color32::from_gray(190),
                );

                ui.separator();
                ui.add_space(4.0);

                // Navigation Tabs: Real Interactive Buttons with Hover & Selection States
                let tabs = [
                    (ActiveTab::Dashboard, "⊞ Dashboard"),
                    (ActiveTab::Consoles, "🎮 Consoles"),
                    (ActiveTab::Favorites, "⭐ Favorites"),
                    (ActiveTab::Settings, "⚙ Settings"),
                ];

                for (tab, title) in tabs {
                    let is_active = self.active_tab == tab;
                    let text_color = if is_active {
                        Color32::from_rgb(0, 229, 255)
                    } else {
                        Color32::from_gray(190)
                    };
                    let bg = if is_active {
                        Color32::from_rgb(18, 32, 48)
                    } else {
                        Color32::from_rgb(13, 17, 25)
                    };
                    let stroke = Stroke::new(
                        1.0,
                        if is_active {
                            Color32::from_rgb(0, 229, 255)
                        } else {
                            Color32::from_rgb(30, 40, 56)
                        },
                    );

                    let btn = egui::Button::new(RichText::new(title).size(12.5).strong().color(text_color))
                        .fill(bg)
                        .stroke(stroke)
                        .corner_radius(6);

                    if ui.add(btn).clicked() {
                        self.active_tab = tab;
                    }
                    ui.add_space(3.0);
                }

                // Right-aligned status pill
                let avail = (ui.available_width() - 8.0).max(10.0);
                ui.allocate_ui(Vec2::new(avail, 26.0), |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let total_favs: usize = self
                            .platform_states
                            .iter()
                            .map(|p| p.favorites.as_ref().map(|f| f.games.len()).unwrap_or(0))
                            .sum();
                        render_badge(
                            ui,
                            &format!("👤 User • {} Favs", total_favs),
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
            });
            ui.add_space(5.0);
            ui.separator();
        });
    }

    fn render_step_ribbon(&mut self, ui: &mut egui::Ui) {
        ui.add_space(3.0);
        ui.columns(5, |cols| {
            let steps = [
                ("1", "1 · 💾 SD CARD & FORMAT", ActiveTab::Dashboard),
                ("2", "2 · 🎮 DEVICE PROFILE", ActiveTab::Dashboard),
                ("3", "3 · 📂 ROMS & FAVORITES", ActiveTab::Dashboard),
                ("4", "4 · 🖼 BOXART SCRAPING", ActiveTab::Dashboard),
                ("5", "5 · ⚡ QUICKINSTALL", ActiveTab::Dashboard),
            ];

            for (i, (_num, title, target_tab)) in steps.iter().enumerate() {
                let col = &mut cols[i];
                let is_active = self.wizard_focused_step == (i + 1);

                let text_color = if is_active {
                    Color32::BLACK
                } else {
                    Color32::from_gray(215)
                };
                let bg = if is_active {
                    Color32::from_rgb(0, 229, 255)
                } else {
                    Color32::from_rgb(15, 21, 31)
                };
                let stroke = Stroke::new(
                    1.0,
                    if is_active {
                        Color32::from_rgb(0, 229, 255)
                    } else {
                        Color32::from_rgb(30, 42, 60)
                    },
                );

                let btn = egui::Button::new(RichText::new(*title).size(12.0).strong().color(text_color))
                    .fill(bg)
                    .stroke(stroke)
                    .corner_radius(12);

                if col.add_sized([col.available_width(), 32.0], btn).clicked() {
                    self.wizard_focused_step = i + 1;
                    self.active_tab = *target_tab;
                }
            }
        });
        ui.add_space(5.0);
        ui.separator();
    }
}

// ----------------------------------------------------------------------------
// Master Studio Step-by-Step Wizard Engine (Steps 1 through 5)
// ----------------------------------------------------------------------------

fn render_wizard_navigation_footer(
    ui: &mut egui::Ui,
    back_label: Option<&str>,
    next_label: Option<&str>,
) -> (bool, bool) {
    let mut back_clicked = false;
    let mut next_clicked = false;

    ui.add_space(14.0);
    ui.separator();
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        if let Some(back_text) = back_label {
            let btn = egui::Button::new(
                RichText::new(back_text)
                    .size(13.0)
                    .strong()
                    .color(Color32::from_gray(215)),
            )
            .fill(Color32::from_rgb(18, 25, 36))
            .stroke(Stroke::new(1.0, Color32::from_rgb(38, 52, 74)))
            .corner_radius(6);

            if ui.add_sized([180.0, 36.0], btn).clicked() {
                back_clicked = true;
            }
        }

        let avail = (ui.available_width() - 8.0).max(10.0);
        ui.allocate_ui(Vec2::new(avail, 36.0), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(next_text) = next_label {
                    let btn = egui::Button::new(
                        RichText::new(next_text)
                            .size(13.5)
                            .strong()
                            .color(Color32::BLACK),
                    )
                    .fill(Color32::from_rgb(0, 229, 255))
                    .corner_radius(6);

                    if ui.add_sized([220.0, 36.0], btn).clicked() {
                        next_clicked = true;
                    }
                }
            });
        });
    });

    (back_clicked, next_clicked)
}

impl RetroCardMakerApp {
    fn render_dashboard_cockpit(&mut self, ui: &mut egui::Ui) {
        match self.wizard_focused_step {
            1 => self.render_step_1_sd_format(ui),
            2 => self.render_step_2_device_profile(ui),
            3 => self.render_step_3_roms_dedup(ui),
            4 => self.render_step_4_boxart_media(ui),
            5 => self.render_step_5_quickinstall(ui),
            _ => self.render_step_1_sd_format(ui),
        }
    }

    // ------------------------------------------------------------------------
    // Step 1: 💾 Target SD Card & Formatting
    // ------------------------------------------------------------------------
    fn render_step_1_sd_format(&mut self, ui: &mut egui::Ui) {
        card_frame().show(ui, |ui| {
            let selected_drive = self.drives.get(self.selected_drive_idx).cloned();
            let is_removable = selected_drive.as_ref().map(|d| d.is_removable).unwrap_or(false);
            let is_system = selected_drive.as_ref().map(|d| d.is_system).unwrap_or(false);
            let has_hidden = selected_drive.as_ref().map(|d| d.has_hidden_partitions).unwrap_or(false);
            let has_admin = is_elevated();

            render_card_header(ui, "Step 1 of 5: Target Storage Drive & SD Card Formatting", |ui| {
                if has_admin {
                    badge_green(ui, "🛡️ ADMIN");
                }
                if is_removable {
                    badge_cyan(ui, "💾 REMOVABLE");
                } else if is_system {
                    badge_red(ui, "🛡️ SYSTEM DRIVE");
                } else if self.drives.is_empty() {
                    badge_red(ui, "❌ NO DRIVE");
                } else {
                    badge_amber(ui, "⚠️ FIXED DISK");
                }
            });

            ui.label(
                RichText::new("Select your target SD card or USB storage drive. Format with recommended filesystems (FAT32 for legacy handhelds, exFAT for modern systems >32GB) and perform deep MBR clean wipes.")
                    .size(12.5)
                    .color(Color32::from_gray(180)),
            );
            ui.add_space(8.0);

            // Drive Selection Row
            ui.label(RichText::new("Target Storage Volume:").size(13.0).strong().color(Color32::WHITE));
            ui.horizontal(|ui| {
                let drive_display = if let Some(ref d) = selected_drive {
                    let label_str = if d.label.is_empty() { "NO NAME" } else { &d.label };
                    let type_str = if d.is_removable { "Removable" } else if d.is_system { "System" } else { "Fixed" };
                    format!("{}: {} {} ({}) - {}", d.letter.trim_end_matches('\\'), d.total_gb_str(), type_str, label_str, d.file_system)
                } else {
                    "No drive detected".to_string()
                };

                egui::ComboBox::from_id_salt("wizard_step1_drive_combo")
                    .width((ui.available_width() - 110.0).max(180.0))
                    .selected_text(RichText::new(&drive_display).size(13.5).strong().color(Color32::WHITE))
                    .show_ui(ui, |ui| {
                        for (idx, drive) in self.drives.iter().enumerate() {
                            let label_str = if drive.label.is_empty() { "NO NAME" } else { &drive.label };
                            let type_str = if drive.is_removable { "Removable" } else if drive.is_system { "System" } else { "Fixed" };
                            let text = format!("{}: {} {} ({}) - {}", drive.letter.trim_end_matches('\\'), drive.total_gb_str(), type_str, label_str, drive.file_system);
                            ui.selectable_value(&mut self.selected_drive_idx, idx, text);
                        }
                    });

                let ref_btn = egui::Button::new(RichText::new("🔄 Refresh").size(12.5).strong())
                    .fill(Color32::from_rgb(20, 28, 40))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(36, 50, 72)))
                    .corner_radius(6);
                if ui.add(ref_btn).clicked() {
                    self.refresh_drives();
                }
            });

            ui.add_space(10.0);

            // 2 Subcolumns: Hardware Status & Formatting
            ui.columns(2, |fcols| {
                // Left Subcolumn: Hardware & Safety Lock
                let left_ui = &mut fcols[0];
                left_ui.vertical(|ui| {
                    egui::Frame::new()
                        .fill(Color32::from_rgb(10, 14, 20))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(24, 34, 48)))
                        .corner_radius(6)
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            ui.label(RichText::new("Drive Hardware Status").size(12.5).strong().color(Color32::from_rgb(0, 229, 255)));
                            ui.add_space(4.0);

                            if let Some(ref d) = selected_drive {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("File System:").size(12.0).color(Color32::from_gray(160)));
                                    ui.label(RichText::new(&d.file_system).size(13.5).strong().color(Color32::WHITE));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Volume Size:").size(12.0).color(Color32::from_gray(160)));
                                    ui.label(RichText::new(d.total_gb_str()).size(13.0).strong().color(Color32::WHITE));
                                    ui.label(RichText::new(format!("({} free)", d.free_gb_str())).size(12.0).color(Color32::from_rgb(0, 229, 255)));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Hardware Type:").size(12.0).color(Color32::from_gray(160)));
                                    if d.is_removable {
                                        ui.colored_label(Color32::from_rgb(16, 185, 129), "Removable SD / USB Card (Unlocked)");
                                    } else if d.is_system {
                                        ui.colored_label(Color32::from_rgb(239, 68, 68), "System Drive (Strictly Protected)");
                                    } else {
                                        ui.colored_label(Color32::from_rgb(245, 158, 11), "Internal Fixed Disk (Protected)");
                                    }
                                });
                            } else {
                                ui.label(RichText::new("No storage drive selected. Insert an SD card or USB drive.").size(12.0).color(Color32::GRAY));
                            }

                            if has_hidden {
                                ui.add_space(6.0);
                                let phys_str = selected_drive.as_ref().map(|d| d.physical_gb_str()).unwrap_or_default();
                                let vol_str = selected_drive.as_ref().map(|d| d.total_gb_str()).unwrap_or_default();
                                egui::Frame::new()
                                    .fill(Color32::from_rgb(38, 26, 10))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(245, 158, 11)))
                                    .corner_radius(6)
                                    .inner_margin(Margin::same(8))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("⚠️").size(14.0));
                                            ui.label(RichText::new(format!("Hidden OEM / Linux partitions detected:\nVolume is {}, but physical card is {}.\nClean wipe recommended to recover full size!", vol_str, phys_str)).size(11.5).color(Color32::from_rgb(255, 200, 100)));
                                        });
                                    });
                            }
                        });
                });

                // Right Subcolumn: Formatting options
                let right_ui = &mut fcols[1];
                right_ui.vertical(|ui| {
                    egui::Frame::new()
                        .fill(Color32::from_rgb(10, 14, 20))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(24, 34, 48)))
                        .corner_radius(6)
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            ui.label(RichText::new("Formatting & Partitioning").size(12.5).strong().color(Color32::from_rgb(0, 229, 255)));
                            ui.add_space(4.0);

                            if !is_removable || is_system {
                                ui.colored_label(Color32::from_rgb(239, 68, 68), "Formatting is strictly locked for internal and system drives.");
                            } else {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Format Target Drive").size(13.0).color(Color32::WHITE));
                                    toggle_switch(ui, &mut self.do_format);
                                });

                                if self.do_format {
                                    ui.add_space(4.0);
                                    ui.horizontal(|ui| {
                                        let fat32_sel = self.format_fs == FormatFileSystem::Fat32;
                                        if ui.selectable_label(fat32_sel, "FAT32 (Legacy & ≤32GB)").clicked() {
                                            self.format_fs = FormatFileSystem::Fat32;
                                        }
                                        let exfat_sel = self.format_fs == FormatFileSystem::ExFat;
                                        if ui.selectable_label(exfat_sel, "exFAT (Modern & >32GB)").clicked() {
                                            self.format_fs = FormatFileSystem::ExFat;
                                        }
                                    });

                                    ui.add_space(3.0);
                                    let fs_desc = match self.format_fs {
                                        FormatFileSystem::Fat32 => "Recommended for GarlicOS, OnionOS, RG35XX & cards ≤32GB (4GB single-file limit).",
                                        FormatFileSystem::ExFat => "Recommended for modern devices & cards >32GB (supports PS2, PSP files >4GB).",
                                    };
                                    ui.label(RichText::new(fs_desc).size(11.0).color(Color32::from_rgb(0, 229, 255)));

                                    ui.add_space(6.0);
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Full Repartition & Clean Wipe (Diskpart MBR)").size(12.0).color(Color32::WHITE));
                                        toggle_switch(ui, &mut self.wipe_and_repartition);
                                        if has_hidden {
                                            badge_amber(ui, "Recommended");
                                        }
                                    });

                                    ui.add_space(4.0);
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Volume Label:").size(12.0).color(Color32::from_gray(180)));
                                        ui.add(egui::TextEdit::singleline(&mut self.volume_label).desired_width(100.0));
                                        ui.checkbox(&mut self.format_confirmed, RichText::new("Confirm Erase").size(12.0).color(Color32::from_rgb(255, 180, 100)));
                                    });

                                    if !has_admin {
                                        ui.add_space(4.0);
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("⚠️ Requires Admin:").size(11.5).color(Color32::from_rgb(255, 180, 100)));
                                            let el_btn = egui::Button::new(RichText::new("🛡️ Relaunch as Admin").size(11.0).strong().color(Color32::BLACK))
                                                .fill(Color32::from_rgb(245, 158, 11))
                                                .corner_radius(4);
                                            if ui.add(el_btn).clicked() {
                                                let _ = relaunch_as_admin();
                                            }
                                        });
                                    }
                                }
                            }
                        });
                });
            });

            // Step 1 Navigation Footer
            let (_, next) = render_wizard_navigation_footer(ui, None, Some("Next: Device Profile >"));
            if next {
                self.wizard_focused_step = 2;
            }
        });
    }

    // ------------------------------------------------------------------------
    // Step 2: 🎮 Handheld Launcher & Device Profile
    // ------------------------------------------------------------------------
    fn render_step_2_device_profile(&mut self, ui: &mut egui::Ui) {
        card_frame().show(ui, |ui| {
            let active_profile = LauncherProfile::get_by_id(self.selected_profile_id);
            render_card_header(ui, "Step 2 of 5: Handheld Launcher & Device Profile", |ui| {
                badge_purple(ui, &format!("🕹️ {}", active_profile.name));
            });

            ui.label(
                RichText::new("Choose your handheld OS or frontend. Retro CardMaker automatically maps folder names (e.g. gbc, gba, mastersystem, megadrive, saturn, psx) and artwork directories to match device specifications.")
                    .size(12.5)
                    .color(Color32::from_gray(180)),
            );
            ui.add_space(8.0);

            // Horizontal Profile Cards Grid with Modern Interactive Cards
            ScrollArea::horizontal()
                .id_salt("wizard_step2_profiles_scroll")
                .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for profile in PROFILES.iter() {
                        let is_sel = self.selected_profile_id == profile.id;

                        let icon = match profile.id {
                            ProfileId::AnbernicRgDsLauncher => "🎮",
                            ProfileId::EmulationStationEsDe => "🖥️",
                            ProfileId::RetroArch => "👾",
                            ProfileId::AnbernicOlderGarlicOs => "🕹️",
                            ProfileId::MiyooMiniOnionOs => "📱",
                            ProfileId::Daijisho => "📱",
                            ProfileId::Pegasus => "⭐",
                            ProfileId::BatoceraArkOs => "🐧",
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

                        let frame = egui::Frame::new()
                            .fill(if is_sel { Color32::from_rgb(18, 32, 48) } else { Color32::from_rgb(12, 16, 24) })
                            .stroke(Stroke::new(if is_sel { 1.5 } else { 1.0 }, if is_sel { Color32::from_rgb(0, 229, 255) } else { Color32::from_rgb(28, 38, 54) }))
                            .corner_radius(8)
                            .inner_margin(Margin::symmetric(10, 8));

                        let resp = frame.show(ui, |ui| {
                            ui.set_width(120.0);
                            ui.set_height(72.0);
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    let radio_str = if is_sel { "●" } else { "○" };
                                    let radio_color = if is_sel { Color32::from_rgb(0, 229, 255) } else { Color32::from_gray(140) };
                                    ui.label(RichText::new(radio_str).size(12.0).color(radio_color));
                                    ui.label(RichText::new(icon).size(16.0));
                                });
                                ui.add_space(2.0);
                                ui.label(RichText::new(short_name).size(11.5).strong().color(if is_sel { Color32::WHITE } else { Color32::from_gray(190) }));
                                ui.label(RichText::new(format!("{}/", profile.recommended_sd_subfolder)).size(11.0).color(if is_sel { Color32::from_rgb(0, 229, 255) } else { Color32::from_gray(130) }));
                            });
                        }).response;

                        let id = ui.id().with(format!("prof_btn_{}", profile.name));
                        if ui.interact(resp.rect, id, egui::Sense::click()).clicked() {
                            self.selected_profile_id = profile.id;
                            self.subfolder_name = profile.recommended_sd_subfolder.to_string();
                        }

                        ui.add_space(6.0);
                    }
                });
            });

            ui.add_space(10.0);

            // Profile Details & Directory Mapping Card
            egui::Frame::new()
                .fill(Color32::from_rgb(10, 14, 20))
                .stroke(Stroke::new(1.0, Color32::from_rgb(24, 34, 48)))
                .corner_radius(6)
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ui.columns(2, |pcols| {
                        // Left: Target Subfolder & Paths
                        let left = &mut pcols[0];
                        left.vertical(|ui| {
                            ui.label(RichText::new("Target SD Subfolder Configuration").size(12.5).strong().color(Color32::from_rgb(0, 229, 255)));
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("SD Root Folder:").size(12.5).color(Color32::from_gray(180)));
                                ui.add(egui::TextEdit::singleline(&mut self.subfolder_name).desired_width(120.0));
                                ui.label(RichText::new("(e.g. 'Roms', 'roms', or empty for root)").size(11.0).color(Color32::GRAY));
                            });
                            ui.add_space(6.0);
                            ui.label(RichText::new("Media Directory Routing:").size(12.0).strong().color(Color32::WHITE));
                            let (art_box, art_snap, art_title) = match self.selected_profile_id {
                                ProfileId::EmulationStationEsDe => ("downloaded_media/<system>/covers/", "downloaded_media/<system>/screenshots/", "downloaded_media/<system>/titles/"),
                                ProfileId::RetroArch => ("thumbnails/<system>/Named_Boxarts/", "thumbnails/<system>/Named_Snaps/", "thumbnails/<system>/Named_Titles/"),
                                ProfileId::BatoceraArkOs => ("<system>/images/<rom>-image.png", "<system>/images/<rom>-snap.png", "<system>/images/<rom>-title.png"),
                                _ => ("<system>/Imgs/<rom>.png", "<system>/snaps/<rom>.png", "<system>/titles/<rom>.png"),
                            };
                            ui.label(RichText::new(format!("• Boxart (Covers): {}", art_box)).size(11.0).color(Color32::from_rgb(16, 185, 129)));
                            ui.label(RichText::new(format!("• Screenshots (Snaps): {}", art_snap)).size(11.0).color(Color32::from_rgb(0, 229, 255)));
                            ui.label(RichText::new(format!("• Title Screens: {}", art_title)).size(11.0).color(Color32::from_rgb(168, 130, 255)));
                        });

                        // Right: Console Folder Mapping Table Preview
                        let right = &mut pcols[1];
                        right.vertical(|ui| {
                            ui.label(RichText::new("Console Folder Mapping Matrix").size(12.5).strong().color(Color32::from_rgb(0, 229, 255)));
                            ui.add_space(4.0);
                            let sample_ids = [
                                "gbc",
                                "gba",
                                "sms",
                                "megadrive",
                                "gamegear",
                                "saturn",
                                "dreamcast",
                                "psx",
                                "nds",
                                "nes",
                                "snes",
                                "n64",
                            ];
                            for sys_id in sample_ids {
                                if let Some(plat) = crate::platforms::find_platform_by_id(sys_id) {
                                    let dest_folder = active_profile.get_platform_folder(plat);
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(plat.name).size(11.0).color(Color32::from_gray(170)));
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.label(RichText::new(format!("-> {}/", dest_folder)).size(11.0).strong().color(Color32::WHITE));
                                        });
                                    });
                                }
                            }
                        });
                    });
                });

            // Step 2 Navigation Footer
            let (back, next) = render_wizard_navigation_footer(ui, Some("< Back: SD Card & Format"), Some("Next: ROMs & Deduplication >"));
            if back {
                self.wizard_focused_step = 1;
            }
            if next {
                self.wizard_focused_step = 3;
            }
        });
    }

    // ------------------------------------------------------------------------
    // Step 3: 📂 ROMs Library & 1G1R Deduplication
    // ------------------------------------------------------------------------
    fn render_step_3_roms_dedup(&mut self, ui: &mut egui::Ui) {
        card_frame().show(ui, |ui| {
            let total_raw: usize = self.platform_states.iter().map(|p| p.rom_files.len()).sum();
            let total_filtered: usize = self.platform_states.iter().map(|p| {
                if self.deduplicate_1g1r {
                    filter_roms_1g1r(&p.rom_files, self.region_preference, self.exclude_betas).kept_files.len()
                } else {
                    p.rom_files.len()
                }
            }).sum();

            render_card_header(ui, "Step 3 of 5: ROMs Selection & 1G1R Deduplication", |ui| {
                if self.deduplicate_1g1r && total_raw > total_filtered {
                    badge_purple(ui, &format!("1G1R: {} Games ({} clones filtered)", total_filtered, total_raw - total_filtered));
                } else {
                    badge_cyan(ui, &format!("{} ROMs Loaded", total_raw));
                }
            });

            // 1. Source Folder Picker
            ui.horizontal(|ui| {
                ui.label(RichText::new("ROMs Source Library:").size(12.5).strong().color(Color32::WHITE));
                ui.add(egui::TextEdit::singleline(&mut self.source_path).desired_width((ui.available_width() - 170.0).max(120.0)));

                let browse_btn = egui::Button::new(RichText::new("📂 Browse").size(12.0).strong())
                    .fill(Color32::from_rgb(20, 28, 40))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(36, 50, 72)))
                    .corner_radius(6);
                if ui.add(browse_btn).clicked() {
                    let mut dialog = rfd::FileDialog::new().set_title("Select ROM Library Directory");
                    if !self.source_path.trim().is_empty() && Path::new(&self.source_path).is_dir() {
                        dialog = dialog.set_directory(&self.source_path);
                    }
                    if let Some(path) = dialog.pick_folder() {
                        self.source_path = path.to_string_lossy().to_string();
                        self.scan_source_folder();
                    }
                }

                let scan_btn = egui::Button::new(RichText::new("🔄 Scan").size(12.0).strong().color(Color32::BLACK))
                    .fill(Color32::from_rgb(0, 229, 255))
                    .corner_radius(6);
                if ui.add(scan_btn).clicked() {
                    self.scan_source_folder();
                }
            });

            ui.add_space(8.0);

            // 2. 1G1R Deduplication & Region Preference Panel
            egui::Frame::new()
                .fill(Color32::from_rgb(10, 15, 24))
                .stroke(Stroke::new(1.0, Color32::from_rgb(30, 44, 66)))
                .corner_radius(6)
                .inner_margin(Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("1G1R Logical Deduplication (One Game, One ROM)").size(13.0).strong().color(Color32::WHITE));
                        toggle_switch(ui, &mut self.deduplicate_1g1r);
                        if self.deduplicate_1g1r {
                            badge_green(ui, "Active");
                        }
                    });

                    if self.deduplicate_1g1r {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Preferred Region:").size(12.0).color(Color32::from_gray(170)));
                            let prefs = [
                                (RegionPreference::UsaFirst, "🇺🇸 USA (NTSC-U)"),
                                (RegionPreference::EuropeFirst, "🇪🇺 Europe (PAL)"),
                                (RegionPreference::JapanFirst, "🇯🇵 Japan (NTSC-J)"),
                                (RegionPreference::WorldFirst, "🌐 World / Global"),
                                (RegionPreference::None, "🚫 Keep All"),
                            ];
                            for (pref, label) in prefs {
                                let is_sel = self.region_preference == pref;
                                if ui.selectable_label(is_sel, label).clicked() {
                                    self.region_preference = pref;
                                }
                            }
                        });

                        ui.add_space(3.0);
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.exclude_betas, RichText::new("Exclude Betas, Prototypes, Demos & Samples").size(12.0).color(Color32::from_gray(210)));
                            ui.label(RichText::new("(Keeps highest official revision e.g. Rev 1, Rev 2 & verified clean dumps [!])").size(11.0).color(Color32::from_gray(140)));
                        });
                    }
                });

            ui.add_space(8.0);

            // 3. Two Columns: Consoles Selection (Left) + Live ROMs Preview (Right)
            ui.columns(2, |cols| {
                // Left: Consoles Selection
                let left = &mut cols[0];
                left.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Installed Consoles").size(13.0).strong().color(Color32::WHITE));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let none_btn = egui::Button::new(RichText::new("None").size(11.0))
                                .fill(Color32::from_rgb(18, 25, 36))
                                .corner_radius(4);
                            if ui.add(none_btn).clicked() {
                                for p in &mut self.platform_states { p.enabled = false; }
                            }
                            let all_btn = egui::Button::new(RichText::new("All").size(11.0))
                                .fill(Color32::from_rgb(18, 25, 36))
                                .corner_radius(4);
                            if ui.add(all_btn).clicked() {
                                for p in &mut self.platform_states { p.enabled = true; }
                            }
                        });
                    });

                    ui.add_space(4.0);

                    ScrollArea::vertical()
                        .id_salt("wizard_step3_platforms_scroll")
                        .max_height(250.0)
                        .show(ui, |ui| {
                            let mut open_idx = None;
                            for idx in 0..self.platform_states.len() {
                                let is_selected_for_preview = self.selected_platform_idx == idx;
                                let p_state = &mut self.platform_states[idx];
                                let is_found = p_state.found_dir.is_some();
                                let raw_count = p_state.rom_files.len();

                                let (display_count, filtered_count) = if self.deduplicate_1g1r {
                                    let res = filter_roms_1g1r(&p_state.rom_files, self.region_preference, self.exclude_betas);
                                    (res.kept_files.len(), res.duplicates_filtered + res.betas_filtered)
                                } else {
                                    (raw_count, 0)
                                };

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

                                let plat_name = p_state.platform.name;

                                let inner = ui.push_id(format!("step3_plat_{}", idx), |ui| {
                                    frame.show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.checkbox(&mut p_state.enabled, "");
                                            ui.label(RichText::new(p_icon).size(16.0));

                                            let avail = (ui.available_width() - 140.0).max(80.0);
                                            ui.allocate_ui(Vec2::new(avail, 32.0), |ui| {
                                                ui.vertical(|ui| {
                                                    ui.add(egui::Label::new(RichText::new(plat_name).size(12.5).strong().color(if is_found { Color32::WHITE } else { Color32::from_gray(130) })).truncate());
                                                    let count_str = if filtered_count > 0 {
                                                        format!("{} ROMs (-{} dupes)", display_count, filtered_count)
                                                    } else {
                                                        format!("{} ROMs", display_count)
                                                    };
                                                    ui.label(RichText::new(count_str).size(11.0).color(if display_count > 0 { Color32::from_rgb(0, 229, 255) } else { Color32::from_gray(120) }));
                                                });
                                            });

                                            if curated_favorites_button(ui) {
                                                open_idx = Some(idx);
                                            }
                                        });
                                    })
                                }).inner;

                                let row_id = inner.response.id.with(format!("plat_click_{}", idx));
                                if ui.interact(inner.response.rect, row_id, egui::Sense::click()).clicked() {
                                    self.selected_platform_idx = idx;
                                }

                                ui.add_space(3.0);
                            }

                            if let Some(idx) = open_idx {
                                self.open_favorites_editor(idx);
                            }
                        });
                });

                // Right: Live ROMs Preview & Search
                let right = &mut cols[1];
                right.vertical(|ui| {
                    let active_plat = self.platform_states.get(self.selected_platform_idx).cloned();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔍").size(12.0));
                        ui.add(egui::TextEdit::singleline(&mut self.rom_search_query).desired_width(140.0));
                        if let Some(ref p) = active_plat {
                            badge_cyan(ui, p.platform.name);
                        }
                    });

                    ui.add_space(4.0);

                    ScrollArea::vertical()
                        .id_salt("wizard_step3_roms_scroll")
                        .max_height(250.0)
                        .show(ui, |ui| {
                            if let Some(ref p_state) = active_plat {
                                let (roms_to_show, _) = if self.deduplicate_1g1r {
                                    let res = filter_roms_1g1r(&p_state.rom_files, self.region_preference, self.exclude_betas);
                                    (res.kept_files, res.duplicates_filtered)
                                } else {
                                    (p_state.rom_files.clone(), 0)
                                };

                                let search_lower = self.rom_search_query.to_lowercase();
                                let top_list = p_state.platform.default_favorites;
                                let fav_set = p_state.favorites.as_ref().map(|f| f.games.iter().collect::<HashSet<_>>());

                                let mut shown = 0;
                                for rom in &roms_to_show {
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
                                                ui.label(RichText::new(get_game_icon(rom)).size(14.0));
                                                let avail = (ui.available_width() - 95.0).max(60.0);
                                                ui.allocate_ui(Vec2::new(avail, 28.0), |ui| {
                                                    ui.vertical(|ui| {
                                                        ui.add(egui::Label::new(RichText::new(rom).size(12.0).strong().color(Color32::WHITE)).truncate());
                                                    });
                                                });

                                                if is_top {
                                                    badge_green(ui, "Top");
                                                } else if is_fav {
                                                    badge_purple(ui, "Curated");
                                                } else {
                                                    badge_cyan(ui, "ROM");
                                                }
                                            });
                                        });
                                    ui.add_space(2.0);
                                }

                                if shown == 0 {
                                    ui.label(RichText::new("No ROMs match the filter.").size(12.0).color(Color32::GRAY));
                                }
                            } else {
                                ui.label(RichText::new("No console selected.").size(12.0).color(Color32::GRAY));
                            }
                        });

                    ui.add_space(2.0);
                    ui.label(RichText::new("ℹ️ Multi-track PS1 (.bin/.cue) & CD images are automatically unified.").size(11.0).color(Color32::from_gray(140)));
                });
            });

            // Step 3 Navigation Footer
            let (back, next) = render_wizard_navigation_footer(ui, Some("< Back: Device Profile"), Some("Next: Boxart Scraping >"));
            if back {
                self.wizard_focused_step = 2;
            }
            if next {
                self.wizard_focused_step = 4;
            }
        });
    }

    // ------------------------------------------------------------------------
    // Step 4: 🖼 Boxart & Media Scraping (With Standalone Downloader)
    // ------------------------------------------------------------------------
    fn render_step_4_boxart_media(&mut self, ui: &mut egui::Ui) {
        card_frame().show(ui, |ui| {
            render_card_header(ui, "Step 4 of 5: Boxart & Media Scraping", |ui| {
                badge_green(ui, "🌐 Libretro CDN Online");
            });

            ui.label(
                RichText::new("Scrape official high-resolution front cover boxart, action gameplay screenshots, and title screens from the Libretro No-Intro database. Download directly to PC or sync to SD card.")
                    .size(12.5)
                    .color(Color32::from_gray(180)),
            );
            ui.add_space(8.0);

            // Master Toggle
            ui.horizontal(|ui| {
                ui.label(RichText::new("Enable Boxart & Media Scraping").size(13.5).strong().color(Color32::WHITE));
                toggle_switch(ui, &mut self.download_art);
            });

            ui.add_space(8.0);

            // Media Sets Multi-Selection
            ui.label(RichText::new("Media Sets to Download:").size(13.0).strong().color(Color32::WHITE));
            ui.horizontal(|ui| {
                let media_options = [
                    (ArtType::Boxart, "🖼 Front Boxart (Covers)", "Official front cover box art for launcher menus."),
                    (ArtType::Screenshot, "📸 Screenshots (Snaps)", "In-game action gameplay screenshots."),
                    (ArtType::TitleScreen, "🎮 Title Screens", "Game intro logos and start screens."),
                ];

                for (art_type, title, desc) in media_options {
                    let mut is_checked = self.art_types.contains(&art_type);
                    let frame = egui::Frame::new()
                        .fill(if is_checked { Color32::from_rgb(16, 28, 42) } else { Color32::from_rgb(10, 14, 20) })
                        .stroke(Stroke::new(if is_checked { 1.2 } else { 1.0 }, if is_checked { Color32::from_rgb(0, 229, 255) } else { Color32::from_rgb(26, 36, 50) }))
                        .corner_radius(6)
                        .inner_margin(Margin::same(10));

                    frame.show(ui, |ui| {
                        ui.set_width(210.0);
                        ui.vertical(|ui| {
                            if ui.checkbox(&mut is_checked, RichText::new(title).size(12.5).strong().color(Color32::WHITE)).changed() {
                                if is_checked {
                                    self.art_types.insert(art_type);
                                } else {
                                    self.art_types.remove(&art_type);
                                }
                            }
                            ui.label(RichText::new(desc).size(11.0).color(Color32::from_gray(150)));
                        });
                    });
                    ui.add_space(6.0);
                }
            });

            ui.add_space(10.0);

            // Storage Location & Concurrency Configuration
            egui::Frame::new()
                .fill(Color32::from_rgb(10, 14, 20))
                .stroke(Stroke::new(1.0, Color32::from_rgb(24, 34, 48)))
                .corner_radius(6)
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ui.columns(2, |cols| {
                        // Left: Storage Location
                        let left = &mut cols[0];
                        left.vertical(|ui| {
                            ui.label(RichText::new("Artwork Destination & Folders").size(12.5).strong().color(Color32::from_rgb(0, 229, 255)));
                            ui.add_space(4.0);

                            ui.horizontal(|ui| {
                                let is_sub = self.art_location_mode == ArtLocationMode::RomSourceSubfolder;
                                let is_temp = self.art_location_mode == ArtLocationMode::TempCacheFolder;
                                let is_sd = self.art_location_mode == ArtLocationMode::TargetDriveOnly;

                                if ui.selectable_label(is_sub, "📂 Source Subfolder").clicked() {
                                    self.art_location_mode = ArtLocationMode::RomSourceSubfolder;
                                }
                                if ui.selectable_label(is_temp, "⚡ Temp Cache").clicked() {
                                    self.art_location_mode = ArtLocationMode::TempCacheFolder;
                                }
                                if ui.selectable_label(is_sd, "📁 SD Card Only").clicked() {
                                    self.art_location_mode = ArtLocationMode::TargetDriveOnly;
                                }
                            });

                            ui.add_space(4.0);
                            match self.art_location_mode {
                                ArtLocationMode::RomSourceSubfolder => {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Subfolder:").size(11.5).color(Color32::from_gray(160)));
                                        for name in &["Imgs", "covers", "boxart", "media"] {
                                            if ui.selectable_label(self.art_subfolder_name == *name, *name).clicked() {
                                                self.art_subfolder_name = name.to_string();
                                            }
                                        }
                                        ui.add(egui::TextEdit::singleline(&mut self.art_subfolder_name).desired_width(70.0));
                                    });
                                    ui.label(RichText::new("(Artwork stays permanently alongside ROMs on PC and syncs to SD)").size(11.0).color(Color32::from_rgb(16, 185, 129)));
                                }
                                ArtLocationMode::TempCacheFolder => {
                                    ui.label(RichText::new("Cached in %TEMP%\\retro-cardmaker\\art_cache").size(11.5).color(Color32::from_rgb(0, 229, 255)));
                                }
                                ArtLocationMode::TargetDriveOnly => {
                                    ui.label(RichText::new("Saves directly to SD card launcher paths only.").size(11.5).color(Color32::from_gray(160)));
                                }
                            }
                        });

                        // Right: Concurrency Workers
                        let right = &mut cols[1];
                        right.vertical(|ui| {
                            ui.label(RichText::new("Download Acceleration & Workers").size(12.5).strong().color(Color32::from_rgb(0, 229, 255)));
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Artwork Download Workers:").size(12.0).color(Color32::from_gray(180)));
                                for w in [2, 4, 6, 8, 12] {
                                    if ui.selectable_label(self.art_threads == w, w.to_string()).clicked() {
                                        self.art_threads = w;
                                    }
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("SD File Copy Workers:").size(12.0).color(Color32::from_gray(180)));
                                for w in [2, 4, 8] {
                                    if ui.selectable_label(self.copy_threads == w, w.to_string()).clicked() {
                                        self.copy_threads = w;
                                    }
                                }
                            });
                        });
                    });
                });

            ui.add_space(10.0);

            // STANDALONE ACTION CARD (Addresses user requirement to download separately without SD card!)
            egui::Frame::new()
                .fill(Color32::from_rgb(10, 22, 34))
                .stroke(Stroke::new(1.2, Color32::from_rgb(0, 229, 255)))
                .corner_radius(8)
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("📥").size(24.0));
                        ui.vertical(|ui| {
                            ui.label(RichText::new("Download Boxart & Media Now (Without SD Card)").size(13.5).strong().color(Color32::WHITE));
                            ui.label(RichText::new("Scrapes all selected artwork directly to your local PC storage or temp cache immediately. No SD card required.").size(11.5).color(Color32::from_gray(180)));
                        });

                        let avail = (ui.available_width() - 8.0).max(10.0);
                        ui.allocate_ui(Vec2::new(avail, 36.0), |ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let can_dl = !self.is_running && self.platform_states.iter().any(|p| p.enabled && p.found_dir.is_some());
                                let dl_btn = egui::Button::new(
                                    RichText::new("🚀 DOWNLOAD BOXART & MEDIA NOW")
                                        .size(12.5)
                                        .strong()
                                        .color(if can_dl { Color32::BLACK } else { Color32::from_gray(140) }),
                                )
                                .fill(if can_dl { Color32::from_rgb(0, 229, 255) } else { Color32::from_rgb(20, 30, 42) })
                                .corner_radius(6);

                                if ui.add_sized([290.0, 34.0], dl_btn).clicked() && can_dl {
                                    self.start_sync_art_only();
                                }
                            });
                        });
                    });

                    if self.is_running && self.current_phase.contains("Art") {
                        ui.add_space(8.0);
                        let frac = if self.progress_total > 0 { self.progress_current as f32 / self.progress_total as f32 } else { 0.0 };
                        let extra = format!("{}/{} ({:.0}%)", self.progress_current, self.progress_total, frac * 100.0);
                        render_neon_progress_bar(ui, "DOWNLOADING MEDIA", frac, &extra);
                    }
                });

            // Step 4 Navigation Footer
            let (back, next) = render_wizard_navigation_footer(ui, Some("< Back: ROMs & Deduplication"), Some("Next: QuickInstall Cockpit >"));
            if back {
                self.wizard_focused_step = 3;
            }
            if next {
                self.wizard_focused_step = 5;
            }
        });
    }

    // ------------------------------------------------------------------------
    // Step 5: ⚡ QuickInstall Cockpit & Execution
    // ------------------------------------------------------------------------
    fn render_step_5_quickinstall(&mut self, ui: &mut egui::Ui) {
        card_frame().show(ui, |ui| {
            render_card_header(ui, "Step 5 of 5: QuickInstall Cockpit & Execution", |ui| {
                if self.is_running {
                    badge_amber(ui, "🚀 EXECUTING");
                } else if self.install_summary.is_some() {
                    badge_green(ui, "✓ COMPLETE");
                } else {
                    badge_green(ui, "READY");
                }
            });

            // 1. Pre-flight Summary Card (4 metrics)
            egui::Frame::new()
                .fill(Color32::from_rgb(10, 14, 20))
                .stroke(Stroke::new(1.0, Color32::from_rgb(24, 34, 48)))
                .corner_radius(6)
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ui.columns(4, |cols| {
                        // 1. Target Drive
                        cols[0].vertical(|ui| {
                            ui.label(RichText::new("Target Drive").size(11.5).color(Color32::from_gray(160)));
                            let drive_str = self.drives.get(self.selected_drive_idx).map(|d| format!("{} ({})", d.letter, d.total_gb_str())).unwrap_or_else(|| "None".to_string());
                            ui.label(RichText::new(drive_str).size(13.0).strong().color(Color32::WHITE));
                            let fmt_str = if self.do_format { format!("Format: {:?}", self.format_fs) } else { "Keep existing".to_string() };
                            ui.label(RichText::new(fmt_str).size(11.0).color(Color32::from_rgb(0, 229, 255)));
                        });

                        // 2. Profile
                        cols[1].vertical(|ui| {
                            ui.label(RichText::new("Device Profile").size(11.5).color(Color32::from_gray(160)));
                            let prof = LauncherProfile::get_by_id(self.selected_profile_id);
                            ui.label(RichText::new(prof.name).size(13.0).strong().color(Color32::WHITE));
                            ui.label(RichText::new(format!("Subfolder: {}/", self.subfolder_name)).size(11.0).color(Color32::from_rgb(168, 130, 255)));
                        });

                        // 3. Consoles & Games
                        cols[2].vertical(|ui| {
                            ui.label(RichText::new("Consoles & Games").size(11.5).color(Color32::from_gray(160)));
                            let enabled_count = self.platform_states.iter().filter(|p| p.enabled && p.found_dir.is_some()).count();
                            let total_games: usize = self.platform_states.iter().filter(|p| p.enabled).map(|p| {
                                if self.deduplicate_1g1r {
                                    filter_roms_1g1r(&p.rom_files, self.region_preference, self.exclude_betas).kept_files.len()
                                } else {
                                    p.rom_files.len()
                                }
                            }).sum();
                            ui.label(RichText::new(format!("{} Systems", enabled_count)).size(13.0).strong().color(Color32::WHITE));
                            let dedup_tag = if self.deduplicate_1g1r { "1G1R Filtered" } else { "All Clones" };
                            ui.label(RichText::new(format!("{} Games ({})", total_games, dedup_tag)).size(11.0).color(Color32::from_rgb(16, 185, 129)));
                        });

                        // 4. Media & Transfer Mode
                        cols[3].vertical(|ui| {
                            ui.label(RichText::new("Media & Scraping").size(11.5).color(Color32::from_gray(160)));
                            let media_count = self.art_types.len();
                            let media_str = if !self.download_art { "Disabled".to_string() } else { format!("{} Media Types", media_count) };
                            ui.label(RichText::new(media_str).size(13.0).strong().color(Color32::WHITE));
                            ui.label(RichText::new(format!("{} Art Threads", self.art_threads)).size(11.0).color(Color32::from_rgb(0, 229, 255)));
                        });
                    });

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.copy_roms_first, RichText::new("Copy all ROMs first, then download artwork").size(12.0).color(Color32::from_gray(210)));
                        ui.label(RichText::new("(Maximizes sequential write speeds on removable SD flash memory)").size(11.0).color(Color32::GRAY));
                    });
                });

            ui.add_space(8.0);

            // 2. Dual Progress Bars
            let sync_frac = if self.platform_states.is_empty() { 0.0 } else {
                let enabled_count = self.platform_states.iter().filter(|p| p.enabled && p.found_dir.is_some()).count();
                if enabled_count == 0 { 0.0 } else { 1.0 }
            };
            let sync_text = format!("{:.0}%", sync_frac * 100.0);
            render_neon_progress_bar(ui, "PLATFORM & PROFILE SYNC", sync_frac, &sync_text);

            let (bar_label, copy_frac, copy_text) = if self.current_phase.contains("Art") || self.current_phase.contains("Boxart") {
                let f = if self.progress_total > 0 { self.progress_current as f32 / self.progress_total as f32 } else { 0.0 };
                let t = if self.progress_total > 0 { format!("{}/{} ({:.0}%)", self.progress_current, self.progress_total, f * 100.0) } else { "0/0 (0%)".to_string() };
                (format!("MEDIA SCRAPING ({} workers)", self.art_threads), f, t)
            } else {
                let f = if self.progress_total > 0 { self.progress_current as f32 / self.progress_total as f32 } else { 0.0 };
                let t = if self.progress_total > 0 { format!("{}/{} ({:.0}%)", self.progress_current, self.progress_total, f * 100.0) } else { "0/0 (0%)".to_string() };
                (format!("ROM FILE TRANSFER ({} workers)", self.copy_threads), f, t)
            };
            render_neon_progress_bar(ui, &bar_label, copy_frac, &copy_text);

            let item_label = if self.current_item.is_empty() {
                if self.is_running { "Transferring game assets..." } else { "System idling. Ready for quickinstall." }
            } else {
                &self.current_item
            };
            ui.label(RichText::new(item_label).size(12.0).color(Color32::from_gray(160)));

            ui.add_space(8.0);

            // 3. Action Buttons Row
            if !self.is_running {
                let has_admin = is_elevated();
                let can_start = !self.drives.is_empty()
                    && (!self.do_format || (self.format_confirmed && has_admin))
                    && self.platform_states.iter().any(|p| p.enabled && p.found_dir.is_some());
                let can_sync_art = self.platform_states.iter().any(|p| p.enabled && p.found_dir.is_some());

                let target_letter = self.drives.get(self.selected_drive_idx).map(|d| d.letter.clone()).unwrap_or_else(|| "E:".to_string());

                let mut do_start = false;
                let mut do_sync_art = false;

                ui.columns(2, |bcols| {
                    let start_btn = egui::Button::new(
                        RichText::new(format!("🚀 START COMPLETE QUICKINSTALL ({})", target_letter))
                            .size(14.0)
                            .strong()
                            .color(if can_start { Color32::BLACK } else { Color32::from_gray(140) }),
                    )
                    .fill(if can_start { Color32::from_rgb(16, 185, 129) } else { Color32::from_rgb(20, 30, 42) })
                    .corner_radius(8);

                    if bcols[0].add_sized([bcols[0].available_width(), 42.0], start_btn).clicked() && can_start {
                        do_start = true;
                    }

                    let sync_art_btn = egui::Button::new(
                        RichText::new("📥 DOWNLOAD BOXART ONLY")
                            .size(13.0)
                            .strong()
                            .color(Color32::from_rgb(0, 229, 255)),
                    )
                    .fill(Color32::from_rgb(0, 28, 42))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(0, 180, 210)))
                    .corner_radius(8);

                    if bcols[1].add_sized([bcols[1].available_width(), 42.0], sync_art_btn).clicked() && can_sync_art {
                        do_sync_art = true;
                    }
                });

                if do_start {
                    self.start_install();
                }
                if do_sync_art {
                    self.start_sync_art_only();
                }

                if !can_start {
                    ui.add_space(3.0);
                    let warning_str = if self.do_format && !has_admin {
                        "Administrator privileges required to format. Click 'Relaunch as Admin' in Step 1."
                    } else if self.do_format && !self.format_confirmed {
                        "Format is enabled: Check 'Confirm Erase' in Step 1 to proceed."
                    } else {
                        "Verify that a removable drive is selected and ROM source is configured."
                    };
                    ui.label(RichText::new(warning_str).size(11.5).color(Color32::from_rgb(255, 180, 100)));
                }
            } else {
                let cancel_btn = egui::Button::new(
                    RichText::new("🛑 CANCEL QUICKINSTALL")
                        .size(14.0)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(Color32::from_rgb(239, 68, 68))
                .corner_radius(8);

                if ui.add_sized([ui.available_width(), 42.0], cancel_btn).clicked() {
                    self.cancel_flag.store(true, Ordering::Relaxed);
                    self.logs.push("[CANCEL] Cancellation requested by user.".to_string());
                }
            }

            ui.add_space(10.0);

            // 4. Live Terminal Console (`terminal.log`)
            egui::Frame::new()
                .fill(Color32::from_rgb(6, 9, 14))
                .stroke(Stroke::new(1.0, Color32::from_rgb(26, 36, 50)))
                .corner_radius(8)
                .inner_margin(Margin::same(0))
                .show(ui, |ui| {
                    egui::Frame::new()
                        .fill(Color32::from_rgb(12, 17, 26))
                        .corner_radius(8)
                        .inner_margin(Margin::symmetric(10, 7))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (dots_rect, _) = ui.allocate_exact_size(Vec2::new(40.0, 14.0), egui::Sense::hover());
                                let painter = ui.painter_at(dots_rect);
                                let y = dots_rect.center().y;
                                painter.circle_filled(egui::pos2(dots_rect.min.x + 6.0, y), 4.5, Color32::from_rgb(255, 95, 86));
                                painter.circle_filled(egui::pos2(dots_rect.min.x + 19.0, y), 4.5, Color32::from_rgb(255, 189, 46));
                                painter.circle_filled(egui::pos2(dots_rect.min.x + 32.0, y), 4.5, Color32::from_rgb(39, 201, 63));

                                ui.add_space(2.0);
                                ui.label(RichText::new("terminal.log").monospace().size(12.5).color(Color32::from_gray(190)));
                                let avail = (ui.available_width() - 8.0).max(10.0);
                                ui.allocate_ui(Vec2::new(avail, 20.0), |ui| {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        badge_cyan(ui, "Execution Stream");
                                    });
                                });
                            });
                        });

                    egui::Frame::new()
                        .fill(Color32::from_rgb(6, 9, 14))
                        .inner_margin(Margin::same(8))
                        .show(ui, |ui| {
                            ScrollArea::vertical()
                                .id_salt("wizard_step5_terminal_scroll")
                                .max_height(140.0)
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
                                        ui.label(RichText::new(line).monospace().size(11.5).color(color));
                                    }
                                });
                        });
                });

            // Step 5 Navigation Footer
            let (back, _) = render_wizard_navigation_footer(ui, Some("< Back: Boxart Scraping"), None);
            if back {
                self.wizard_focused_step = 4;
            }
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

        let bg_rect = ui.ctx().content_rect();
        ui.painter().rect_filled(bg_rect, 0.0, Color32::from_black_alpha(175));

        egui::Window::new(format!("⭐ Curate Favorites: {} ({}) - {} ROMs", p_name, p_state.platform.id.to_uppercase(), rom_count))
            .id(egui::Id::new("favorites_curation_modal_window"))
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .fixed_pos(egui::pos2(130.0, 40.0))
            .fixed_size(egui::vec2(940.0, 640.0))
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(11, 16, 24))
                    .stroke(Stroke::new(1.5, Color32::from_rgb(0, 229, 255)))
                    .corner_radius(10)
                    .inner_margin(Margin::same(14)),
            )
            .show(ui.ctx(), |ui| {

                // 1. Top Header Title & Close Button
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "⭐ Curate Favorites: {} ({}) - {} ROMs",
                            p_name,
                            p_state.platform.id.to_uppercase(),
                            rom_count
                        ))
                        .size(16.0)
                        .strong()
                        .color(Color32::WHITE),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let x_btn = egui::Button::new(RichText::new("✕").size(15.0).strong().color(Color32::from_gray(200)))
                            .fill(Color32::from_rgb(28, 36, 48))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(45, 58, 78)))
                            .corner_radius(6);
                        if ui.add(x_btn).clicked() {
                            close_modal = true;
                        }
                    });
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // 2. Action Bar
                ui.horizontal(|ui| {
                    let top_btn = egui::Button::new(
                        RichText::new("⭐ Select Recommended Top Classics")
                            .size(12.0)
                            .strong()
                            .color(Color32::from_rgb(255, 230, 100)),
                    )
                    .fill(Color32::from_rgb(36, 28, 12))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(245, 158, 11)))
                    .corner_radius(6);

                    if ui.add(top_btn).clicked() {
                        let top = FavoritesList::from_default_platform(&p_state.platform);
                        for item in top.games {
                            self.editing_selected_games.insert(item);
                        }
                    }

                    let all_btn = egui::Button::new(RichText::new("Select All").size(11.5).strong())
                        .fill(Color32::from_rgb(18, 25, 36))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(32, 44, 62)))
                        .corner_radius(4);
                    if ui.add(all_btn).clicked() {
                        for r in &p_state.rom_files {
                            self.editing_selected_games.insert(r.clone());
                        }
                    }

                    let clear_btn = egui::Button::new(RichText::new("Clear").size(11.5).strong())
                        .fill(Color32::from_rgb(18, 25, 36))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(32, 44, 62)))
                        .corner_radius(4);
                    if ui.add(clear_btn).clicked() {
                        self.editing_selected_games.clear();
                    }

                    ui.add_space(8.0);
                    ui.label(RichText::new("🔍").size(12.5));
                    ui.add(egui::TextEdit::singleline(&mut self.editing_search_query).desired_width(130.0));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        badge_purple(ui, &format!("{} of {} Curated", self.editing_selected_games.len(), rom_count));
                    });
                });

                ui.add_space(6.0);

                // 3. Two Columns: Left (~440px) Games List, Right (~440px) Live Preview & Pattern Rules
                ui.columns(2, |cols| {
                    // Left Column: Games Table
                    let left_ui = &mut cols[0];
                    ScrollArea::vertical()
                        .id_salt("modal_games_scroll")
                        .max_height(410.0)
                        .show(left_ui, |ui| {
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
                                        .fill(Color32::from_rgb(14, 28, 42))
                                        .stroke(Stroke::new(1.0, Color32::from_rgb(0, 229, 255)))
                                        .corner_radius(6)
                                        .inner_margin(Margin::symmetric(6, 5))
                                } else {
                                    egui::Frame::new()
                                        .fill(Color32::from_rgb(10, 14, 20))
                                        .stroke(Stroke::new(1.0, Color32::from_rgb(22, 30, 42)))
                                        .corner_radius(6)
                                        .inner_margin(Margin::symmetric(6, 5))
                                };

                                let inner = ui.push_id(format!("m_rom_{}", rom), |ui| {
                                    frame.show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            let mut chk = is_checked;
                                            if ui.checkbox(&mut chk, "").changed() {
                                                if chk {
                                                    self.editing_selected_games.insert(rom.clone());
                                                } else {
                                                    self.editing_selected_games.remove(rom);
                                                }
                                            }

                                            ui.label(RichText::new(icon).size(15.0));

                                            let title_w = if is_top { 200.0 } else { 290.0 };
                                            ui.add_sized(
                                                [title_w, 20.0],
                                                egui::Label::new(
                                                    RichText::new(rom)
                                                        .size(12.5)
                                                        .color(if is_checked { Color32::WHITE } else { Color32::from_gray(180) }),
                                                )
                                                .truncate(),
                                            );

                                            if is_top {
                                                badge_green(ui, "★ Top Classic");
                                            }
                                        });
                                    })
                                }).inner;

                                let id = inner.response.id.with(format!("modal_game_{}", rom));
                                if ui.interact(inner.response.rect, id, egui::Sense::click()).clicked() {
                                    self.favorites_preview_game = Some(rom.clone());
                                }
                                ui.add_space(2.0);
                            }

                            if shown == 0 {
                                ui.label(RichText::new("No games found.").size(12.0).color(Color32::GRAY));
                            }
                        });

                    // Right Column: Live Libretro Preview Card & Pattern Rules
                    let right_ui = &mut cols[1];
                    right_ui.vertical(|ui| {
                        ui.label(RichText::new("Live preview").size(13.5).strong().color(Color32::WHITE));
                        ui.add_space(3.0);

                        // Artwork Preview Card
                        egui::Frame::new()
                            .fill(Color32::from_rgb(14, 20, 30))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(28, 40, 58)))
                            .corner_radius(8)
                            .inner_margin(Margin::same(12))
                            .show(ui, |ui| {
                                let preview_title = self
                                    .favorites_preview_game
                                    .clone()
                                    .unwrap_or_else(|| "Super Mario Bros.".to_string());

                                ui.horizontal(|ui| {
                                    egui::Frame::new()
                                        .fill(Color32::from_rgb(18, 30, 44))
                                        .stroke(Stroke::new(1.0, Color32::from_rgb(0, 229, 255)))
                                        .corner_radius(6)
                                        .inner_margin(Margin::same(10))
                                        .show(ui, |ui| {
                                            ui.vertical_centered(|ui| {
                                                ui.label(
                                                    RichText::new("LIBRETRO")
                                                        .size(10.0)
                                                        .strong()
                                                        .color(Color32::from_rgb(0, 229, 255)),
                                                );
                                                ui.label(RichText::new("🖼️").size(32.0));
                                                ui.label(RichText::new("COVER").size(9.0).color(Color32::from_gray(170)));
                                            });
                                        });

                                    let avail_meta = (ui.available_width() - 8.0).max(180.0);
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(avail_meta, 82.0),
                                        egui::Layout::top_down(egui::Align::Min),
                                        |ui| {
                                            ui.set_width(avail_meta);
                                            ui.add_sized(
                                                [avail_meta - 10.0, 22.0],
                                                egui::Label::new(
                                                    RichText::new(&preview_title).size(14.0).strong().color(Color32::WHITE),
                                                )
                                                .truncate(),
                                            );
                                            ui.add_space(2.0);
                                            ui.label(
                                                RichText::new(format!("Platform: {}", p_state.platform.name))
                                                    .size(12.0)
                                                    .color(Color32::from_gray(170)),
                                            );
                                            ui.label(RichText::new("Publisher: Verified Dump").size(11.5).color(Color32::from_gray(150)));
                                            ui.label(
                                                RichText::new("CDN: Libretro Thumbnails")
                                                    .size(11.5)
                                                    .color(Color32::from_rgb(16, 185, 129)),
                                            );
                                        },
                                    );
                                });
                            });

                        ui.add_space(8.0);

                        // Keyword Pattern Rules Card
                        egui::Frame::new()
                            .fill(Color32::from_rgb(14, 20, 30))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(28, 40, 58)))
                            .corner_radius(8)
                            .inner_margin(Margin::same(10))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("Keyword Pattern Rules:")
                                        .size(12.5)
                                        .strong()
                                        .color(Color32::from_rgb(0, 229, 255)),
                                );
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.add(egui::TextEdit::singleline(&mut self.new_favorite_pattern).desired_width(180.0));
                                    let add_btn = egui::Button::new(RichText::new("+ Add").size(11.5).strong())
                                        .fill(Color32::from_rgb(20, 36, 54))
                                        .stroke(Stroke::new(1.0, Color32::from_rgb(0, 180, 210)))
                                        .corner_radius(4);
                                    if ui.add(add_btn).clicked() && !self.new_favorite_pattern.trim().is_empty() {
                                        let pat = self.new_favorite_pattern.trim().to_string();
                                        if !self.editing_custom_patterns.contains(&pat) {
                                            self.editing_custom_patterns.push(pat);
                                        }
                                        self.new_favorite_pattern.clear();
                                    }
                                });

                                ui.add_space(6.0);
                                ScrollArea::vertical()
                                    .id_salt("modal_patterns_scroll")
                                    .max_height(140.0)
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        ui.set_max_width(ui.available_width());
                                        ui.horizontal_wrapped(|ui| {
                                            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                                            let mut to_remove = None;
                                            for (p_i, pat) in self.editing_custom_patterns.iter().enumerate() {
                                                if pattern_chip(ui, pat) {
                                                    to_remove = Some(p_i);
                                                }
                                            }
                                            if let Some(r) = to_remove {
                                                self.editing_custom_patterns.remove(r);
                                            }
                                        });
                                    });
                            });
                    });
                });

                if let Some(ref note) = self.favorites_save_notification {
                    ui.add_space(4.0);
                    ui.colored_label(Color32::from_rgb(80, 240, 120), note);
                }

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

                // 4. Modal Footer with Real Buttons
                ui.horizontal(|ui| {
                    let target_folder = p_state
                        .found_dir
                        .as_ref()
                        .map(|d| d.display().to_string())
                        .unwrap_or_else(|| "Not found".to_string());
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("Destination: {}\\{}", target_folder, "favorites.json"))
                                .size(11.5)
                                .color(Color32::from_gray(150)),
                        )
                        .truncate(),
                    );

                    let avail = ui.available_width();
                    let needed = 85.0 + 10.0 + 190.0 + 10.0 + 90.0;
                    if avail > needed {
                        ui.add_space(avail - needed - 8.0);
                    }

                    ui.label(RichText::new("Sync Boxart").size(12.0).color(Color32::from_gray(200)));
                    toggle_switch(ui, &mut self.download_art);
                    ui.add_space(8.0);

                    let save_btn = egui::Button::new(
                        RichText::new("💾 Save favorites.json")
                            .size(12.5)
                            .strong()
                            .color(Color32::BLACK),
                    )
                    .fill(Color32::from_rgb(132, 204, 22))
                    .corner_radius(6);

                    if ui.add_sized([190.0, 32.0], save_btn).clicked() {
                        do_save = true;
                    }

                    ui.add_space(8.0);

                    let close_btn = egui::Button::new(RichText::new("Close").size(12.5).strong())
                        .fill(Color32::from_rgb(20, 28, 40))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(36, 50, 72)))
                        .corner_radius(6);

                    if ui.add_sized([80.0, 32.0], close_btn).clicked() {
                        close_modal = true;
                    }
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
                // Scrollable platform chips so all systems fit without pushing page width
                ScrollArea::horizontal()
                    .id_salt("fav_tab_chips_scroll")
                    .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (i, p) in self.platform_states.iter().enumerate() {
                            let is_cur = self.selected_platform_idx == i;
                            let btn = egui::Button::new(
                                RichText::new(p.platform.id.to_uppercase())
                                    .size(11.5)
                                    .strong()
                                    .color(if is_cur { Color32::WHITE } else { Color32::from_gray(160) }),
                            )
                            .fill(if is_cur { Color32::from_rgb(0, 160, 210) } else { Color32::from_rgb(18, 24, 34) })
                            .stroke(Stroke::new(1.0, if is_cur { Color32::from_rgb(0, 229, 255) } else { Color32::from_rgb(28, 38, 54) }))
                            .corner_radius(4);

                            if ui.add(btn).clicked() {
                                self.selected_platform_idx = i;
                            }
                        }
                    });
                });
            });

            // Two Columns: Left (~60%) Games List, Right (~40%) Live JSON Code Preview
            ui.columns(2, |fcols| {
                let left_ui = &mut fcols[0];
                left_ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔍").size(12.5));
                        ui.text_edit_singleline(&mut self.rom_search_query);

                        badge_cyan(ui, "Genre: RPG ×");
                        badge_purple(ui, "Format: Verified ×");
                    });

                    ui.add_space(6.0);

                    // Games list table
                    ScrollArea::vertical()
                        .id_salt("fav_tab_games_scroll")
                        .max_height(420.0)
                        .show(ui, |ui| {
                        if let Some(p_state) = self.platform_states.get_mut(active_idx) {
                            let top_list = p_state.platform.default_favorites;
                            let search_lower = self.rom_search_query.to_lowercase();

                            for rom in &p_state.rom_files {
                                if !search_lower.is_empty() && !rom.to_lowercase().contains(&search_lower) {
                                    continue;
                                }

                                let is_top = top_list.iter().any(|&c| rom.to_lowercase().contains(&c.to_lowercase()));
                                let icon = get_game_icon(rom);

                                let frame = egui::Frame::new()
                                    .fill(Color32::from_rgb(12, 17, 24))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(24, 34, 48)))
                                    .corner_radius(6)
                                    .inner_margin(Margin::symmetric(8, 6));

                                ui.push_id(format!("fav_tab_row_{}", rom), |ui| {
                                    frame.show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            let mut checked = is_top;
                                            ui.checkbox(&mut checked, "");
                                            ui.label(RichText::new(icon).size(16.0));

                                            let avail = (ui.available_width() - 120.0).max(60.0);
                                            ui.allocate_ui(Vec2::new(avail, 34.0), |ui| {
                                                ui.vertical(|ui| {
                                                    ui.add(egui::Label::new(RichText::new(rom).size(13.0).strong().color(Color32::WHITE)).truncate());
                                                    ui.label(RichText::new(format!("System: {}", p_state.platform.name)).size(11.5).color(Color32::from_gray(140)));
                                                });
                                            });

                                            if is_top {
                                                badge_green(ui, "Top Classic");
                                            }
                                            ui.label(RichText::new("[48 MB]").size(11.0).color(Color32::from_gray(150)));
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
                        ui.label(RichText::new("favorites.json Code Preview").size(13.0).strong().color(Color32::WHITE));
                        ui.add_space(4.0);

                        let p_info = &self.platform_states[active_idx].platform;
                        let sample_fav = FavoritesList::from_default_platform(p_info);
                        if let Ok(mut json_code) = serde_json::to_string_pretty(&sample_fav) {
                            ScrollArea::vertical()
                                .id_salt("fav_code_scroll")
                                .max_height(400.0)
                                .show(ui, |ui| {
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

            // Bottom Footer with Real Buttons
            ui.horizontal(|ui| {
                ui.label(RichText::new("18 of 120 selected").size(12.0).strong().color(Color32::from_rgb(0, 229, 255)));

                let avail = (ui.available_width() - 8.0).max(10.0);
                ui.allocate_ui(Vec2::new(avail, 28.0), |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let ref_btn = egui::Button::new(RichText::new("🔄 Refresh List").size(12.0).strong())
                            .fill(Color32::from_rgb(20, 28, 40))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(36, 50, 72)))
                            .corner_radius(6);
                        if ui.add(ref_btn).clicked() {
                            self.scan_source_folder();
                        }

                        let save_btn = egui::Button::new(RichText::new("Save favorites.json").size(12.5).strong().color(Color32::BLACK))
                            .fill(Color32::from_rgb(16, 185, 129))
                            .corner_radius(6);
                        if ui.add(save_btn).clicked() {
                            self.open_favorites_editor(active_idx);
                        }
                    });
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
                ui.label(RichText::new("Source Library Root:").size(12.5).strong().color(Color32::WHITE));
                ui.text_edit_singleline(&mut self.source_path);

                let browse_btn = egui::Button::new(RichText::new("📂 Browse...").size(12.0).strong())
                    .fill(Color32::from_rgb(20, 28, 40))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(36, 50, 72)))
                    .corner_radius(6);
                if ui.add(browse_btn).clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.source_path = folder.to_string_lossy().to_string();
                        self.scan_source_folder();
                    }
                }

                let rescan_btn = egui::Button::new(RichText::new("🔄 Rescan").size(12.0).strong())
                    .fill(Color32::from_rgb(20, 28, 40))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(36, 50, 72)))
                    .corner_radius(6);
                if ui.add(rescan_btn).clicked() {
                    self.scan_source_folder();
                }
            });

            ui.add_space(8.0);

            // Table of Platforms
            ScrollArea::vertical()
                .id_salt("settings_platforms_scroll")
                .max_height(460.0)
                .show(ui, |ui| {
                let mut open_idx = None;
                for idx in 0..self.platform_states.len() {
                    let p_state = &mut self.platform_states[idx];
                    let is_found = p_state.found_dir.is_some();
                    let plat_name = p_state.platform.name;
                    let plat_id_upper = p_state.platform.id.to_uppercase();
                    let rom_count = p_state.rom_files.len();
                    let path_str = p_state.found_dir.as_ref().map(|d| d.display().to_string()).unwrap_or_else(|| "Directory not found in source library".to_string());

                    let frame = egui::Frame::new()
                        .fill(Color32::from_rgb(11, 16, 24))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(24, 34, 48)))
                        .corner_radius(6)
                        .inner_margin(Margin::symmetric(10, 8));

                    ui.push_id(format!("settings_plat_{}", idx), |ui| {
                        frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut p_state.enabled, "");
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(plat_name).size(13.5).strong().color(Color32::WHITE));
                                        badge_cyan(ui, &plat_id_upper);
                                    });

                                    ui.label(RichText::new(path_str).size(11.5).color(if is_found { Color32::from_gray(160) } else { Color32::from_gray(110) }));
                                });

                                let avail = (ui.available_width() - 8.0).max(10.0);
                                ui.allocate_ui(Vec2::new(avail, 28.0), |ui| {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        badge_green(ui, &format!("{} ROMs", rom_count));
                                        if curated_favorites_button(ui) {
                                            open_idx = Some(idx);
                                        }
                                    });
                                });
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
                cols[0].vertical(|ui| {
                    ui.label(RichText::new("Storage Safety Safeguards").size(13.5).strong().color(Color32::WHITE));
                    ui.add_space(4.0);
                    ui.label(RichText::new("• Formatting is strictly restricted to removable USB and SD cards.\n• System drive C: and internal fixed SSDs (NVMe) are hardware locked.\n• Diskpart clean wipe restores 100% capacity from trapped Linux/ext4 partitions.").size(12.0).color(Color32::from_gray(160)));
                });

                cols[1].vertical(|ui| {
                    ui.label(RichText::new("Libretro Artwork CDN Engine").size(13.5).strong().color(Color32::WHITE));
                    ui.add_space(4.0);
                    ui.label(RichText::new("• Base URL: https://raw.githubusercontent.com/libretro-thumbnails/libretro-thumbnails/master/\n• Automatic name sanitization (&, :, /, \\ converted to _).\n• Zero API key requirement with high-speed parallel asset syncing.").size(12.0).color(Color32::from_gray(160)));
                });
            });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);

            // Artwork & Parallel Workers Config
            ui.label(RichText::new("Artwork & High-Speed Parallel Performance").size(13.5).strong().color(Color32::WHITE));
            ui.add_space(6.0);

            egui::Grid::new("settings_art_grid").spacing([20.0, 10.0]).show(ui, |ui| {
                ui.label(RichText::new("Art Destination:").size(12.0).strong().color(Color32::from_gray(200)));
                ui.horizontal(|ui| {
                    let is_sub = self.art_location_mode == ArtLocationMode::RomSourceSubfolder;
                    let is_temp = self.art_location_mode == ArtLocationMode::TempCacheFolder;
                    let is_sd = self.art_location_mode == ArtLocationMode::TargetDriveOnly;

                    if ui.selectable_label(is_sub, "📂 ROM Source Subfolder").clicked() {
                        self.art_location_mode = ArtLocationMode::RomSourceSubfolder;
                    }
                    if ui.selectable_label(is_temp, "⚡ Temp Cache Folder").clicked() {
                        self.art_location_mode = ArtLocationMode::TempCacheFolder;
                    }
                    if ui.selectable_label(is_sd, "📁 Target SD Card Only").clicked() {
                        self.art_location_mode = ArtLocationMode::TargetDriveOnly;
                    }
                });
                ui.end_row();

                if self.art_location_mode == ArtLocationMode::RomSourceSubfolder {
                    ui.label(RichText::new("Source Subfolder:").size(12.0).strong().color(Color32::from_gray(200)));
                    ui.horizontal(|ui| {
                        for name in &["Imgs", "covers", "boxart", "media"] {
                            let sel = self.art_subfolder_name == *name;
                            if ui.selectable_label(sel, *name).clicked() {
                                self.art_subfolder_name = name.to_string();
                            }
                        }
                        ui.add_sized([80.0, 20.0], egui::TextEdit::singleline(&mut self.art_subfolder_name));
                        ui.label(RichText::new("Stores covers permanently alongside your ROMs on your PC").size(11.5).color(Color32::from_rgb(16, 185, 129)));
                    });
                    ui.end_row();
                }

                ui.label(RichText::new("Parallel Copy Threads:").size(12.0).strong().color(Color32::from_gray(200)));
                ui.horizontal(|ui| {
                    for c in [1, 2, 4, 8, 12] {
                        if ui.selectable_label(self.copy_threads == c, format!("{} threads", c)).clicked() {
                            self.copy_threads = c;
                        }
                    }
                });
                ui.end_row();

                ui.label(RichText::new("Parallel Art Threads:").size(12.0).strong().color(Color32::from_gray(200)));
                ui.horizontal(|ui| {
                    for c in [2, 4, 6, 8, 16] {
                        if ui.selectable_label(self.art_threads == c, format!("{} threads", c)).clicked() {
                            self.art_threads = c;
                        }
                    }
                });
                ui.end_row();

                ui.label(RichText::new("Transfer Pipeline Order:").size(12.0).strong().color(Color32::from_gray(200)));
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.copy_roms_first, "Copy all ROMs first (Phase 1), then download boxart later (Phase 2)");
                });
                ui.end_row();
            });

            ui.add_space(16.0);
            ui.separator();
            ui.label(RichText::new(format!("Retro CardMaker v{} • Native Windows x64 Build • Pair programming Antigravity", env!("CARGO_PKG_VERSION"))).size(11.5).color(Color32::from_gray(140)));
        });
    }
}
