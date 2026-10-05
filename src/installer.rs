use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use walkdir::WalkDir;

use crate::art_scraper::{ArtScraper, ArtType};
use crate::dedup::{filter_roms_1g1r, RegionPreference};
use crate::drives::{format_drive, wipe_and_repartition_drive, FormatFileSystem};
use crate::favorites::{load_favorites, FavoritesList};
use crate::launcher_profiles::{LauncherProfile, ProfileId};
use crate::platforms::{filter_primary_rom_files, find_platform_by_id, get_companion_files, PlatformInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyMode {
    AllRoms,
    FavoritesOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ArtLocationMode {
    /// Save directly to the SD card destination directory (e.g. <SD>/Roms/snes/Imgs)
    TargetDriveOnly,
    /// Save to a subfolder of the ROM source folder on PC (e.g. <source>/Imgs or <source>/covers) and sync to SD
    RomSourceSubfolder,
    /// Save to a local temporary cache folder (%TEMP%\retro-cardmaker\art_cache) and sync to SD
    TempCacheFolder,
}

impl Default for ArtLocationMode {
    fn default() -> Self {
        Self::RomSourceSubfolder
    }
}

impl ArtLocationMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::TargetDriveOnly => "SD Card Only",
            Self::RomSourceSubfolder => "ROM Source Subfolder",
            Self::TempCacheFolder => "Temp Cache Folder",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::TargetDriveOnly => "Downloads artwork directly onto SD card structure only.",
            Self::RomSourceSubfolder => "Saves artwork inside your PC's ROM folder (e.g. Imgs/) permanently, and copies to SD.",
            Self::TempCacheFolder => "Caches artwork in system temp (%TEMP%/retro-cardmaker) and copies to SD.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerExecutionMode {
    /// Full installation: formats (if enabled), copies all ROMs (Phase 1), then syncs boxart (Phase 2)
    FullInstall,
    /// Only copies ROMs and companion files without boxart
    RomsOnly,
    /// Only scrapes/downloads boxart to the designated target without copying ROMs or formatting
    ArtOnly,
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
    pub wipe_and_repartition: bool,
    pub volume_label: String,
    pub profile_id: ProfileId,
    pub platforms: Vec<PlatformInstallConfig>,
    pub download_art: bool,
    pub art_types: Vec<ArtType>,
    pub art_location_mode: ArtLocationMode,
    pub art_subfolder_name: String,
    pub region_preference: RegionPreference,
    pub exclude_betas: bool,
    pub copy_threads: usize,
    pub art_threads: usize,
    pub copy_roms_first: bool,
    pub execution_mode: InstallerExecutionMode,
}

#[derive(Debug, Clone)]
pub struct InstallSummary {
    pub total_roms_copied: usize,
    pub total_art_downloaded: usize,
    pub total_bytes_copied: u64,
    pub total_duplicates_filtered: usize,
    pub execution_mode: InstallerExecutionMode,
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

#[derive(Debug, Clone)]
struct CopyTask {
    _platform_name: String,
    src_path: PathBuf,
    dest_path: PathBuf,
    filename: String,
    is_companion: bool,
}

#[derive(Debug, Clone)]
struct ArtTask {
    platform: PlatformInfo,
    filename: String,
    stem: String,
    art_type: ArtType,
    source_dir: PathBuf,
    dest_art_path: Option<PathBuf>,
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
            total_duplicates_filtered: 0,
            execution_mode: config.execution_mode,
            errors: Vec::new(),
        };

        // 1. Format or clean repartition drive if requested (Skipped in ArtOnly mode)
        if config.execution_mode != InstallerExecutionMode::ArtOnly {
            if let Some(fs) = config.format_option {
                if config.wipe_and_repartition {
                    let _ = tx.send(InstallerEvent::Phase(format!("Wiping & Repartitioning SD Card (MBR / {:?})...", fs)));
                    let _ = tx.send(InstallerEvent::Log(format!(
                        "Wiping hidden partitions and repartitioning drive {} with {} filesystem...",
                        config.drive_letter,
                        fs.as_str()
                    )));

                    match wipe_and_repartition_drive(&config.drive_letter, fs, &config.volume_label) {
                        Ok(msg) => {
                            let _ = tx.send(InstallerEvent::Log(msg));
                        }
                        Err(err) => {
                            let _ = tx.send(InstallerEvent::Failed(format!("Clean repartitioning failed: {}", err)));
                            return;
                        }
                    }
                } else {
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
            }
        }

        if cancel_flag.load(Ordering::Relaxed) {
            let _ = tx.send(InstallerEvent::Failed("Operation cancelled by user.".to_string()));
            return;
        }

        // 2. Ensure base destination exists if writing to target drive
        if config.execution_mode != InstallerExecutionMode::ArtOnly || config.art_location_mode == ArtLocationMode::TargetDriveOnly {
            if let Err(e) = fs::create_dir_all(&config.destination_path) {
                let _ = tx.send(InstallerEvent::Failed(format!(
                    "Failed to create destination folder {}: {}",
                    config.destination_path.display(),
                    e
                )));
                return;
            }
        }

        let profile = LauncherProfile::get_by_id(config.profile_id);

        // 3. Scan ROM directories and build task queues
        let _ = tx.send(InstallerEvent::Phase("Scanning ROM directories & deduplicating...".to_string()));
        let mut copy_tasks: Vec<CopyTask> = Vec::new();
        let mut art_tasks: Vec<ArtTask> = Vec::new();

        let effective_art_types = if config.art_types.is_empty() {
            vec![ArtType::Boxart]
        } else {
            config.art_types.clone()
        };

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

            let mut found_files = Vec::new();
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

                    found_files.push((path.to_path_buf(), filename));
                }
            }

            // Filter for primary game entries
            let raw_filenames: Vec<String> = found_files.iter().map(|(_, name)| name.clone()).collect();
            let primary_filenames = filter_primary_rom_files(platform.id, &raw_filenames);

            // Apply 1G1R deduplication and region preference
            let dedup_result = filter_roms_1g1r(&primary_filenames, config.region_preference, config.exclude_betas);
            summary.total_duplicates_filtered += dedup_result.duplicates_filtered;
            let primary_set: std::collections::HashSet<_> = dedup_result.kept_files.into_iter().collect();

            let mut platform_found = 0;
            for (path, filename) in found_files {
                if !primary_set.contains(&filename) {
                    continue;
                }

                // Check favorites filter if in FavoritesOnly mode
                if let Some(ref favs) = effective_favorites {
                    if !favs.is_match(&filename) {
                        continue;
                    }
                }

                platform_found += 1;

                // 3a. Prepare ROM copy task
                if config.execution_mode != InstallerExecutionMode::ArtOnly {
                    let dest_rom_path = profile.get_rom_destination(
                        &config.destination_path,
                        &platform,
                        &filename,
                    );

                    copy_tasks.push(CopyTask {
                        _platform_name: platform.name.to_string(),
                        src_path: path.clone(),
                        dest_path: dest_rom_path,
                        filename: filename.clone(),
                        is_companion: false,
                    });

                    // Collect companion files (audio tracks, multiple bin data tracks)
                    let companions = get_companion_files(&path);
                    for comp_path in companions {
                        if let Some(name_str) = comp_path.file_name().and_then(|s| s.to_str()) {
                            let comp_filename = name_str.to_string();
                            let dest_comp_path = profile.get_rom_destination(
                                &config.destination_path,
                                &platform,
                                &comp_filename,
                            );
                            copy_tasks.push(CopyTask {
                                _platform_name: platform.name.to_string(),
                                src_path: comp_path,
                                dest_path: dest_comp_path,
                                filename: comp_filename,
                                is_companion: true,
                            });
                        }
                    }
                }

                // 3b. Prepare artwork tasks for each requested ArtType
                if config.download_art && config.execution_mode != InstallerExecutionMode::RomsOnly {
                    let stem = Path::new(&filename)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&filename)
                        .to_string();

                    for &art_type in &effective_art_types {
                        let dest_art_path = Some(profile.get_typed_art_destination(
                            &config.destination_path,
                            &platform,
                            &stem,
                            art_type,
                        ));

                        art_tasks.push(ArtTask {
                            platform: platform.clone(),
                            filename: filename.clone(),
                            stem: stem.clone(),
                            art_type,
                            source_dir: p_cfg.source_dir.clone(),
                            dest_art_path,
                        });
                    }
                }
            }

            let _ = tx.send(InstallerEvent::Log(format!(
                "Platform [{}]: discovered {} unique games (Deduplication: {:?}, Mode: {:?})",
                platform.name, platform_found, config.region_preference, p_cfg.mode
            )));
        }

        // ====================================================================
        // PHASE 1: HIGH-SPEED PARALLEL ROM COPYING
        // ====================================================================
        if config.execution_mode != InstallerExecutionMode::ArtOnly && !copy_tasks.is_empty() {
            let num_workers = config.copy_threads.clamp(1, 16);
            let total_copy_items = copy_tasks.len();
            let _ = tx.send(InstallerEvent::Phase(format!(
                "Phase 1/2: Copying ROMs ({} parallel workers)...",
                num_workers
            )));
            let _ = tx.send(InstallerEvent::Log(format!(
                "🚀 Starting parallel transfer of {} ROM/companion files across {} threads...",
                total_copy_items, num_workers
            )));

            let copy_queue = Arc::new(Mutex::new(VecDeque::from(copy_tasks)));
            let copy_completed = Arc::new(AtomicUsize::new(0));
            let copy_roms_count = Arc::new(AtomicUsize::new(0));
            let copy_bytes_count = Arc::new(AtomicU64::new(0));
            let copy_errors = Arc::new(Mutex::new(Vec::<String>::new()));

            let mut worker_handles = Vec::new();
            for worker_id in 0..num_workers {
                let queue = copy_queue.clone();
                let cancel = cancel_flag.clone();
                let tx = tx.clone();
                let completed = copy_completed.clone();
                let roms_count = copy_roms_count.clone();
                let bytes_count = copy_bytes_count.clone();
                let errors = copy_errors.clone();
                let total = total_copy_items;

                let handle = thread::Builder::new()
                    .name(format!("rom-copy-worker-{}", worker_id))
                    .spawn(move || {
                        loop {
                            if cancel.load(Ordering::Relaxed) {
                                break;
                            }
                            let item = {
                                let mut lock = match queue.lock() {
                                    Ok(l) => l,
                                    Err(e) => e.into_inner(),
                                };
                                lock.pop_front()
                            };
                            let Some(task) = item else {
                                break;
                            };

                            // Ensure destination parent directory exists
                            if let Some(parent) = task.dest_path.parent() {
                                if !parent.exists() {
                                    let _ = fs::create_dir_all(parent);
                                }
                            }

                            // Check if file copy is needed
                            let mut need_copy = true;
                            if task.dest_path.exists() {
                                if let (Ok(s_m), Ok(d_m)) = (task.src_path.metadata(), task.dest_path.metadata()) {
                                    if s_m.len() == d_m.len() {
                                        need_copy = false;
                                    }
                                }
                            }

                            if need_copy {
                                match fs::copy(&task.src_path, &task.dest_path) {
                                    Ok(bytes) => {
                                        bytes_count.fetch_add(bytes, Ordering::Relaxed);
                                        if !task.is_companion {
                                            roms_count.fetch_add(1, Ordering::Relaxed);
                                        }
                                    }
                                    Err(e) => {
                                        let err_msg = format!("Failed to copy {}: {}", task.filename, e);
                                        if let Ok(mut errs) = errors.lock() {
                                            errs.push(err_msg.clone());
                                        }
                                        let _ = tx.send(InstallerEvent::Log(format!("ERROR: {}", err_msg)));
                                    }
                                }
                            } else if !task.is_companion {
                                roms_count.fetch_add(1, Ordering::Relaxed);
                            }

                            let current = completed.fetch_add(1, Ordering::Relaxed) + 1;
                            let _ = tx.send(InstallerEvent::Progress {
                                current,
                                total,
                                item: task.filename,
                            });
                        }
                    });

                if let Ok(h) = handle {
                    worker_handles.push(h);
                }
            }

            for handle in worker_handles {
                let _ = handle.join();
            }

            summary.total_roms_copied = copy_roms_count.load(Ordering::Relaxed);
            summary.total_bytes_copied = copy_bytes_count.load(Ordering::Relaxed);
            if let Ok(mut errs) = copy_errors.lock() {
                summary.errors.append(&mut *errs);
            }

            let mb_copied = summary.total_bytes_copied as f64 / (1024.0 * 1024.0);
            let _ = tx.send(InstallerEvent::Log(format!(
                "✓ Phase 1 Complete: {} primary ROMs transferred ({:.2} MB transferred).",
                summary.total_roms_copied, mb_copied
            )));
        }

        if cancel_flag.load(Ordering::Relaxed) {
            let _ = tx.send(InstallerEvent::Failed("Operation cancelled by user.".to_string()));
            return;
        }

        // ====================================================================
        // PHASE 2: HIGH-SPEED PARALLEL ARTWORK SCRAPING & SYNC
        // ====================================================================
        if config.download_art && config.execution_mode != InstallerExecutionMode::RomsOnly && !art_tasks.is_empty() {
            let num_art_workers = config.art_threads.clamp(1, 16);
            let total_art_items = art_tasks.len();
            let base_subfolder_name = if config.art_subfolder_name.trim().is_empty() {
                "Imgs".to_string()
            } else {
                config.art_subfolder_name.trim().to_string()
            };

            let _ = tx.send(InstallerEvent::Phase(format!(
                "Phase 2/2: Syncing Artwork & Media ({}, {} workers)...",
                config.art_location_mode.label(),
                num_art_workers
            )));
            let _ = tx.send(InstallerEvent::Log(format!(
                "🎨 Starting parallel artwork & media sync for {} items to {} ({} threads)...",
                total_art_items,
                config.art_location_mode.label(),
                num_art_workers
            )));

            let art_queue = Arc::new(Mutex::new(VecDeque::from(art_tasks)));
            let art_completed = Arc::new(AtomicUsize::new(0));
            let art_downloaded_count = Arc::new(AtomicUsize::new(0));
            let scraper = Arc::new(ArtScraper::new());

            let mut art_handles = Vec::new();
            for worker_id in 0..num_art_workers {
                let queue = art_queue.clone();
                let cancel = cancel_flag.clone();
                let tx = tx.clone();
                let completed = art_completed.clone();
                let downloaded_count = art_downloaded_count.clone();
                let scraper = scraper.clone();
                let base_subfolder = base_subfolder_name.clone();
                let location_mode = config.art_location_mode;
                let total = total_art_items;

                let handle = thread::Builder::new()
                    .name(format!("art-worker-{}", worker_id))
                    .spawn(move || {
                        loop {
                            if cancel.load(Ordering::Relaxed) {
                                break;
                            }
                            let item = {
                                let mut lock = match queue.lock() {
                                    Ok(l) => l,
                                    Err(e) => e.into_inner(),
                                };
                                lock.pop_front()
                            };
                            let Some(task) = item else {
                                break;
                            };

                            let subfolder = match task.art_type {
                                ArtType::Boxart => base_subfolder.clone(),
                                ArtType::Screenshot => "snaps".to_string(),
                                ArtType::TitleScreen => "titles".to_string(),
                            };
                            let source_art_dir = task.source_dir.join(&subfolder);
                            let temp_cache_dir = ArtScraper::get_typed_temp_art_cache_dir(task.platform.id, task.art_type);

                            // 1. Look for existing local artwork first (instant, 0 network calls!)
                            let local_found = ArtScraper::find_existing_local_art(&source_art_dir, &task.stem)
                                .or_else(|| ArtScraper::find_existing_local_art(&temp_cache_dir, &task.stem));

                            if let Some(local_path) = local_found {
                                // If found in temp cache and mode is RomSourceSubfolder, save to source subfolder too
                                if location_mode == ArtLocationMode::RomSourceSubfolder && !local_path.starts_with(&source_art_dir) {
                                    let _ = fs::create_dir_all(&source_art_dir);
                                    let target_sub = source_art_dir.join(format!("{}.png", task.stem));
                                    let _ = fs::copy(&local_path, &target_sub);
                                }

                                // Copy to SD card destination if specified
                                if let Some(ref dest_art) = task.dest_art_path {
                                    if let Some(parent) = dest_art.parent() {
                                        let _ = fs::create_dir_all(parent);
                                    }
                                    let mut need_copy = true;
                                    if dest_art.exists() {
                                        if let (Ok(s), Ok(d)) = (local_path.metadata(), dest_art.metadata()) {
                                            if s.len() == d.len() {
                                                need_copy = false;
                                            }
                                        }
                                    }
                                    if need_copy {
                                        let _ = fs::copy(&local_path, dest_art);
                                    }
                                }
                                downloaded_count.fetch_add(1, Ordering::Relaxed);
                            } else {
                                // 2. Not found locally: Download from Libretro CDN
                                let primary_save_target = match location_mode {
                                    ArtLocationMode::RomSourceSubfolder => {
                                        let _ = fs::create_dir_all(&source_art_dir);
                                        source_art_dir.join(format!("{}.png", task.stem))
                                    }
                                    ArtLocationMode::TempCacheFolder => {
                                        let _ = fs::create_dir_all(&temp_cache_dir);
                                        temp_cache_dir.join(format!("{}.png", task.stem))
                                    }
                                    ArtLocationMode::TargetDriveOnly => {
                                        if let Some(ref dest) = task.dest_art_path {
                                            dest.clone()
                                        } else {
                                            let _ = fs::create_dir_all(&temp_cache_dir);
                                            temp_cache_dir.join(format!("{}.png", task.stem))
                                        }
                                    }
                                };

                                match scraper.download_artwork(&task.platform, task.art_type, &task.filename, &primary_save_target) {
                                    Ok(true) => {
                                        downloaded_count.fetch_add(1, Ordering::Relaxed);
                                        let _ = tx.send(InstallerEvent::Log(format!(
                                            "[ART] Downloaded {} for '{}' -> {}",
                                            task.art_type.label(),
                                            task.stem,
                                            location_mode.label()
                                        )));

                                        // If stored in source subfolder or cache, also copy to SD destination
                                        if location_mode != ArtLocationMode::TargetDriveOnly {
                                            if let Some(ref dest_art) = task.dest_art_path {
                                                if let Some(parent) = dest_art.parent() {
                                                    let _ = fs::create_dir_all(parent);
                                                }
                                                let _ = fs::copy(&primary_save_target, dest_art);
                                            }
                                        }
                                    }
                                    Ok(false) => {
                                        // Already present
                                        downloaded_count.fetch_add(1, Ordering::Relaxed);
                                    }
                                    Err(_) => {
                                        // Boxart missing on CDN, non-fatal
                                    }
                                }
                            }

                            let current = completed.fetch_add(1, Ordering::Relaxed) + 1;
                            let _ = tx.send(InstallerEvent::Progress {
                                current,
                                total,
                                item: format!("{}: {}", task.art_type.label(), task.stem),
                            });
                        }
                    });

                if let Ok(h) = handle {
                    art_handles.push(h);
                }
            }

            for handle in art_handles {
                let _ = handle.join();
            }

            summary.total_art_downloaded = art_downloaded_count.load(Ordering::Relaxed);
            let _ = tx.send(InstallerEvent::Log(format!(
                "✓ Phase 2 Complete: {} media items downloaded/synced.",
                summary.total_art_downloaded
            )));
        }

        // 5. Special Frontend Additions (e.g. Pegasus metadata template)
        if config.profile_id == ProfileId::Pegasus && config.execution_mode != InstallerExecutionMode::ArtOnly {
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
