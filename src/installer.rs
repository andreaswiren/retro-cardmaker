use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use walkdir::WalkDir;

use crate::art_scraper::ArtScraper;
use crate::drives::{format_drive, FormatFileSystem};
use crate::favorites::{load_favorites, FavoritesList};
use crate::launcher_profiles::{LauncherProfile, ProfileId};
use crate::platforms::{find_platform_by_id, PlatformInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyMode {
    AllRoms,
    FavoritesOnly,
}

#[derive(Debug, Clone)]
pub struct PlatformInstallConfig {
    pub platform_id: String,
    pub source_dir: PathBuf,
    pub mode: CopyMode,
    pub favorites: Option<FavoritesList>,
}

#[derive(Debug, Clone)]
pub struct InstallConfig {
    pub drive_letter: String,
    pub destination_path: PathBuf,
    pub format_option: Option<FormatFileSystem>,
    pub volume_label: String,
    pub profile_id: ProfileId,
    pub platforms: Vec<PlatformInstallConfig>,
    pub download_art: bool,
}

#[derive(Debug, Clone)]
pub struct InstallSummary {
    pub total_roms_copied: usize,
    pub total_art_downloaded: usize,
    pub total_bytes_copied: u64,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum InstallerEvent {
    Log(String),
    Phase(String),
    Progress {
        current: usize,
        total: usize,
        item: String,
    },
    Success(InstallSummary),
    Failed(String),
}

pub struct InstallerEngine;

impl InstallerEngine {
    pub fn run(
        config: InstallConfig,
        cancel_flag: Arc<AtomicBool>,
        tx: Sender<InstallerEvent>,
    ) {
        let _ = tx.send(InstallerEvent::Phase("Preparing QuickInstaller...".to_string()));
        let mut summary = InstallSummary {
            total_roms_copied: 0,
            total_art_downloaded: 0,
            total_bytes_copied: 0,
            errors: Vec::new(),
        };

        // 1. Format drive if requested
        if let Some(fs) = config.format_option {
            let _ = tx.send(InstallerEvent::Phase(format!("Formatting SD Card ({:?})...", fs)));
            let _ = tx.send(InstallerEvent::Log(format!(
                "Formatting drive {} with {} filesystem...",
                config.drive_letter,
                fs.as_str()
            )));

            match format_drive(&config.drive_letter, fs, &config.volume_label) {
                Ok(msg) => {
                    let _ = tx.send(InstallerEvent::Log(msg));
                }
                Err(err) => {
                    let _ = tx.send(InstallerEvent::Failed(format!("Formatting failed: {}", err)));
                    return;
                }
            }
        }

        if cancel_flag.load(Ordering::Relaxed) {
            let _ = tx.send(InstallerEvent::Failed("Operation cancelled by user.".to_string()));
            return;
        }

        // 2. Ensure base destination exists
        if let Err(e) = fs::create_dir_all(&config.destination_path) {
            let _ = tx.send(InstallerEvent::Failed(format!(
                "Failed to create destination folder {}: {}",
                config.destination_path.display(),
                e
            )));
            return;
        }

        let profile = LauncherProfile::get_by_id(config.profile_id);
        let scraper = ArtScraper::new();

        // 3. Scan all files to copy across platforms
        let _ = tx.send(InstallerEvent::Phase("Scanning ROM directories...".to_string()));
        let mut tasks: Vec<(PlatformInfo, PathBuf, String)> = Vec::new(); // (platform, src_path, filename)

        for p_cfg in &config.platforms {
            if cancel_flag.load(Ordering::Relaxed) {
                let _ = tx.send(InstallerEvent::Failed("Operation cancelled by user.".to_string()));
                return;
            }

            let platform = match find_platform_by_id(&p_cfg.platform_id) {
                Some(p) => p.clone(),
                None => continue,
            };

            if !p_cfg.source_dir.exists() {
                let _ = tx.send(InstallerEvent::Log(format!(
                    "Warning: Source folder for {} not found: {}",
                    platform.name,
                    p_cfg.source_dir.display()
                )));
                continue;
            }

            // Load favorites if needed
            let effective_favorites = if p_cfg.mode == CopyMode::FavoritesOnly {
                p_cfg.favorites.clone().or_else(|| load_favorites(&p_cfg.source_dir))
            } else {
                None
            };

            let mut platform_found = 0;
            for entry in WalkDir::new(&p_cfg.source_dir)
                .max_depth(2)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() {
                    let path = entry.path();
                    let filename = match path.file_name().and_then(|s| s.to_str()) {
                        Some(f) => f.to_string(),
                        None => continue,
                    };

                    // Check extension
                    let has_valid_ext = platform
                        .extensions
                        .iter()
                        .any(|&ext| filename.to_lowercase().ends_with(ext));

                    if !has_valid_ext {
                        continue;
                    }

                    // Check favorites filter if in FavoritesOnly mode
                    if let Some(ref favs) = effective_favorites {
                        if !favs.is_match(&filename) {
                            continue;
                        }
                    }

                    tasks.push((platform.clone(), path.to_path_buf(), filename));
                    platform_found += 1;
                }
            }

            let _ = tx.send(InstallerEvent::Log(format!(
                "Platform [{}]: queued {} files (Mode: {:?})",
                platform.name, platform_found, p_cfg.mode
            )));
        }

        let total_tasks = tasks.len();
        let _ = tx.send(InstallerEvent::Log(format!(
            "Total ROMs to transfer: {}",
            total_tasks
        )));

        // 4. Copy ROMs and optionally download artwork
        let _ = tx.send(InstallerEvent::Phase("Copying ROMs & Setting Up Art...".to_string()));

        for (idx, (platform, src_path, filename)) in tasks.iter().enumerate() {
            if cancel_flag.load(Ordering::Relaxed) {
                let _ = tx.send(InstallerEvent::Failed("Operation cancelled by user.".to_string()));
                return;
            }

            let current_num = idx + 1;
            let _ = tx.send(InstallerEvent::Progress {
                current: current_num,
                total: total_tasks,
                item: filename.clone(),
            });

            // Target ROM path
            let dest_rom_path = profile.get_rom_destination(
                &config.destination_path,
                platform,
                filename,
            );

            if let Some(parent) = dest_rom_path.parent() {
                if !parent.exists() {
                    let _ = fs::create_dir_all(parent);
                }
            }

            // Copy file if not exists or different size
            let mut need_copy = true;
            if dest_rom_path.exists() {
                if let (Ok(src_meta), Ok(dest_meta)) = (src_path.metadata(), dest_rom_path.metadata()) {
                    if src_meta.len() == dest_meta.len() {
                        need_copy = false;
                    }
                }
            }

            if need_copy {
                match fs::copy(src_path, &dest_rom_path) {
                    Ok(bytes) => {
                        summary.total_roms_copied += 1;
                        summary.total_bytes_copied += bytes;
                    }
                    Err(e) => {
                        let err_msg = format!("Failed to copy {}: {}", filename, e);
                        let _ = tx.send(InstallerEvent::Log(err_msg.clone()));
                        summary.errors.push(err_msg);
                    }
                }
            } else {
                summary.total_roms_copied += 1;
            }

            // Artwork processing
            if config.download_art {
                let stem = Path::new(filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(filename);

                let art_dest = profile.get_art_destination(
                    &config.destination_path,
                    platform,
                    stem,
                );

                if !art_dest.exists() {
                    match scraper.download_boxart(platform, filename, &art_dest) {
                        Ok(true) => {
                            summary.total_art_downloaded += 1;
                            let _ = tx.send(InstallerEvent::Log(format!(
                                "Downloaded art for: {}",
                                stem
                            )));
                        }
                        Ok(false) => {}
                        Err(_) => {
                            // Non-critical, just keep going
                        }
                    }
                }
            }
        }

        // 5. Special Frontend Additions (e.g. Pegasus metadata template)
        if config.profile_id == ProfileId::Pegasus {
            let pegasus_meta = config.destination_path.join("metadata.pegasus.txt");
            if !pegasus_meta.exists() {
                let _ = fs::write(
                    &pegasus_meta,
                    "# Pegasus Frontend Configuration generated by Retro CardMaker\ncollection: All Games\n",
                );
            }
        }

        let _ = tx.send(InstallerEvent::Phase("QuickInstaller Completed!".to_string()));
        let _ = tx.send(InstallerEvent::Success(summary));
    }
}
