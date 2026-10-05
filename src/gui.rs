use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use walkdir::WalkDir;

use eframe::egui::{self, Color32, Margin, RichText, ScrollArea, Stroke, Vec2};

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

    // Step 4: Artwork & Parallel Workers
    pub download_art: bool,
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

            download_art: true,
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

        let config = InstallConfig {
            drive_letter: clean_drive.to_string(),
            destination_path: dest_path,
            format_option: format_opt,
            wipe_and_repartition: self.wipe_and_repartition && self.do_format,
            volume_label: self.volume_label.clone(),
            profile_id: self.selected_profile_id,
            platforms: platform_configs,
            download_art: self.download_art,
            art_location_mode: self.art_location_mode,
            art_subfolder_name: self.art_subfolder_name.clone(),
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

        let config = InstallConfig {
            drive_letter: clean_drive,
            destination_path: dest_path,
            format_option: None,
            wipe_and_repartition: false,
            volume_label: self.volume_label.clone(),
            profile_id: self.selected_profile_id,
            platforms: platform_configs,
            download_art: true,
            art_location_mode: self.art_location_mode,
            art_subfolder_name: self.art_subfolder_name.clone(),
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
            let has_admin = is_elevated();

            render_card_header(ui, "1. SD Card & Format", |ui| {
                if has_admin {
                    badge_green(ui, "🛡️ ADMIN");
                }
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
            ui.label(RichText::new("Target Storage Drive:").size(12.5).color(Color32::from_gray(180)));
            ui.horizontal(|ui| {
                let drive_display = if let Some(ref d) = selected_drive {
                    let label_str = if d.label.is_empty() { "NO NAME" } else { &d.label };
                    let type_str = if d.is_removable { "Removable" } else if d.is_system { "System" } else { "Fixed" };
                    format!("{}: {} {} ({})", d.letter.trim_end_matches('\\'), d.total_gb_str(), type_str, label_str)
                } else {
                    "No drive detected".to_string()
                };

                egui::ComboBox::from_id_salt("dashboard_drive_combo")
                    .width((ui.available_width() - 95.0).max(140.0))
                    .selected_text(RichText::new(&drive_display).size(13.0).strong().color(Color32::WHITE))
                    .show_ui(ui, |ui| {
                        for (idx, drive) in self.drives.iter().enumerate() {
                            let label_str = if drive.label.is_empty() { "NO NAME" } else { &drive.label };
                            let type_str = if drive.is_removable { "Removable" } else if drive.is_system { "System" } else { "Fixed" };
                            let text = format!("{}: {} {} ({}) - {}", drive.letter.trim_end_matches('\\'), drive.total_gb_str(), type_str, label_str, drive.file_system);
                            ui.selectable_value(&mut self.selected_drive_idx, idx, text);
                        }
                    });

                let ref_btn = egui::Button::new(RichText::new("🔄 Refresh").size(12.0).strong())
                    .fill(Color32::from_rgb(20, 28, 40))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(36, 50, 72)))
                    .corner_radius(6);
                if ui.add(ref_btn).clicked() {
                    self.refresh_drives();
                }
            });

            ui.add_space(8.0);

            // Row 2: Current Filesystem & Safety Lock on left, Format toggles on right
            ui.columns(2, |fcols| {
                // Left Subcolumn
                let left_ui = &mut fcols[0];
                left_ui.vertical(|ui| {
                    ui.label(RichText::new("Current file system").size(12.0).color(Color32::from_gray(160)));
                    let fs_text = selected_drive.as_ref().map(|d| d.file_system.as_str()).unwrap_or("Unknown");
                    ui.label(RichText::new(fs_text).size(15.0).strong().color(Color32::WHITE));
                    ui.add_space(4.0);

                    // Safety Lock Badge
                    egui::Frame::new()
                        .fill(Color32::from_rgb(0, 32, 42))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(0, 229, 255)))
                        .corner_radius(12)
                        .inner_margin(Margin::symmetric(10, 4))
                        .show(ui, |ui| {
                            ui.label(RichText::new("🛡️ SAFETY LOCK").size(11.5).strong().color(Color32::from_rgb(0, 229, 255)));
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
                            ui.label(RichText::new("Format as FAT32").size(13.0).color(Color32::from_gray(220)));
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
                            ui.label(RichText::new("Format as exFAT").size(13.0).color(Color32::from_gray(220)));
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
                        .inner_margin(Margin::same(8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("⚠️").size(14.0));
                                ui.label(RichText::new(format!("Hidden partitions detected: Volume is {} but card is {}.", vol_str, phys_str)).size(12.0).color(Color32::from_rgb(255, 200, 100)));
                            });
                        });
                    ui.add_space(4.0);
                }

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Full Repartition & Clean Wipe (Diskpart MBR)").size(12.5).color(Color32::WHITE));
                    toggle_switch(ui, &mut self.wipe_and_repartition);
                    if has_hidden {
                        badge_amber(ui, "Recommended");
                    }
                });

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Volume Label:").size(12.5).color(Color32::from_gray(180)));
                    ui.add(egui::TextEdit::singleline(&mut self.volume_label).desired_width(120.0));
                    ui.checkbox(&mut self.format_confirmed, RichText::new("Confirm Erase").size(12.0).color(Color32::from_rgb(255, 180, 100)));
                });

                if !has_admin {
                    ui.add_space(6.0);
                    egui::Frame::new()
                        .fill(Color32::from_rgb(42, 28, 10))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(245, 158, 11)))
                        .corner_radius(6)
                        .inner_margin(Margin::same(8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("⚠️").size(14.0));
                                ui.label(RichText::new("Formatting requires Windows Administrator privileges.").size(12.0).color(Color32::from_rgb(255, 200, 100)));
                                let el_btn = egui::Button::new(RichText::new("🛡️ Relaunch as Admin").size(11.5).strong().color(Color32::BLACK))
                                    .fill(Color32::from_rgb(245, 158, 11))
                                    .corner_radius(4);
                                if ui.add(el_btn).clicked() {
                                    let _ = relaunch_as_admin();
                                }
                            });
                        });
                }
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

            // Horizontal Profile Cards Grid with Modern Interactive Cards
            ScrollArea::horizontal()
                .id_salt("card2_profiles_scroll")
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
                            ui.set_width(114.0);
                            ui.set_height(68.0);
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

                        let id = ui.id().with(format!("prof_{}", profile.name));
                        if ui.interact(resp.rect, id, egui::Sense::click()).clicked() {
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
                    let all_btn = egui::Button::new(RichText::new("All").size(11.5).strong())
                        .fill(Color32::from_rgb(18, 25, 36))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(32, 44, 62)))
                        .corner_radius(4);
                    if ui.add(all_btn).clicked() {
                        for p in &mut self.platform_states {
                            p.enabled = true;
                        }
                    }

                    let none_btn = egui::Button::new(RichText::new("None").size(11.5).strong())
                        .fill(Color32::from_rgb(18, 25, 36))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(32, 44, 62)))
                        .corner_radius(4);
                    if ui.add(none_btn).clicked() {
                        for p in &mut self.platform_states {
                            p.enabled = false;
                        }
                    }
                });
            });

            // Scrollable List of detected consoles
            ScrollArea::vertical()
                .id_salt("card3_platforms_scroll")
                .max_height(270.0)
                .show(ui, |ui| {
                let mut open_idx = None;

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

                    let plat_name = p_state.platform.name;

                    let inner = ui.push_id(format!("plat_row_{}", idx), |ui| {
                        frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut p_state.enabled, "");
                                ui.label(RichText::new(p_icon).size(18.0));

                                // Bounded left width to prevent pushing screen width
                                let avail = (ui.available_width() - 150.0).max(110.0);
                                ui.allocate_ui(Vec2::new(avail, 34.0), |ui| {
                                    ui.vertical(|ui| {
                                        ui.add(egui::Label::new(RichText::new(plat_name).size(13.5).strong().color(if is_found { Color32::WHITE } else { Color32::from_gray(130) })).truncate());
                                        ui.label(
                                            RichText::new(format!("{} ROMs", rom_count))
                                                .size(11.5)
                                                .color(if rom_count > 0 { Color32::from_rgb(0, 229, 255) } else { Color32::from_gray(120) }),
                                        );
                                    });
                                });

                                // Real Curated Favorites Button
                                if curated_favorites_button(ui) {
                                    open_idx = Some(idx);
                                }
                            });
                        })
                    }).inner;

                    let row_id = inner.response.id.with(format!("console_row_{}", idx));
                    if ui.interact(inner.response.rect, row_id, egui::Sense::click()).clicked() {
                        self.selected_platform_idx = idx;
                    }

                    ui.add_space(4.0);
                }

                if let Some(idx) = open_idx {
                    self.open_favorites_editor(idx);
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
                ui.label(RichText::new("🔍").size(13.0));
                ui.text_edit_singleline(&mut self.rom_search_query);

                let avail = (ui.available_width() - 8.0).max(10.0);
                ui.allocate_ui(Vec2::new(avail, 24.0), |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        badge_green(ui, "Libretro CDN");
                        toggle_switch(ui, &mut self.download_art);
                        ui.label(RichText::new("Sync Boxart").size(12.0).color(Color32::from_gray(200)));
                    });
                });
            });

            // If Boxart sync is enabled, show artwork target location and parallel configuration
            if self.download_art {
                ui.add_space(4.0);
                egui::Frame::new()
                    .fill(Color32::from_rgb(8, 12, 18))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(20, 28, 40)))
                    .corner_radius(6)
                    .inner_margin(Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        // Row 1: Target Location Selector
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Save Art To:").size(11.5).strong().color(Color32::from_gray(190)));

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

                        // Row 2: Subfolder options or path details
                        ui.horizontal(|ui| {
                            match self.art_location_mode {
                                ArtLocationMode::RomSourceSubfolder => {
                                    ui.label(RichText::new("Subfolder:").size(11.0).color(Color32::from_gray(150)));
                                    for name in &["Imgs", "covers", "boxart", "media"] {
                                        let sel = self.art_subfolder_name == *name;
                                        if ui.selectable_label(sel, *name).clicked() {
                                            self.art_subfolder_name = name.to_string();
                                        }
                                    }
                                    ui.add_sized([65.0, 18.0], egui::TextEdit::singleline(&mut self.art_subfolder_name));
                                    ui.label(RichText::new("(saved alongside ROMs & synced to SD)").size(11.0).color(Color32::from_rgb(16, 185, 129)));
                                }
                                ArtLocationMode::TempCacheFolder => {
                                    ui.label(RichText::new("Cache: %TEMP%\\retro-cardmaker\\art_cache (synced to SD)").size(11.0).color(Color32::from_rgb(0, 229, 255)));
                                }
                                ArtLocationMode::TargetDriveOnly => {
                                    ui.label(RichText::new("Saves directly to SD card launcher folders only").size(11.0).color(Color32::from_gray(150)));
                                }
                            }
                        });

                        // Row 3: Parallel Workers & Action button
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Workers:").size(11.0).color(Color32::from_gray(150)));
                            ui.label(RichText::new("Copy:").size(10.5).color(Color32::GRAY));
                            for c in [2, 4, 8] {
                                if ui.selectable_label(self.copy_threads == c, c.to_string()).clicked() {
                                    self.copy_threads = c;
                                }
                            }
                            ui.label(RichText::new("Art:").size(10.5).color(Color32::GRAY));
                            for c in [2, 4, 6, 8] {
                                if ui.selectable_label(self.art_threads == c, c.to_string()).clicked() {
                                    self.art_threads = c;
                                }
                            }

                            let avail = (ui.available_width() - 4.0).max(10.0);
                            ui.allocate_ui(Vec2::new(avail, 20.0), |ui| {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let btn_sync = egui::Button::new(
                                        RichText::new("📥 Sync Boxart Now")
                                            .size(11.0)
                                            .strong()
                                            .color(Color32::from_rgb(0, 229, 255)),
                                    )
                                    .fill(Color32::from_rgb(0, 32, 46))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(0, 180, 210)))
                                    .corner_radius(4);

                                    let can_sync = self.platform_states.iter().any(|p| p.enabled && p.found_dir.is_some());
                                    if ui.add(btn_sync).on_hover_text("Download/sync boxart for all enabled platforms now without copying ROMs").clicked() && !self.is_running && can_sync {
                                        self.start_sync_art_only();
                                    }
                                });
                            });
                        });
                    });
            }

            ui.add_space(4.0);

            // Games Table
            ScrollArea::vertical()
                .id_salt("card4_roms_scroll")
                .max_height(145.0)
                .show(ui, |ui| {
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
                                            ui.label(RichText::new(get_game_icon(rom)).size(16.0));
                                        });

                                    // Bounded text width using truncate to prevent layout blowout
                                    let avail = (ui.available_width() - 110.0).max(60.0);
                                    ui.allocate_ui(Vec2::new(avail, 34.0), |ui| {
                                        ui.vertical(|ui| {
                                            ui.add(egui::Label::new(RichText::new(rom).size(13.0).strong().color(Color32::WHITE)).truncate());
                                            ui.label(RichText::new(format!("System: {}", p_state.platform.name)).size(11.5).color(Color32::from_gray(140)));
                                        });
                                    });

                                    // Badges on right
                                    if is_top {
                                        badge_green(ui, "Top Classic");
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
                        ui.label(RichText::new("No ROMs match the filter or folder empty.").size(12.0).color(Color32::GRAY));
                    }
                } else {
                    ui.label(RichText::new("No platform selected.").size(12.0).color(Color32::GRAY));
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

            // Progress Bar 2: FILE COPY or BOXART SYNC
            let (bar_label, copy_frac, copy_text) = if self.current_phase.contains("Art") || self.current_phase.contains("Boxart") {
                let f = if self.progress_total > 0 {
                    self.progress_current as f32 / self.progress_total as f32
                } else {
                    0.0
                };
                let t = if self.progress_total > 0 {
                    format!("{}/{} ({:.0}%)", self.progress_current, self.progress_total, f * 100.0)
                } else {
                    "0/0 (0%)".to_string()
                };
                (format!("BOXART SYNC ({} workers)", self.art_threads), f, t)
            } else {
                let f = if self.progress_total > 0 {
                    self.progress_current as f32 / self.progress_total as f32
                } else {
                    0.0
                };
                let t = if self.progress_total > 0 {
                    format!("{}/{} ({:.0}%)", self.progress_current, self.progress_total, f * 100.0)
                } else {
                    "0/0 (0%)".to_string()
                };
                (format!("FILE COPY ({} workers)", self.copy_threads), f, t)
            };
            render_neon_progress_bar(ui, &bar_label, copy_frac, &copy_text);

            let item_label = if self.current_item.is_empty() {
                if self.is_running { "Transferring game assets..." } else { "System idling. Ready for quickinstall." }
            } else {
                &self.current_item
            };
            ui.label(RichText::new(item_label).size(12.0).color(Color32::from_gray(160)));

            ui.add_space(6.0);

            // Action Buttons
            if !self.is_running {
                let has_admin = is_elevated();
                let can_start = !self.drives.is_empty()
                    && (!self.do_format || (self.format_confirmed && has_admin))
                    && self.platform_states.iter().any(|p| p.enabled && p.found_dir.is_some());

                let target_letter = self.drives.get(self.selected_drive_idx).map(|d| d.letter.as_str()).unwrap_or("E:");

                let start_btn = egui::Button::new(
                    RichText::new(format!("🚀 START QUICKINSTALL ({})", target_letter))
                        .size(14.5)
                        .strong()
                        .color(Color32::BLACK),
                )
                .fill(Color32::from_rgb(16, 185, 129))
                .corner_radius(8);

                if ui.add_sized([ui.available_width(), 38.0], start_btn).clicked() && can_start {
                    self.start_install();
                }

                ui.add_space(4.0);

                let can_sync_art = self.platform_states.iter().any(|p| p.enabled && p.found_dir.is_some());
                let sync_art_btn = egui::Button::new(
                    RichText::new("📥 SYNC / DOWNLOAD BOXART ONLY")
                        .size(12.0)
                        .strong()
                        .color(Color32::from_rgb(0, 229, 255)),
                )
                .fill(Color32::from_rgb(0, 28, 42))
                .stroke(Stroke::new(1.0, Color32::from_rgb(0, 180, 210)))
                .corner_radius(6);

                if ui.add_sized([ui.available_width(), 28.0], sync_art_btn).clicked() && can_sync_art {
                    self.start_sync_art_only();
                }

                if !can_start {
                    ui.add_space(3.0);
                    let warning_str = if self.do_format && !has_admin {
                        "Administrator privileges required to format. Click 'Relaunch as Admin' in Card 1."
                    } else {
                        "Confirm erase or verify removable drive to launch."
                    };
                    ui.label(RichText::new(warning_str).size(11.5).color(Color32::from_rgb(255, 180, 100)));
                }
            } else {
                let cancel_btn = egui::Button::new(
                    RichText::new("🛑 CANCEL QUICKINSTALL")
                        .size(14.5)
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
                // Header with crisp vector painted macOS control dots
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

                // Scrollable Console Body
                egui::Frame::new()
                    .fill(Color32::from_rgb(6, 9, 14))
                    .inner_margin(Margin::same(8))
                    .show(ui, |ui| {
                        ScrollArea::vertical()
                            .id_salt("card6_terminal_scroll")
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
                                    ui.label(RichText::new(line).monospace().size(11.5).color(color));
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
