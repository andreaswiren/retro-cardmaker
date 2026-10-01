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
use crate::platforms::{find_platform_by_id, PlatformInfo, PLATFORMS};

#[derive(Clone)]
pub struct PlatformUiState {
    pub platform: PlatformInfo,
    pub enabled: bool,
    pub found_dir: Option<PathBuf>,
    pub rom_files: Vec<String>,
    pub mode: CopyMode,
    pub favorites: Option<FavoritesList>,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum ActiveTab {
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
        .fill(Color32::from_rgb(15, 20, 28))
        .stroke(Stroke::new(1.0, Color32::from_rgb(28, 36, 50)))
        .corner_radius(8)
        .inner_margin(Margin::same(12))
}

fn card_frame_selected() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::from_rgb(16, 26, 38))
        .stroke(Stroke::new(1.2, Color32::from_rgb(0, 229, 255)))
        .corner_radius(8)
        .inner_margin(Margin::same(12))
}

fn card_frame_protected() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::from_rgb(28, 14, 18))
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
        Color32::from_rgb(0, 35, 45),
        Color32::from_rgb(0, 180, 210),
        Color32::from_rgb(0, 229, 255),
    );
}

fn badge_green(ui: &mut egui::Ui, text: &str) {
    render_badge(
        ui,
        text,
        Color32::from_rgb(10, 32, 22),
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

fn render_step_card(ui: &mut egui::Ui, step_num: &str, title: &str, desc: &str, is_active: bool) -> bool {
    let frame = if is_active {
        egui::Frame::new()
            .fill(Color32::from_rgb(18, 28, 42))
            .stroke(Stroke::new(1.5, Color32::from_rgb(0, 229, 255)))
            .corner_radius(8)
            .inner_margin(Margin::symmetric(10, 8))
    } else {
        egui::Frame::new()
            .fill(Color32::from_rgb(15, 20, 28))
            .stroke(Stroke::new(1.0, Color32::from_rgb(28, 36, 50)))
            .corner_radius(8)
            .inner_margin(Margin::symmetric(10, 8))
    };

    let inner = frame.show(ui, |ui| {
        ui.vertical(|ui| {
            let num_color = if is_active { Color32::from_rgb(0, 229, 255) } else { Color32::from_gray(120) };
            ui.label(RichText::new(step_num).size(9.5).color(num_color).strong());
            ui.label(RichText::new(title).size(11.5).strong().color(if is_active { Color32::WHITE } else { Color32::from_gray(210) }));
            ui.label(RichText::new(desc).size(10.0).color(Color32::from_gray(140)));
        });
    });

    let id = inner.response.id.with(format!("step_card_{}", step_num));
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
    // Tab Navigation
    pub active_tab: ActiveTab,

    // Step 1: SD Card & Format
    pub drives: Vec<DriveInfo>,
    pub selected_drive_idx: usize,
    pub subfolder_name: String,
    pub do_format: bool,
    pub format_fs: FormatFileSystem,
    pub volume_label: String,
    pub format_confirmed: bool,

    // Step 2: Launcher & Device Profile
    pub selected_profile_id: ProfileId,

    // Step 3: Source & Platforms
    pub source_path: String,
    pub platform_states: Vec<PlatformUiState>,

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

    // Modal: Favorites Editor
    pub editing_platform_idx: Option<usize>,
    pub editing_selected_games: HashSet<String>,
    pub editing_custom_patterns: Vec<String>,
    pub editing_search_query: String,
    pub editing_show_selected_only: bool,
    pub new_favorite_pattern: String,
    pub favorites_save_notification: Option<String>,
}

impl RetroCardMakerApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let drives = get_available_drives();
        let default_profile = ProfileId::AnbernicRgDsLauncher;
        let profile = LauncherProfile::get_by_id(default_profile);

        let mut app = Self {
            active_tab: ActiveTab::SdCard,
            drives,
            selected_drive_idx: 0,
            subfolder_name: profile.recommended_sd_subfolder.to_string(),
            do_format: false,
            format_fs: FormatFileSystem::ExFat,
            volume_label: "RETRO".to_string(),
            format_confirmed: false,

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

            download_art: true,

            is_running: false,
            current_phase: "Ready".to_string(),
            progress_current: 0,
            progress_total: 0,
            current_item: String::new(),
            logs: vec![
                "[INIT] Retro CardMaker core engine ready.".to_string(),
                "[DRIVE] Hardware scanner initialized.".to_string(),
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
        };

        // Apply Refined Compact Desktop Dark Theme
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

        app
    }

    pub fn refresh_drives(&mut self) {
        self.drives = get_available_drives();
        if self.selected_drive_idx >= self.drives.len() && !self.drives.is_empty() {
            self.selected_drive_idx = 0;
        }
    }

    pub fn scan_source_folder(&mut self) {
        let root = Path::new(&self.source_path);
        if !root.exists() || !root.is_dir() {
            return;
        }

        for p_state in &mut self.platform_states {
            p_state.found_dir = None;
            p_state.rom_files.clear();
            p_state.favorites = None;

            for alias in p_state.platform.folder_aliases {
                let candidate = root.join(alias);
                if candidate.is_dir() {
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
                    break;
                }
            }
        }
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
        self.active_tab = ActiveTab::Install;

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

        // 1. Top Header Bar (Matching Mockup)
        ui.vertical(|ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                // Gradient-styled Brand Icon
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
                    "v0.2.0",
                    Color32::from_rgb(18, 24, 34),
                    Color32::from_rgb(45, 58, 78),
                    Color32::from_gray(180),
                );

                badge_cyan(ui, "SD QUICKINSTALLER");

                ui.separator();
                ui.label(
                    RichText::new("SD-Card Formatter • Curated ROMs • Libretro Artwork Scraper")
                        .size(11.0)
                        .color(Color32::from_gray(160)),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    render_badge(
                        ui,
                        "Windows x64 Native",
                        Color32::from_rgb(15, 20, 28),
                        Color32::from_rgb(28, 36, 50),
                        Color32::from_gray(160),
                    );
                });
            });
            ui.add_space(4.0);
            ui.separator();
        });

        // 2. Context Bar (Matching Mockup)
        ui.add_space(4.0);
        egui::Frame::new()
            .fill(Color32::from_rgb(15, 20, 28))
            .stroke(Stroke::new(1.0, Color32::from_rgb(28, 36, 50)))
            .corner_radius(8)
            .inner_margin(Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("Active Target Configuration")
                                .size(12.0)
                                .strong()
                                .color(Color32::WHITE),
                        );
                        ui.label(
                            RichText::new("Target Handheld SD Pipeline")
                                .size(10.0)
                                .color(Color32::from_gray(140)),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Artwork CDN pill
                        let art_str = if self.download_art {
                            "Art: Libretro Raw CDN"
                        } else {
                            "Art: Disabled"
                        };
                        badge_green(ui, art_str);

                        // ROM count pill
                        let total_roms: usize = self
                            .platform_states
                            .iter()
                            .filter(|p| p.enabled)
                            .map(|p| match p.mode {
                                CopyMode::AllRoms => p.rom_files.len(),
                                CopyMode::FavoritesOnly => {
                                    p.favorites.as_ref().map(|f| f.games.len()).unwrap_or(0)
                                }
                            })
                            .sum();
                        let active_platforms = self
                            .platform_states
                            .iter()
                            .filter(|p| p.enabled && p.found_dir.is_some())
                            .count();
                        badge_purple(ui, &format!("ROMs: {} / {} Systems", total_roms, active_platforms));

                        // Profile pill
                        let profile = LauncherProfile::get_by_id(self.selected_profile_id);
                        render_badge(
                            ui,
                            &format!("Profile: {}", profile.name),
                            Color32::from_rgb(20, 25, 38),
                            Color32::from_rgb(45, 58, 78),
                            Color32::from_gray(200),
                        );

                        // Drive pill
                        if let Some(drive) = self.drives.get(self.selected_drive_idx) {
                            let rem_str = if drive.is_removable {
                                "Removable"
                            } else {
                                "Fixed"
                            };
                            let size_gb = drive.total_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                            badge_cyan(
                                ui,
                                &format!(
                                    "Drive: {} ({:.0} GB - {})",
                                    drive.letter, size_gb, rem_str
                                ),
                            );
                        }
                    });
                });
            });

        // 3. Wizard Step Bar (Matching Mockup 5-Column Cards)
        ui.add_space(4.0);
        ui.columns(5, |cols| {
            let steps = [
                (
                    ActiveTab::SdCard,
                    "Step 1",
                    "💾 SD Card & Format",
                    "Target drive & safety guard",
                ),
                (
                    ActiveTab::Profile,
                    "Step 2",
                    "🕹️ Device Profile",
                    "Anbernic, ES-DE, RetroArch",
                ),
                (
                    ActiveTab::Platforms,
                    "Step 3",
                    "📂 ROMs & Curate",
                    "Full list vs Favorites.json",
                ),
                (
                    ActiveTab::Artwork,
                    "Step 4",
                    "🖼️ Boxart Pipeline",
                    "Local Imgs + Libretro CDN",
                ),
                (
                    ActiveTab::Install,
                    "Step 5",
                    "🚀 QuickInstall",
                    "Execute & progress logs",
                ),
            ];

            for (i, (tab, step_num, title, desc)) in steps.iter().enumerate() {
                let is_active = self.active_tab == *tab;
                if render_step_card(&mut cols[i], step_num, title, desc, is_active) {
                    self.active_tab = *tab;
                }
            }
        });

        ui.add_space(4.0);
        ui.separator();

        // 4. Main Tab Content in ScrollArea
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.add_space(6.0);
                match self.active_tab {
                    ActiveTab::SdCard => self.render_sd_card_tab(ui),
                    ActiveTab::Profile => self.render_profile_tab(ui),
                    ActiveTab::Platforms => self.render_platforms_tab(ui),
                    ActiveTab::Artwork => self.render_artwork_tab(ui),
                    ActiveTab::Install => self.render_install_tab(ui),
                }
            });

        // 5. Supercharged Favorites Editor Modal
        self.render_favorites_modal(ui);
    }
}

// ----------------------------------------------------------------------------
// Step Tab Renderers
// ----------------------------------------------------------------------------

impl RetroCardMakerApp {
    fn render_sd_card_tab(&mut self, ui: &mut egui::Ui) {
        ui.columns(2, |cols| {
            // Left Column: Target SD Card Selection & Format Settings
            let ui = &mut cols[0];

            card_frame().show(ui, |ui| {
                render_card_header(ui, "Target SD Card Selection", |ui| {
                    badge_cyan(ui, "Physical Storage");
                });

                if self.drives.is_empty() {
                    ui.colored_label(Color32::RED, "No storage drives found!");
                } else {
                    for (idx, drive) in self.drives.iter().enumerate() {
                        let is_sel = self.selected_drive_idx == idx;
                        let is_sys = drive.is_system;

                        let frame = if is_sel {
                            card_frame_selected()
                        } else if is_sys {
                            card_frame_protected()
                        } else {
                            egui::Frame::new()
                                .fill(Color32::from_rgb(10, 14, 20))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(24, 32, 44)))
                                .corner_radius(6)
                                .inner_margin(Margin::symmetric(10, 8))
                        };

                        let inner = frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let icon = if is_sys { "🛡️" } else { "💾" };
                                ui.label(RichText::new(icon).size(16.0));
                                ui.vertical(|ui| {
                                    let label_str = if drive.label.is_empty() {
                                        "SD_CARD"
                                    } else {
                                        &drive.label
                                    };
                                    ui.label(
                                        RichText::new(format!("{} [{}]", drive.letter, label_str))
                                            .size(11.5)
                                            .strong()
                                            .color(Color32::WHITE),
                                    );
                                    let type_str = if drive.is_removable {
                                        "Removable SD/USB"
                                    } else {
                                        "Fixed Disk"
                                    };
                                    ui.label(
                                        RichText::new(format!(
                                            "{} • {} • Free: {} / {}",
                                            type_str,
                                            drive.file_system,
                                            drive.free_gb_str(),
                                            drive.total_gb_str()
                                        ))
                                        .size(10.0)
                                        .color(Color32::from_gray(150)),
                                    );
                                });

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if is_sys {
                                            badge_red(ui, "PROTECTED");
                                        } else if drive.is_removable {
                                            badge_green(ui, "Recommended");
                                        }
                                    },
                                );
                            });
                        });

                        let id = inner.response.id.with(format!("drive_opt_{}", idx));
                        if ui
                            .interact(inner.response.rect, id, egui::Sense::click())
                            .clicked()
                        {
                            self.selected_drive_idx = idx;
                        }
                        ui.add_space(4.0);
                    }
                }

                ui.add_space(4.0);
                if ui.button(RichText::new("🔄 Refresh Drives").size(11.0)).clicked() {
                    self.refresh_drives();
                }

                ui.add_space(8.0);

                // Format Settings Subcard
                egui::Frame::new()
                    .fill(Color32::from_rgb(10, 14, 20))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(24, 32, 44)))
                    .corner_radius(6)
                    .inner_margin(Margin::same(10))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("Format SD-Card before copying")
                                        .size(11.5)
                                        .strong()
                                        .color(Color32::WHITE),
                                );
                                ui.label(
                                    RichText::new(
                                        "Formats storage cleanly with optimal cluster allocation.",
                                    )
                                    .size(10.0)
                                    .color(Color32::from_gray(140)),
                                );
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.checkbox(&mut self.do_format, "");
                                },
                            );
                        });

                        if self.do_format {
                            ui.add_space(6.0);
                            ui.columns(2, |fcols| {
                                // exFAT Box
                                let exfat_sel = self.format_fs == FormatFileSystem::ExFat;
                                let frame1 = if exfat_sel {
                                    card_frame_selected()
                                } else {
                                    card_frame()
                                };
                                let inner1 = frame1.show(&mut fcols[0], |ui| {
                                    ui.vertical(|ui| {
                                        ui.label(
                                            RichText::new("exFAT (Recommended)")
                                                .size(11.0)
                                                .strong()
                                                .color(if exfat_sel {
                                                    Color32::from_rgb(0, 229, 255)
                                                } else {
                                                    Color32::WHITE
                                                }),
                                        );
                                        ui.label(
                                            RichText::new(
                                                "Modern handhelds (RG Cube, Odin, Deck) & 64GB+.",
                                            )
                                            .size(9.5)
                                            .color(Color32::from_gray(140)),
                                        );
                                    });
                                });
                                let id1 = inner1.response.id.with("fs_exfat_btn");
                                if fcols[0]
                                    .interact(inner1.response.rect, id1, egui::Sense::click())
                                    .clicked()
                                {
                                    self.format_fs = FormatFileSystem::ExFat;
                                }

                                // FAT32 Box
                                let fat32_sel = self.format_fs == FormatFileSystem::Fat32;
                                let frame2 = if fat32_sel {
                                    card_frame_selected()
                                } else {
                                    card_frame()
                                };
                                let inner2 = frame2.show(&mut fcols[1], |ui| {
                                    ui.vertical(|ui| {
                                        ui.label(
                                            RichText::new("FAT32")
                                                .size(11.0)
                                                .strong()
                                                .color(if fat32_sel {
                                                    Color32::from_rgb(0, 229, 255)
                                                } else {
                                                    Color32::WHITE
                                                }),
                                        );
                                        ui.label(
                                            RichText::new(
                                                "Legacy handhelds (RG350, Miyoo, GarlicOS) & ≤32GB.",
                                            )
                                            .size(9.5)
                                            .color(Color32::from_gray(140)),
                                        );
                                    });
                                });
                                let id2 = inner2.response.id.with("fs_fat32_btn");
                                if fcols[1]
                                    .interact(inner2.response.rect, id2, egui::Sense::click())
                                    .clicked()
                                {
                                    self.format_fs = FormatFileSystem::Fat32;
                                }
                            });

                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Volume Label:")
                                        .size(11.0)
                                        .color(Color32::from_gray(180)),
                                );
                                ui.text_edit_singleline(&mut self.volume_label);
                            });

                            ui.add_space(4.0);
                            ui.checkbox(
                                &mut self.format_confirmed,
                                RichText::new(
                                    "I understand formatting will erase all data on this target drive.",
                                )
                                .size(10.5)
                                .color(Color32::from_rgb(255, 180, 100)),
                            );
                        }

                        if let Some(drive) = self.drives.get(self.selected_drive_idx) {
                            if drive.is_system {
                                ui.add_space(6.0);
                                egui::Frame::new()
                                    .fill(Color32::from_rgb(40, 15, 18))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(239, 68, 68)))
                                    .corner_radius(6)
                                    .inner_margin(Margin::same(8))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(
                                                "🛡️ SYSTEM DRIVE PROTECTED: Drive C: is strictly locked against formatting.",
                                            )
                                            .size(10.5)
                                            .color(Color32::from_rgb(255, 140, 140))
                                            .strong(),
                                        );
                                    });
                            }
                        }
                    });
            });

            // Right Column: Destination Subfolder Card
            let ui = &mut cols[1];
            card_frame().show(ui, |ui| {
                render_card_header(ui, "Destination Subfolder", |ui| {
                    badge_purple(ui, "SD Layout");
                });

                ui.label(
                    RichText::new("Target Subfolder:")
                        .size(11.5)
                        .strong()
                        .color(Color32::WHITE),
                );
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.subfolder_name);
                    if ui.button(RichText::new("roms").size(11.0)).clicked() {
                        self.subfolder_name = "roms".to_string();
                    }
                    if ui.button(RichText::new("Roms").size(11.0)).clicked() {
                        self.subfolder_name = "Roms".to_string();
                    }
                    if ui.button(RichText::new("Root").size(11.0)).clicked() {
                        self.subfolder_name = String::new();
                    }
                });

                ui.add_space(8.0);
                let drive_prefix = self
                    .drives
                    .get(self.selected_drive_idx)
                    .map(|d| d.letter.as_str())
                    .unwrap_or("E:\\");
                let clean_prefix = drive_prefix.trim_end_matches('\\').trim_end_matches('/');
                let preview_path = if self.subfolder_name.trim().is_empty() {
                    format!("{}\\", clean_prefix)
                } else {
                    format!("{}\\{}\\", clean_prefix, self.subfolder_name.trim())
                };

                egui::Frame::new()
                    .fill(Color32::from_rgb(8, 12, 18))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(24, 32, 44)))
                    .corner_radius(6)
                    .inner_margin(Margin::same(8))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Full Destination Base:")
                                .size(10.5)
                                .color(Color32::from_gray(140)),
                        );
                        ui.label(
                            RichText::new(preview_path)
                                .size(12.0)
                                .monospace()
                                .strong()
                                .color(Color32::from_rgb(0, 229, 255)),
                        );
                    });

                ui.add_space(8.0);
                ui.label(
                    RichText::new("💡 Profile Matches:")
                        .size(11.0)
                        .strong()
                        .color(Color32::from_gray(190)),
                );
                ui.label(
                    RichText::new("• Anbernic RG DS Launcher: E:\\roms\\NDS, GBA, SFC, FC")
                        .size(10.0)
                        .color(Color32::from_gray(150)),
                );
                ui.label(
                    RichText::new("• Miyoo Mini (OnionOS): E:\\Roms\\GBA, FC, SFC")
                        .size(10.0)
                        .color(Color32::from_gray(150)),
                );
                ui.label(
                    RichText::new("• EmulationStation (ES-DE): E:\\roms\\nds, gba, snes")
                        .size(10.0)
                        .color(Color32::from_gray(150)),
                );

                ui.add_space(16.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let next_btn = egui::Button::new(
                        RichText::new("Proceed to Device Profiles →")
                            .size(11.5)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(0, 110, 200));

                    if ui.add(next_btn).clicked() {
                        self.active_tab = ActiveTab::Profile;
                    }
                });
            });
        });
    }

    fn render_profile_tab(&mut self, ui: &mut egui::Ui) {
        card_frame().show(ui, |ui| {
            let active_profile = LauncherProfile::get_by_id(self.selected_profile_id);
            render_card_header(ui, "Select Emulation Frontend / Device Launcher", |ui| {
                badge_purple(ui, &format!("{} Selected", active_profile.name));
            });
            ui.label(
                RichText::new(
                    "Adjusts directory naming codes and boxart destination rules automatically.",
                )
                .size(10.5)
                .color(Color32::from_gray(150)),
            );
            ui.add_space(8.0);

            ui.columns(2, |cols| {
                for (p_idx, profile) in PROFILES.iter().enumerate() {
                    let col = &mut cols[p_idx % 2];
                    let is_sel = self.selected_profile_id == profile.id;

                    let frame = if is_sel {
                        card_frame_selected()
                    } else {
                        egui::Frame::new()
                            .fill(Color32::from_rgb(10, 14, 20))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(24, 32, 44)))
                            .corner_radius(6)
                            .inner_margin(Margin::same(10))
                    };

                    let icon = match profile.id {
                        ProfileId::AnbernicRgDsLauncher => "🕹️",
                        ProfileId::EmulationStationEsDe => "🎮",
                        ProfileId::RetroArch => "👾",
                        ProfileId::AnbernicOlderGarlicOs => "🧄",
                        ProfileId::MiyooMiniOnionOs => "🧅",
                        ProfileId::Daijisho => "📱",
                        ProfileId::Pegasus => "🎠",
                        ProfileId::BatoceraArkOs => "🦇",
                        ProfileId::Custom => "⚙️",
                    };

                    let tag_str = match profile.id {
                        ProfileId::AnbernicRgDsLauncher => "Cube & Dual-Screen",
                        ProfileId::EmulationStationEsDe => "Universal",
                        ProfileId::RetroArch => "Libretro",
                        ProfileId::AnbernicOlderGarlicOs => "Legacy Handheld",
                        ProfileId::MiyooMiniOnionOs => "Miyoo",
                        ProfileId::Daijisho => "Android",
                        ProfileId::Pegasus => "Metadata",
                        ProfileId::BatoceraArkOs => "Retro",
                        ProfileId::Custom => "Custom",
                    };

                    let inner = frame.show(col, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(icon).size(16.0));
                            ui.label(
                                RichText::new(profile.name)
                                    .size(11.5)
                                    .strong()
                                    .color(Color32::WHITE),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if is_sel {
                                        badge_cyan(ui, "Active");
                                    } else {
                                        badge_purple(ui, tag_str);
                                    }
                                },
                            );
                        });
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(profile.description)
                                .size(10.0)
                                .color(Color32::from_gray(140)),
                        );
                        ui.add_space(4.0);

                        egui::Frame::new()
                            .fill(Color32::from_rgb(6, 9, 13))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(20, 28, 38)))
                            .corner_radius(4)
                            .inner_margin(Margin::symmetric(6, 4))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(format!("Pattern: {}", profile.guidance))
                                        .size(9.5)
                                        .monospace()
                                        .color(Color32::from_rgb(0, 229, 255)),
                                );
                            });
                    });

                    let id = inner.response.id.with(format!("prof_opt_{}", p_idx));
                    if col
                        .interact(inner.response.rect, id, egui::Sense::click())
                        .clicked()
                    {
                        self.selected_profile_id = profile.id;
                        self.subfolder_name = profile.recommended_sd_subfolder.to_string();
                    }
                    col.add_space(6.0);
                }
            });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new("← Back to SD Setup").size(11.0))
                    .clicked()
                {
                    self.active_tab = ActiveTab::SdCard;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let next_btn = egui::Button::new(
                        RichText::new("Proceed to ROMs & Favorites →")
                            .size(11.5)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(0, 110, 200));

                    if ui.add(next_btn).clicked() {
                        self.active_tab = ActiveTab::Platforms;
                    }
                });
            });
        });
    }

    fn render_platforms_tab(&mut self, ui: &mut egui::Ui) {
        card_frame().show(ui, |ui| {
            render_card_header(ui, "Discovered Platforms & Curated Games", |ui| {
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Select All").size(10.5)).clicked() {
                        for p in &mut self.platform_states {
                            p.enabled = true;
                        }
                    }
                    if ui.button(RichText::new("Deselect All").size(10.5)).clicked() {
                        for p in &mut self.platform_states {
                            p.enabled = false;
                        }
                    }
                });
            });

            // Source Library Directory Bar
            egui::Frame::new()
                .fill(Color32::from_rgb(10, 14, 20))
                .stroke(Stroke::new(1.0, Color32::from_rgb(24, 32, 44)))
                .corner_radius(6)
                .inner_margin(Margin::same(8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Source Library:")
                                .size(11.5)
                                .strong()
                                .color(Color32::WHITE),
                        );
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
                });

            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "Toggle between full ROM collection or curated shortlists (favorites.json):",
                )
                .size(10.5)
                .color(Color32::from_gray(140)),
            );
            ui.add_space(6.0);

            // Platform Rows
            for idx in 0..self.platform_states.len() {
                let p_state = &mut self.platform_states[idx];
                let is_found = p_state.found_dir.is_some();
                let rom_count = p_state.rom_files.len();
                let fav_count = p_state.favorites.as_ref().map(|f| f.games.len()).unwrap_or(0);

                let frame = if p_state.enabled && is_found {
                    egui::Frame::new()
                        .fill(Color32::from_rgb(12, 17, 24))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(32, 44, 62)))
                        .corner_radius(6)
                        .inner_margin(Margin::symmetric(10, 8))
                } else {
                    egui::Frame::new()
                        .fill(Color32::from_rgb(8, 11, 16))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(20, 26, 36)))
                        .corner_radius(6)
                        .inner_margin(Margin::symmetric(10, 8))
                };

                let p_icon = match p_state.platform.id {
                    "nds" => "📱",
                    "gba" | "gb" | "gbc" => "📟",
                    "snes" | "nes" => "🎮",
                    "n64" => "🕹️",
                    "psx" | "ps2" => "💿",
                    "psp" => "🎮",
                    "megadrive" | "genesis" => "⚡",
                    "dreamcast" => "🌀",
                    "saturn" => "🪐",
                    _ => "🕹️",
                };

                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut p_state.enabled, "");
                        ui.label(RichText::new(p_icon).size(14.0));
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(p_state.platform.name)
                                    .size(11.5)
                                    .strong()
                                    .color(if is_found {
                                        Color32::WHITE
                                    } else {
                                        Color32::from_gray(120)
                                    }),
                            );
                            if is_found {
                                let fav_str = if p_state.favorites.is_some() {
                                    "• favorites.json detected"
                                } else {
                                    ""
                                };
                                ui.label(
                                    RichText::new(format!("{} ROMs found {}", rom_count, fav_str))
                                        .size(10.0)
                                        .color(Color32::from_gray(140)),
                                );
                            } else {
                                ui.label(
                                    RichText::new("Folder not found in source library")
                                        .size(9.5)
                                        .color(Color32::from_gray(100)),
                                );
                            }
                        });

                        if is_found {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // Curate Button
                                    let curate_btn = egui::Button::new(
                                        RichText::new("⭐ Curate Favorites...")
                                            .size(11.0)
                                            .strong()
                                            .color(Color32::BLACK),
                                    )
                                    .fill(Color32::from_rgb(0, 229, 255));

                                    if ui.add(curate_btn).clicked() {
                                        self.editing_platform_idx = Some(idx);
                                        self.editing_selected_games.clear();
                                        self.editing_custom_patterns.clear();
                                        self.editing_search_query.clear();
                                        self.editing_show_selected_only = false;
                                        self.favorites_save_notification = None;

                                        if let Some(ref favs) = p_state.favorites {
                                            for g in &favs.games {
                                                self.editing_selected_games.insert(g.clone());
                                            }
                                            for pat in &favs.patterns {
                                                self.editing_custom_patterns.push(pat.clone());
                                            }
                                        } else {
                                            let default_favs = FavoritesList::from_default_platform(
                                                &p_state.platform,
                                            );
                                            for g in &default_favs.games {
                                                self.editing_selected_games.insert(g.clone());
                                            }
                                            for pat in &default_favs.patterns {
                                                self.editing_custom_patterns.push(pat.clone());
                                            }
                                        }
                                    }

                                    // Mode Segmented Control: [All (N)] vs [Curated (M)]
                                    ui.horizontal(|ui| {
                                        let all_active = p_state.mode == CopyMode::AllRoms;
                                        let all_btn = egui::Button::new(
                                            RichText::new(format!("All ({})", rom_count))
                                                .size(10.5)
                                                .color(if all_active {
                                                    Color32::WHITE
                                                } else {
                                                    Color32::from_gray(160)
                                                }),
                                        )
                                        .fill(if all_active {
                                            Color32::from_rgb(139, 92, 246)
                                        } else {
                                            Color32::from_rgb(15, 20, 28)
                                        });
                                        if ui.add(all_btn).clicked() {
                                            p_state.mode = CopyMode::AllRoms;
                                        }

                                        let cur_active = p_state.mode == CopyMode::FavoritesOnly;
                                        let cur_btn = egui::Button::new(
                                            RichText::new(format!("Curated ({})", fav_count))
                                                .size(10.5)
                                                .color(if cur_active {
                                                    Color32::WHITE
                                                } else {
                                                    Color32::from_gray(160)
                                                }),
                                        )
                                        .fill(if cur_active {
                                            Color32::from_rgb(139, 92, 246)
                                        } else {
                                            Color32::from_rgb(15, 20, 28)
                                        });
                                        if ui.add(cur_btn).clicked() {
                                            p_state.mode = CopyMode::FavoritesOnly;
                                        }
                                    });
                                },
                            );
                        }
                    });
                });
                ui.add_space(4.0);
            }

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new("← Back to Profile").size(11.0))
                    .clicked()
                {
                    self.active_tab = ActiveTab::Profile;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let next_btn = egui::Button::new(
                        RichText::new("Proceed to Boxart Pipeline →")
                            .size(11.5)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(0, 110, 200));

                    if ui.add(next_btn).clicked() {
                        self.active_tab = ActiveTab::Artwork;
                    }
                });
            });
        });
    }

    fn render_artwork_tab(&mut self, ui: &mut egui::Ui) {
        ui.columns(2, |cols| {
            // Left Column: Artwork Scraping & Local Media Card
            let ui = &mut cols[0];
            card_frame().show(ui, |ui| {
                render_card_header(ui, "Artwork Scraping & Local Media", |ui| {
                    badge_green(ui, "Libretro CDN");
                });

                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("Enable Boxart Syncing during Copy")
                                .size(11.5)
                                .strong()
                                .color(Color32::WHITE),
                        );
                        ui.label(
                            RichText::new(
                                "Reuses local covers in Imgs/ and downloads missing ones from Libretro.",
                            )
                            .size(10.0)
                            .color(Color32::from_gray(140)),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.checkbox(&mut self.download_art, "");
                    });
                });

                ui.add_space(8.0);
                egui::Frame::new()
                    .fill(Color32::from_rgb(10, 14, 20))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(24, 32, 44)))
                    .corner_radius(6)
                    .inner_margin(Margin::same(10))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Boxart Strategy:")
                                .size(11.0)
                                .strong()
                                .color(Color32::from_rgb(0, 229, 255)),
                        );
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(
                                "1. Syncs existing local media (e.g. nds\\Imgs\\) directly.",
                            )
                            .size(10.0)
                            .color(Color32::from_gray(160)),
                        );
                        ui.label(
                            RichText::new(
                                "2. Normalizes filenames (replaces '& * / : \\' with '_').",
                            )
                            .size(10.0)
                            .color(Color32::from_gray(160)),
                        );
                        let profile = LauncherProfile::get_by_id(self.selected_profile_id);
                        ui.label(
                            RichText::new(format!(
                                "3. Places into target folder expected by {}.",
                                profile.name
                            ))
                            .size(10.0)
                            .color(Color32::from_gray(160)),
                        );
                        ui.label(
                            RichText::new(
                                "4. Downloads missing covers from Libretro Thumbnails CDN (Free, no keys).",
                            )
                            .size(10.0)
                            .color(Color32::from_gray(160)),
                        );
                    });

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("← Back to ROMs").size(11.0)).clicked() {
                        self.active_tab = ActiveTab::Platforms;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let next_btn = egui::Button::new(
                            RichText::new("Review & Run QuickInstaller →")
                                .size(11.5)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(0, 110, 200));

                        if ui.add(next_btn).clicked() {
                            self.active_tab = ActiveTab::Install;
                        }
                    });
                });
            });

            // Right Column: Live Scraper Preview Card
            let ui = &mut cols[1];
            card_frame().show(ui, |ui| {
                render_card_header(ui, "Live Scraper Preview", |ui| {
                    badge_cyan(ui, "Matched Cover");
                });

                let profile = LauncherProfile::get_by_id(self.selected_profile_id);
                let dummy_dest = Path::new("E:\\roms");
                let dummy_platform = find_platform_by_id("nds").unwrap();
                let art_example =
                    profile.get_art_destination(dummy_dest, dummy_platform, "Chrono Trigger");

                egui::Frame::new()
                    .fill(Color32::from_rgb(10, 14, 20))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(24, 32, 44)))
                    .corner_radius(6)
                    .inner_margin(Margin::same(12))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            egui::Frame::new()
                                .fill(Color32::from_rgb(18, 25, 36))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(0, 229, 255)))
                                .corner_radius(6)
                                .inner_margin(Margin::same(14))
                                .show(ui, |ui| {
                                    ui.label(RichText::new("🖼️").size(24.0));
                                });

                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("Chrono Trigger")
                                        .size(12.5)
                                        .strong()
                                        .color(Color32::WHITE),
                                );
                                ui.label(
                                    RichText::new("Source: nds\\Imgs\\Chrono Trigger (Europe).png")
                                        .size(10.0)
                                        .color(Color32::from_gray(140)),
                                );
                                ui.add_space(4.0);
                                egui::Frame::new()
                                    .fill(Color32::from_rgb(6, 9, 13))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(20, 28, 38)))
                                    .corner_radius(4)
                                    .inner_margin(Margin::symmetric(6, 4))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(format!(
                                                "Destination: {}",
                                                art_example.display()
                                            ))
                                            .size(9.5)
                                            .monospace()
                                            .color(Color32::from_rgb(0, 229, 255)),
                                        );
                                    });
                            });
                        });
                    });
            });
        });
    }

    fn render_install_tab(&mut self, ui: &mut egui::Ui) {
        ui.columns(2, |cols| {
            // Left Column: QuickInstall Execution Card
            let ui = &mut cols[0];
            card_frame().show(ui, |ui| {
                render_card_header(ui, "QuickInstall Execution", |ui| {
                    if self.is_running {
                        badge_amber(ui, "Running");
                    } else if self.install_summary.is_some() {
                        badge_green(ui, "Complete");
                    } else {
                        badge_green(ui, "Ready");
                    }
                });

                // Summary Info Box
                egui::Frame::new()
                    .fill(Color32::from_rgb(10, 14, 20))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(24, 32, 44)))
                    .corner_radius(6)
                    .inner_margin(Margin::same(10))
                    .show(ui, |ui| {
                        if let Some(drive) = self.drives.get(self.selected_drive_idx) {
                            let fmt_info = if self.do_format {
                                format!("as {} ('{}')", self.format_fs.as_str(), self.volume_label)
                            } else {
                                "Keep Existing Data".to_string()
                            };
                            ui.label(
                                RichText::new(format!(
                                    "• Target: {} ({}) {}",
                                    drive.letter,
                                    drive.total_gb_str(),
                                    fmt_info
                                ))
                                .size(10.5)
                                .color(Color32::from_gray(180)),
                            );
                        }

                        let profile = LauncherProfile::get_by_id(self.selected_profile_id);
                        ui.label(
                            RichText::new(format!(
                                "• Profile: {} ({})",
                                profile.name, self.subfolder_name
                            ))
                            .size(10.5)
                            .color(Color32::from_rgb(168, 130, 255)),
                        );

                        let active_platforms: Vec<_> = self
                            .platform_states
                            .iter()
                            .filter(|p| p.enabled && p.found_dir.is_some())
                            .collect();
                        let plat_names: Vec<String> = active_platforms
                            .iter()
                            .take(4)
                            .map(|p| {
                                format!(
                                    "{} ({})",
                                    p.platform.name,
                                    match p.mode {
                                        CopyMode::AllRoms => "All",
                                        CopyMode::FavoritesOnly => "Curated",
                                    }
                                )
                            })
                            .collect();
                        ui.label(
                            RichText::new(format!("• Platforms: {}", plat_names.join(", ")))
                                .size(10.0)
                                .color(Color32::from_gray(160)),
                        );

                        let total_roms: usize = active_platforms
                            .iter()
                            .map(|p| match p.mode {
                                CopyMode::AllRoms => p.rom_files.len(),
                                CopyMode::FavoritesOnly => {
                                    p.favorites.as_ref().map(|f| f.games.len()).unwrap_or(0)
                                }
                            })
                            .sum();
                        ui.label(
                            RichText::new(format!(
                                "• Total: {} ROMs + Paired Saves + Boxarts",
                                total_roms
                            ))
                            .size(10.5)
                            .strong()
                            .color(Color32::WHITE),
                        );
                    });

                ui.add_space(8.0);

                // Progress Bar & Phase
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("Status: {}", self.current_phase))
                            .size(11.0)
                            .strong()
                            .color(Color32::WHITE),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let pct = if self.progress_total > 0 {
                            (self.progress_current as f32 / self.progress_total as f32) * 100.0
                        } else {
                            0.0
                        };
                        ui.label(
                            RichText::new(format!("{:.0}%", pct))
                                .size(11.5)
                                .strong()
                                .color(Color32::from_rgb(0, 229, 255)),
                        );
                    });
                });

                let fraction = if self.progress_total > 0 {
                    self.progress_current as f32 / self.progress_total as f32
                } else {
                    0.0
                };

                ui.add(egui::ProgressBar::new(fraction).animate(self.is_running));

                let item_label = if self.current_item.is_empty() {
                    if self.is_running {
                        "Processing files..."
                    } else {
                        "Awaiting launch..."
                    }
                } else {
                    &self.current_item
                };
                ui.label(
                    RichText::new(item_label)
                        .size(10.0)
                        .color(Color32::from_gray(140)),
                );

                ui.add_space(10.0);

                // Controls
                ui.horizontal(|ui| {
                    if !self.is_running {
                        let can_start = !self.drives.is_empty()
                            && (!self.do_format || self.format_confirmed)
                            && self
                                .platform_states
                                .iter()
                                .any(|p| p.enabled && p.found_dir.is_some());

                        let start_btn = egui::Button::new(
                            RichText::new("🚀 Start QuickInstaller")
                                .size(12.5)
                                .strong()
                                .color(Color32::BLACK),
                        )
                        .fill(Color32::from_rgb(16, 185, 129));

                        if ui.add_enabled(can_start, start_btn).clicked() {
                            self.start_install();
                        }

                        if !can_start {
                            ui.colored_label(
                                Color32::from_rgb(255, 170, 70),
                                "Confirm format or select drive to proceed.",
                            );
                        }
                    } else {
                        let cancel_btn = egui::Button::new(
                            RichText::new("🛑 Cancel Installation")
                                .size(12.0)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(220, 50, 50));

                        if ui.add(cancel_btn).clicked() {
                            self.cancel_flag.store(true, Ordering::Relaxed);
                            self.logs.push("Cancelling operation...".to_string());
                        }
                    }
                });

                if let Some(ref summary) = self.install_summary {
                    ui.add_space(6.0);
                    ui.colored_label(
                        Color32::from_rgb(80, 240, 80),
                        format!(
                            "🎉 Complete! Copied {} ROMs, synced {} boxarts ({:.1} MB).",
                            summary.total_roms_copied,
                            summary.total_art_downloaded,
                            summary.total_bytes_copied as f64 / (1024.0 * 1024.0)
                        ),
                    );
                }

                if let Some(ref err) = self.install_error {
                    ui.add_space(6.0);
                    ui.colored_label(Color32::from_rgb(255, 90, 90), format!("❌ Error: {}", err));
                }
            });

            // Right Column: Styled Terminal Console Window Card
            let ui = &mut cols[1];
            egui::Frame::new()
                .fill(Color32::from_rgb(5, 8, 12))
                .stroke(Stroke::new(1.0, Color32::from_rgb(28, 36, 50)))
                .corner_radius(8)
                .inner_margin(Margin::same(0))
                .show(ui, |ui| {
                    // Terminal Header Bar with Red, Yellow, Green dots
                    egui::Frame::new()
                        .fill(Color32::from_rgb(11, 15, 23))
                        .stroke(Stroke::new(0.0, Color32::TRANSPARENT))
                        .corner_radius(8)
                        .inner_margin(Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("🔴").size(8.0));
                                ui.label(RichText::new("🟡").size(8.0));
                                ui.label(RichText::new("🟢").size(8.0));
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new("Console Output")
                                        .size(10.5)
                                        .color(Color32::from_gray(160)),
                                );
                            });
                        });

                    // Terminal Body with scroll
                    egui::Frame::new()
                        .fill(Color32::from_rgb(5, 8, 12))
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            ScrollArea::vertical()
                                .max_height(240.0)
                                .stick_to_bottom(true)
                                .show(ui, |ui| {
                                    for line in &self.logs {
                                        let color = if line.contains("ERROR")
                                            || line.contains("Failed")
                                        {
                                            Color32::from_rgb(255, 100, 100)
                                        } else if line.contains("Phase")
                                            || line.contains("Succeeded")
                                        {
                                            Color32::from_rgb(0, 229, 255)
                                        } else if line.contains("Downloaded")
                                            || line.contains("Copied")
                                        {
                                            Color32::from_rgb(16, 185, 129)
                                        } else {
                                            Color32::from_rgb(180, 200, 225)
                                        };
                                        ui.label(
                                            RichText::new(line)
                                                .monospace()
                                                .size(10.0)
                                                .color(color),
                                        );
                                    }
                                });
                        });
                });
        });
    }

    // ------------------------------------------------------------------------
    // Curated Favorites Modal (Matching Mockup 2-Column Layout)
    // ------------------------------------------------------------------------

    fn render_favorites_modal(&mut self, ui: &mut egui::Ui) {
        let Some(p_idx) = self.editing_platform_idx else {
            return;
        };

        let p_state = &self.platform_states[p_idx];
        let p_name = p_state.platform.name;
        let mut close_modal = false;
        let mut do_save = false;

        egui::Window::new(format!("⭐ Curate Favorites: {}", p_name))
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(880.0, 560.0))
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Easily select what is saved into favorites.json. Search, toggle items, or add keyword rules.")
                            .size(11.0)
                            .color(Color32::from_gray(160)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        badge_purple(
                            ui,
                            &format!(
                                "{} of {} Curated",
                                self.editing_selected_games.len(),
                                p_state.rom_files.len()
                            ),
                        );
                    });
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // Two-Column Grid inside the Modal (Matching Mockup)
                ui.columns(2, |mcols| {
                    // Left Column (~55%): Search, Quick Bulk Actions & Games Table
                    let ui = &mut mcols[0];

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔍").size(12.0));
                        ui.text_edit_singleline(&mut self.editing_search_query);

                        if ui.button(RichText::new("⭐ Top Essentials").size(10.5)).clicked() {
                            let top = FavoritesList::from_default_platform(&p_state.platform);
                            for item in top.games {
                                self.editing_selected_games.insert(item);
                            }
                        }
                        if ui.button(RichText::new("All").size(10.5)).clicked() {
                            for rom in &p_state.rom_files {
                                if self.editing_search_query.is_empty()
                                    || rom
                                        .to_lowercase()
                                        .contains(&self.editing_search_query.to_lowercase())
                                {
                                    self.editing_selected_games.insert(rom.clone());
                                }
                            }
                        }
                        if ui.button(RichText::new("Invert").size(10.5)).clicked() {
                            for rom in &p_state.rom_files {
                                if self.editing_selected_games.contains(rom) {
                                    self.editing_selected_games.remove(rom);
                                } else {
                                    self.editing_selected_games.insert(rom.clone());
                                }
                            }
                        }
                        if ui.button(RichText::new("Clear").size(10.5)).clicked() {
                            self.editing_selected_games.clear();
                        }
                    });

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.checkbox(
                            &mut self.editing_show_selected_only,
                            RichText::new(format!(
                                "Show Selected Only ({})",
                                self.editing_selected_games.len()
                            ))
                            .size(10.5),
                        );
                        ui.separator();
                        ui.label(
                            RichText::new(format!("Total ROMs: {}", p_state.rom_files.len()))
                                .size(10.5)
                                .color(Color32::from_gray(150)),
                        );
                    });

                    ui.add_space(4.0);

                    // Games List Table
                    ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                        let search_lower = self.editing_search_query.to_lowercase();
                        let top_list = p_state.platform.default_favorites;

                        let mut displayed_count = 0;
                        for rom in &p_state.rom_files {
                            let is_selected = self.editing_selected_games.contains(rom);

                            if self.editing_show_selected_only && !is_selected {
                                continue;
                            }
                            if !search_lower.is_empty() && !rom.to_lowercase().contains(&search_lower) {
                                continue;
                            }

                            displayed_count += 1;
                            let is_essential = top_list
                                .iter()
                                .any(|&c| rom.to_lowercase().contains(&c.to_lowercase()));
                            let game_icon = get_game_icon(rom);

                            let row_frame = if is_selected {
                                egui::Frame::new()
                                    .fill(Color32::from_rgb(14, 24, 34))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(0, 180, 210)))
                                    .corner_radius(4)
                                    .inner_margin(Margin::symmetric(6, 4))
                            } else {
                                egui::Frame::new()
                                    .fill(Color32::from_rgb(10, 14, 20))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(22, 28, 38)))
                                    .corner_radius(4)
                                    .inner_margin(Margin::symmetric(6, 4))
                            };

                            row_frame.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let mut checked = is_selected;
                                    if ui.checkbox(&mut checked, "").changed() {
                                        if checked {
                                            self.editing_selected_games.insert(rom.clone());
                                        } else {
                                            self.editing_selected_games.remove(rom);
                                        }
                                    }

                                    ui.label(RichText::new(game_icon).size(13.0));

                                    ui.label(
                                        RichText::new(rom)
                                            .size(11.0)
                                            .color(if is_selected {
                                                Color32::WHITE
                                            } else {
                                                Color32::from_gray(160)
                                            }),
                                    );

                                    if is_essential {
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                badge_green(ui, "Essential");
                                            },
                                        );
                                    }
                                });
                            });
                            ui.add_space(2.0);
                        }

                        if displayed_count == 0 {
                            ui.label(
                                RichText::new("No games match the current search filter.")
                                    .size(10.5)
                                    .color(Color32::GRAY),
                            );
                        }
                    });

                    // Right Column (~45%): Search Pattern Rules & Live Generated Code Preview
                    let ui = &mut mcols[1];

                    // 1. Search Pattern Rules Card
                    egui::Frame::new()
                        .fill(Color32::from_rgb(10, 14, 20))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(24, 32, 44)))
                        .corner_radius(6)
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Search Pattern Rules:")
                                        .size(11.0)
                                        .strong()
                                        .color(Color32::from_rgb(0, 229, 255)),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new("Matches substrings")
                                                .size(9.5)
                                                .color(Color32::from_gray(130)),
                                        );
                                    },
                                );
                            });

                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut self.new_favorite_pattern);
                                if ui.button(RichText::new("+ Add").size(10.5)).clicked()
                                    && !self.new_favorite_pattern.trim().is_empty()
                                {
                                    let pat = self.new_favorite_pattern.trim().to_string();
                                    if !self.editing_custom_patterns.contains(&pat) {
                                        self.editing_custom_patterns.push(pat);
                                    }
                                    self.new_favorite_pattern.clear();
                                }
                            });

                            ui.add_space(6.0);
                            ui.horizontal_wrapped(|ui| {
                                let mut to_remove = None;
                                for (p_i, pat) in self.editing_custom_patterns.iter().enumerate() {
                                    egui::Frame::new()
                                        .fill(Color32::from_rgb(0, 35, 45))
                                        .stroke(Stroke::new(1.0, Color32::from_rgb(0, 180, 210)))
                                        .corner_radius(4)
                                        .inner_margin(Margin::symmetric(5, 2))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(pat)
                                                        .size(10.0)
                                                        .monospace()
                                                        .color(Color32::from_rgb(165, 243, 252)),
                                                );
                                                if ui.button(RichText::new("✖").size(8.5)).clicked()
                                                {
                                                    to_remove = Some(p_i);
                                                }
                                            });
                                        });
                                }
                                if let Some(r_idx) = to_remove {
                                    self.editing_custom_patterns.remove(r_idx);
                                }
                            });
                        });

                    ui.add_space(8.0);

                    // 2. Live Generated favorites.json Preview Card
                    egui::Frame::new()
                        .fill(Color32::from_rgb(10, 14, 20))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(24, 32, 44)))
                        .corner_radius(6)
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Live Generated favorites.json:")
                                        .size(11.0)
                                        .strong()
                                        .color(Color32::WHITE),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        badge_green(ui, "✓ Auto-Synchronized");
                                    },
                                );
                            });

                            ui.add_space(4.0);
                            let mut preview_games: Vec<String> =
                                self.editing_selected_games.iter().cloned().collect();
                            preview_games.sort();
                            let preview_fav = FavoritesList {
                                platform: p_state.platform.id.to_string(),
                                title: format!("{} Curated Favorites", p_state.platform.name),
                                games: preview_games,
                                patterns: self.editing_custom_patterns.clone(),
                            };

                            if let Ok(mut json_str) = serde_json::to_string_pretty(&preview_fav) {
                                ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                                    ui.add(
                                        egui::TextEdit::multiline(&mut json_str)
                                            .font(egui::TextStyle::Monospace)
                                            .desired_rows(10)
                                            .desired_width(f32::INFINITY),
                                    );
                                });
                            }
                        });
                });

                if let Some(ref note) = self.favorites_save_notification {
                    ui.add_space(4.0);
                    ui.colored_label(Color32::from_rgb(80, 240, 120), note);
                }

                ui.add_space(8.0);
                ui.separator();

                // Modal Footer
                ui.horizontal(|ui| {
                    let target_folder = p_state
                        .found_dir
                        .as_ref()
                        .map(|d| d.display().to_string())
                        .unwrap_or_else(|| "Not found".to_string());
                    ui.label(
                        RichText::new(format!("Target: {}\\{}", target_folder, "favorites.json"))
                            .size(10.0)
                            .color(Color32::from_gray(150)),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let save_btn = egui::Button::new(
                            RichText::new("💾 Save favorites.json")
                                .size(11.5)
                                .strong()
                                .color(Color32::BLACK),
                        )
                        .fill(Color32::from_rgb(16, 185, 129));

                        if ui.add(save_btn).clicked() {
                            do_save = true;
                        }

                        if ui.button(RichText::new("Cancel").size(11.0)).clicked() {
                            close_modal = true;
                        }
                    });
                });
            });

        if do_save {
            if let Some(ref dir) = self.platform_states[p_idx].found_dir {
                let mut games_list: Vec<String> =
                    self.editing_selected_games.iter().cloned().collect();
                games_list.sort();

                let favs = FavoritesList {
                    platform: self.platform_states[p_idx].platform.id.to_string(),
                    title: format!(
                        "{} Curated Favorites",
                        self.platform_states[p_idx].platform.name
                    ),
                    games: games_list,
                    patterns: self.editing_custom_patterns.clone(),
                };

                match save_favorites(dir, &favs) {
                    Ok(_) => {
                        self.favorites_save_notification = Some(format!(
                            "✓ Successfully saved favorites.json with {} games!",
                            favs.games.len()
                        ));
                        self.platform_states[p_idx].favorites = Some(favs);
                    }
                    Err(e) => {
                        self.favorites_save_notification =
                            Some(format!("Error saving favorites: {}", e));
                    }
                }
            } else {
                self.favorites_save_notification =
                    Some("Error: Source folder not found.".to_string());
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
