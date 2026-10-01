use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use walkdir::WalkDir;

use eframe::egui::{self, Color32, RichText, ScrollArea, Stroke, Vec2};

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
    pub new_favorite_pattern: String,
}

impl RetroCardMakerApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let drives = get_available_drives();
        let default_profile = ProfileId::EmulationStationEsDe;
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
            logs: vec!["Retro CardMaker initialized. Ready to setup SD card.".to_string()],
            install_summary: None,
            install_error: None,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            event_rx: None,

            editing_platform_idx: None,
            editing_selected_games: HashSet::new(),
            new_favorite_pattern: String::new(),
        };

        // If removable drive found, auto-select it
        if let Some(pos) = app.drives.iter().position(|d| d.is_removable) {
            app.selected_drive_idx = pos;
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

            // Search for platform directory by aliases
            for alias in p_state.platform.folder_aliases {
                let candidate = root.join(alias);
                if candidate.is_dir() {
                    p_state.found_dir = Some(candidate.clone());

                    // Check for favorites.json
                    p_state.favorites = load_favorites(&candidate);

                    // Scan files
                    for entry in WalkDir::new(&candidate)
                        .max_depth(2)
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
        self.logs.push("Starting QuickInstaller process...".to_string());
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

impl eframe::App for RetroCardMakerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_events();
        if self.is_running {
            ui.ctx().request_repaint();
        }

        // Top Header
        ui.vertical(|ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading(
                    RichText::new("🎮 RETRO CARDMAKER")
                        .size(24.0)
                        .color(Color32::from_rgb(90, 200, 250))
                        .strong(),
                );
                ui.label(
                    RichText::new("v0.1.0")
                        .size(13.0)
                        .color(Color32::LIGHT_GRAY),
                );
                ui.separator();
                ui.label(
                    RichText::new("SD-Card Formatter, Curated ROM Manager & Boxart Scraper")
                        .color(Color32::GRAY),
                );
            });
            ui.add_space(4.0);
            ui.separator();
        });

        // Tab Navigation Bar
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let tabs = [
                (ActiveTab::SdCard, "1. 💾 SD Card & Format"),
                (ActiveTab::Profile, "2. 🕹️ Device Profile"),
                (ActiveTab::Platforms, "3. 📂 ROMs & Favorites"),
                (ActiveTab::Artwork, "4. 🖼️ Boxart Scraping"),
                (ActiveTab::Install, "5. 🚀 QuickInstall"),
            ];

            for (tab, label) in tabs {
                let is_selected = self.active_tab == tab;
                let btn = if is_selected {
                    egui::Button::new(
                        RichText::new(label)
                            .strong()
                            .color(Color32::from_rgb(255, 255, 255)),
                    )
                    .fill(Color32::from_rgb(45, 110, 210))
                } else {
                    egui::Button::new(RichText::new(label))
                };

                if ui.add(btn).clicked() {
                    self.active_tab = tab;
                }
            }
        });
        ui.add_space(6.0);
        ui.separator();

        // Main Tab Content
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.add_space(10.0);
                match self.active_tab {
                    ActiveTab::SdCard => self.render_sd_card_tab(ui),
                    ActiveTab::Profile => self.render_profile_tab(ui),
                    ActiveTab::Platforms => self.render_platforms_tab(ui),
                    ActiveTab::Artwork => self.render_artwork_tab(ui),
                    ActiveTab::Install => self.render_install_tab(ui),
                }
            });

        // Favorites Editor Modal
        self.render_favorites_modal(ui);
    }
}

impl RetroCardMakerApp {
    fn render_sd_card_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Step 1: Select SD Card & Formatting Options");
        ui.add_space(4.0);
        ui.label("Choose your target SD card drive. The tool detects removable drives and protects system partitions.");

        ui.add_space(10.0);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Target Drive:").strong());
                if self.drives.is_empty() {
                    ui.label(RichText::new("No drives found!").color(Color32::RED));
                } else {
                    egui::ComboBox::from_id_salt("drive_combo")
                        .width(420.0)
                        .selected_text(
                            self.drives
                                .get(self.selected_drive_idx)
                                .map(|d| d.display_summary())
                                .unwrap_or_else(|| "Select Drive".to_string()),
                        )
                        .show_ui(ui, |ui| {
                            for (idx, drive) in self.drives.iter().enumerate() {
                                ui.selectable_value(
                                    &mut self.selected_drive_idx,
                                    idx,
                                    drive.display_summary(),
                                );
                            }
                        });
                }

                if ui.button("🔄 Refresh Drives").clicked() {
                    self.refresh_drives();
                }
            });

            if let Some(drive) = self.drives.get(self.selected_drive_idx) {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(format!("Drive: {}", drive.letter));
                    ui.separator();
                    ui.label(format!("Label: {}", if drive.label.is_empty() { "None" } else { &drive.label }));
                    ui.separator();
                    ui.label(format!("File System: {}", drive.file_system));
                    ui.separator();
                    ui.label(format!("Free: {} / Total: {}", drive.free_gb_str(), drive.total_gb_str()));
                });

                if drive.is_system {
                    ui.add_space(6.0);
                    ui.colored_label(
                        Color32::LIGHT_RED,
                        "⚠️ PROTECTED SYSTEM DRIVE (C:): Formatting is strictly disabled for system drives.",
                    );
                }
            }
        });

        ui.add_space(15.0);
        ui.heading("Formatting & Destination");
        egui::Frame::group(ui.style()).show(ui, |ui| {
            let is_system = self
                .drives
                .get(self.selected_drive_idx)
                .map(|d| d.is_system)
                .unwrap_or(false);

            ui.add_enabled_ui(!is_system, |ui| {
                ui.checkbox(&mut self.do_format, RichText::new("Format SD Card before installing").strong());
                if self.do_format {
                    ui.indent("format_options", |ui| {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label("File System:");
                            ui.radio_value(
                                &mut self.format_fs,
                                FormatFileSystem::ExFat,
                                "exFAT (Recommended for modern handhelds & 64GB+ cards)",
                            );
                            ui.radio_value(
                                &mut self.format_fs,
                                FormatFileSystem::Fat32,
                                "FAT32 (Required for <=32GB cards & older handhelds/GarlicOS)",
                            );
                        });

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label("Volume Label:");
                            ui.text_edit_singleline(&mut self.volume_label);
                        });

                        ui.add_space(6.0);
                        ui.colored_label(
                            Color32::from_rgb(255, 170, 0),
                            "⚠️ WARNING: Quick formatting will permanently erase all data on the selected drive!",
                        );
                        ui.checkbox(
                            &mut self.format_confirmed,
                            "I confirm that I want to format this drive and lose any existing files.",
                        );
                    });
                }
            });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("SD Card Subfolder:").strong());
                ui.text_edit_singleline(&mut self.subfolder_name);
                ui.label("(Leave blank for SD card root, or e.g. 'roms' or 'Roms')");
            });
        });

        ui.add_space(15.0);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(RichText::new("💡 Handheld SD Guidance:").strong().color(Color32::from_rgb(100, 200, 255)));
            ui.label("• Anbernic RG DS Launcher / RG Cube / Odin: Format as exFAT; ROMs inside 'roms/'.");
            ui.label("• Anbernic RG35XX (GarlicOS) / Miyoo Mini (OnionOS): Best with FAT32; ROMs inside 'Roms/'.");
            ui.label("• Steam Deck / ES-DE: Standard exFAT; ROMs inside 'roms/'.");
        });
    }

    fn render_profile_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Step 2: Select Device & Launcher Profile");
        ui.label("Choose your target emulation frontend or handheld device. Folder structures and boxart formats will adjust automatically.");
        ui.add_space(10.0);

        for profile in PROFILES {
            let is_selected = self.selected_profile_id == profile.id;
            egui::Frame::group(ui.style())
                .stroke(if is_selected {
                    Stroke::new(2.0, Color32::from_rgb(70, 150, 255))
                } else {
                    Stroke::new(1.0, Color32::from_gray(60))
                })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .radio(is_selected, RichText::new(profile.name).strong().size(15.0))
                            .clicked()
                        {
                            self.selected_profile_id = profile.id;
                            self.subfolder_name = profile.recommended_sd_subfolder.to_string();
                        }
                    });
                    ui.label(RichText::new(profile.description).color(Color32::LIGHT_GRAY));
                    ui.add_space(3.0);
                    ui.label(
                        RichText::new(format!("Folder Layout: {}", profile.guidance))
                            .color(Color32::from_rgb(130, 210, 130)),
                    );
                });
            ui.add_space(4.0);
        }
    }

    fn render_platforms_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Step 3: Source ROMs & Curated Favorites");
        ui.label("Select the root directory containing your ROM folders. Retro CardMaker will automatically match platforms, count games, and allow selecting Curated Favorites.");
        ui.add_space(10.0);

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Source Directory:").strong());
                ui.text_edit_singleline(&mut self.source_path);
                if ui.button("📂 Browse...").clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.source_path = folder.to_string_lossy().to_string();
                        self.scan_source_folder();
                    }
                }
                if ui.button("🔍 Scan Folder").clicked() {
                    self.scan_source_folder();
                }
            });
        });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Platforms Registry:").strong());
            if ui.button("Select All Platforms").clicked() {
                for p in &mut self.platform_states {
                    p.enabled = true;
                }
            }
            if ui.button("Deselect All").clicked() {
                for p in &mut self.platform_states {
                    p.enabled = false;
                }
            }
        });

        ui.add_space(8.0);
        for idx in 0..self.platform_states.len() {
            let p_state = &mut self.platform_states[idx];
            let is_found = p_state.found_dir.is_some();
            let rom_count = p_state.rom_files.len();

            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut p_state.enabled, "");
                    ui.label(
                        RichText::new(p_state.platform.name)
                            .strong()
                            .size(15.0)
                            .color(if is_found {
                                Color32::WHITE
                            } else {
                                Color32::GRAY
                            }),
                    );

                    if is_found {
                        ui.label(
                            RichText::new(format!("({} ROMs found)", rom_count))
                                .color(Color32::from_rgb(100, 230, 100)),
                        );
                    } else {
                        ui.label(RichText::new("(Folder not found)").color(Color32::DARK_GRAY));
                    }
                });

                if is_found {
                    ui.indent("platform_details", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Copy Mode:");
                            ui.radio_value(&mut p_state.mode, CopyMode::AllRoms, "Full ROM List");
                            ui.radio_value(
                                &mut p_state.mode,
                                CopyMode::FavoritesOnly,
                                "Curated Only (favorites.json)",
                            );

                            ui.separator();
                            if let Some(ref favs) = p_state.favorites {
                                ui.label(
                                    RichText::new(format!(
                                        "favorites.json: {} entries",
                                        favs.games.len() + favs.patterns.len()
                                    ))
                                    .color(Color32::from_rgb(100, 200, 255)),
                                );
                            } else {
                                ui.label(
                                    RichText::new("favorites.json: Missing")
                                        .color(Color32::from_rgb(255, 180, 80)),
                                );
                            }

                            if ui.button("⭐ Edit / Generate favorites.json").clicked() {
                                self.editing_platform_idx = Some(idx);
                                self.editing_selected_games.clear();
                                // Pre-fill with current favorites or default recommendations
                                if let Some(ref favs) = p_state.favorites {
                                    for g in &favs.games {
                                        self.editing_selected_games.insert(g.clone());
                                    }
                                } else {
                                    // Seed with default top classics
                                    let default_favs = FavoritesList::from_default_platform(&p_state.platform);
                                    for g in &default_favs.games {
                                        self.editing_selected_games.insert(g.clone());
                                    }
                                }
                            }
                        });
                    });
                }
            });
            ui.add_space(4.0);
        }
    }

    fn render_artwork_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Step 4: Boxart & Artwork Scraping");
        ui.label("Automatically fetch cover boxarts and title screens from the Libretro Thumbnails database and name them according to your launcher profile.");
        ui.add_space(10.0);

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.checkbox(
                &mut self.download_art,
                RichText::new("Enable Boxart Downloader during installation")
                    .strong()
                    .size(15.0),
            );
            ui.add_space(6.0);
            ui.label("• Source: Libretro Thumbnails GitHub Raw CDN (Official, high-res PNG, free, no API keys or accounts required).");
            ui.label("• Smart matching: Strips region tags and normalizes illegal characters according to Libretro rules.");
            ui.label("• Preserves bandwidth: Skips any boxart that is already present on the destination card.");
        });

        ui.add_space(10.0);
        let profile = LauncherProfile::get_by_id(self.selected_profile_id);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(RichText::new("Current Profile Boxart Pattern:").strong().color(Color32::from_rgb(120, 210, 255)));
            ui.label(format!("Frontend: {}", profile.name));
            ui.label(format!("Target Art Path Layout: {}", profile.guidance));
            ui.add_space(4.0);
            ui.label("Example:");
            let dummy_platform = find_platform_by_id("gba").unwrap();
            let dummy_dest = Path::new("E:\\roms");
            let art_example = profile.get_art_destination(dummy_dest, dummy_platform, "Pokemon - Emerald Version (USA)");
            ui.code(format!("{}", art_example.display()));
        });
    }

    fn render_install_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Step 5: Review & Run QuickInstaller");
        ui.add_space(6.0);

        // Summary Card
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(RichText::new("Installation Configuration Summary:").strong());
            ui.add_space(4.0);

            if let Some(drive) = self.drives.get(self.selected_drive_idx) {
                ui.label(format!("• Target SD Card: {} ({})", drive.letter, drive.display_summary()));
            }

            ui.label(format!(
                "• Formatting: {}",
                if self.do_format {
                    format!("Format as {} (Label: '{}')", self.format_fs.as_str(), self.volume_label)
                } else {
                    "Skip formatting (keep existing files)".to_string()
                }
            ));

            let profile = LauncherProfile::get_by_id(self.selected_profile_id);
            ui.label(format!("• Launcher Profile: {}", profile.name));
            ui.label(format!("• Target Folder: {}/{}", self.drives.get(self.selected_drive_idx).map(|d| d.letter.as_str()).unwrap_or("?"), self.subfolder_name));
            ui.label(format!("• Download Boxarts: {}", if self.download_art { "Yes" } else { "No" }));

            let active_platforms: Vec<_> = self.platform_states.iter().filter(|p| p.enabled && p.found_dir.is_some()).collect();
            ui.label(format!("• Platforms to Process ({}):", active_platforms.len()));
            for p in &active_platforms {
                let mode_str = match p.mode {
                    CopyMode::AllRoms => format!("All {} ROMs", p.rom_files.len()),
                    CopyMode::FavoritesOnly => "Curated Favorites Only".to_string(),
                };
                ui.label(format!("    - {}: {}", p.platform.name, mode_str));
            }
        });

        ui.add_space(12.0);

        // Start / Cancel Controls
        ui.horizontal(|ui| {
            if !self.is_running {
                let can_start = !self.drives.is_empty()
                    && (!self.do_format || self.format_confirmed)
                    && self.platform_states.iter().any(|p| p.enabled && p.found_dir.is_some());

                let start_btn = egui::Button::new(
                    RichText::new("🚀 Start QuickInstaller")
                        .size(16.0)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(Color32::from_rgb(30, 160, 60));

                if ui.add_enabled(can_start, start_btn).clicked() {
                    self.start_install();
                }

                if !can_start {
                    if self.do_format && !self.format_confirmed {
                        ui.colored_label(Color32::from_rgb(255, 180, 80), "Please confirm format checkbox in Step 1.");
                    } else {
                        ui.colored_label(Color32::from_rgb(255, 180, 80), "Ensure SD card and ROM source are selected.");
                    }
                }
            } else {
                let cancel_btn = egui::Button::new(
                    RichText::new("🛑 Cancel QuickInstaller")
                        .size(15.0)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(Color32::from_rgb(200, 50, 50));

                if ui.add(cancel_btn).clicked() {
                    self.cancel_flag.store(true, Ordering::Relaxed);
                    self.logs.push("Cancelling operation...".to_string());
                }
            }
        });

        // Progress Section
        ui.add_space(10.0);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(RichText::new(format!("Current Phase: {}", self.current_phase)).strong());
            ui.add_space(4.0);

            let progress_fraction = if self.progress_total > 0 {
                self.progress_current as f32 / self.progress_total as f32
            } else {
                0.0
            };

            ui.add(
                egui::ProgressBar::new(progress_fraction)
                    .show_percentage()
                    .animate(self.is_running),
            );

            if !self.current_item.is_empty() {
                ui.label(format!("Processing ({}/{}): {}", self.progress_current, self.progress_total, self.current_item));
            }
        });

        // Activity Log Console
        ui.add_space(8.0);
        ui.label(RichText::new("Live Activity Log:").strong());
        egui::Frame::dark_canvas(ui.style()).show(ui, |ui| {
            ScrollArea::vertical()
                .max_height(220.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in &self.logs {
                        ui.label(
                            RichText::new(line)
                                .monospace()
                                .size(11.0)
                                .color(Color32::from_rgb(200, 225, 255)),
                        );
                    }
                });
        });

        // Final result alert
        if let Some(ref summary) = self.install_summary {
            ui.add_space(8.0);
            ui.colored_label(
                Color32::from_rgb(80, 240, 80),
                format!(
                    "🎉 Complete! Copied {} ROMs, downloaded {} boxarts ({:.1} MB).",
                    summary.total_roms_copied,
                    summary.total_art_downloaded,
                    summary.total_bytes_copied as f64 / (1024.0 * 1024.0)
                ),
            );
        }

        if let Some(ref err) = self.install_error {
            ui.add_space(8.0);
            ui.colored_label(Color32::from_rgb(255, 90, 90), format!("❌ Error: {}", err));
        }
    }

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
            .default_size(Vec2::new(600.0, 500.0))
            .show(ui.ctx(), |ui| {
                ui.label("Select the games you want to include in this platform's favorites.json list.");
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    if ui.button("Select Recommended Top Classics").clicked() {
                        let top = FavoritesList::from_default_platform(&p_state.platform);
                        for item in top.games {
                            self.editing_selected_games.insert(item);
                        }
                    }
                    if ui.button("Select All Discovered").clicked() {
                        for rom in &p_state.rom_files {
                            self.editing_selected_games.insert(rom.clone());
                        }
                    }
                    if ui.button("Clear Selection").clicked() {
                        self.editing_selected_games.clear();
                    }
                });

                ui.add_space(6.0);
                ui.separator();
                ui.label(RichText::new("Add Custom Game / Search Pattern:").strong());
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.new_favorite_pattern);
                    if ui.button("➕ Add Pattern").clicked() && !self.new_favorite_pattern.trim().is_empty() {
                        self.editing_selected_games.insert(self.new_favorite_pattern.trim().to_string());
                        self.new_favorite_pattern.clear();
                    }
                });

                ui.add_space(6.0);
                ui.separator();
                ui.label(RichText::new(format!("Discovered ROM Files ({}):", p_state.rom_files.len())).strong());

                ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    if p_state.rom_files.is_empty() {
                        ui.label("No ROM files discovered in this platform directory yet.");
                    } else {
                        for rom in &p_state.rom_files {
                            let mut is_checked = self.editing_selected_games.contains(rom);
                            if ui.checkbox(&mut is_checked, rom).changed() {
                                if is_checked {
                                    self.editing_selected_games.insert(rom.clone());
                                } else {
                                    self.editing_selected_games.remove(rom);
                                }
                            }
                        }
                    }
                });

                ui.add_space(8.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("💾 Save favorites.json").strong().color(Color32::WHITE)).clicked() {
                        do_save = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close_modal = true;
                    }
                });
            });

        if do_save {
            if let Some(ref dir) = self.platform_states[p_idx].found_dir {
                let games_list: Vec<String> = self.editing_selected_games.iter().cloned().collect();
                let favs = FavoritesList {
                    platform: self.platform_states[p_idx].platform.id.to_string(),
                    title: format!("{} Curated Favorites", self.platform_states[p_idx].platform.name),
                    games: games_list.clone(),
                    patterns: games_list,
                };

                let _ = save_favorites(dir, &favs);
                self.platform_states[p_idx].favorites = Some(favs);
            }
            close_modal = true;
        }

        if close_modal {
            self.editing_platform_idx = None;
            self.editing_selected_games.clear();
        }
    }
}
