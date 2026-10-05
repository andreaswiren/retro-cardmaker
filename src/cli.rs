use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Arc;
use clap::{Parser, Subcommand, ValueEnum};

use crate::drives::{format_drive, get_available_drives, FormatFileSystem};
use crate::favorites::load_favorites;
use crate::installer::{
    ArtLocationMode, CopyMode, InstallConfig, InstallerEngine, InstallerEvent,
    InstallerExecutionMode, PlatformInstallConfig,
};
use crate::launcher_profiles::{ProfileId, PROFILES};
use crate::platforms::PLATFORMS;

#[derive(Parser, Debug)]
#[command(name = "retro-cardmaker")]
#[command(author = "Retro CardMaker Team")]
#[command(version)]
#[command(about = "Quickinstaller for formatting SD-cards, structuring ROMs, curated favorites, and downloading artwork for retro handhelds.", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Run in terminal interactive wizard mode instead of GUI
    #[arg(short, long)]
    pub cli: bool,

    /// Capture screenshot of GUI to file and exit
    #[arg(long)]
    pub screenshot: Option<String>,

    /// Tab to select when taking screenshot (e.g. "dashboard", "favorites", "consoles", "settings")
    #[arg(long)]
    pub screenshot_tab: Option<String>,

    /// Open favorites modal when taking screenshot
    #[arg(long)]
    pub screenshot_modal: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List all available disks and removable SD cards
    ListDrives,

    /// Format a removable SD card to FAT32 or exFAT with safety checks
    Format {
        /// Target drive letter (e.g. E: or F:)
        #[arg(short, long)]
        drive: String,

        /// File system to format to (fat32 or exfat)
        #[arg(long, default_value = "exfat")]
        fs: CliFileSystem,

        /// Volume label name
        #[arg(short, long, default_value = "RETRO")]
        label: String,

        /// Wipe all hidden partitions and cleanly repartition SD card (MBR)
        #[arg(long)]
        wipe_and_repartition: bool,

        /// Confirm format without interactive prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Run quickinstaller to format, structure folders, copy ROMs and download boxart
    Install {
        /// Target drive letter (e.g. E:)
        #[arg(short, long)]
        drive: String,

        /// Destination subfolder on target drive (e.g. "roms", "Roms", or "" for root)
        #[arg(long, default_value = "roms")]
        dest_folder: String,

        /// Source root directory containing platform ROM folders
        #[arg(short, long)]
        source: PathBuf,

        /// Launcher/Device profile
        #[arg(short, long, default_value = "es-de")]
        profile: CliProfile,

        /// Only copy games matching favorites.json
        #[arg(long)]
        favorites_only: bool,

        /// Automatically download boxart from Libretro CDN
        #[arg(long)]
        download_art: bool,

        /// Format SD card before installing
        #[arg(long)]
        format: Option<CliFileSystem>,

        /// Wipe all hidden partitions and cleanly repartition SD card (MBR)
        #[arg(long)]
        wipe_and_repartition: bool,

        /// Comma-separated list of platform IDs (e.g. 'gba,snes,psx,nds' or 'all')
        #[arg(long, default_value = "all")]
        platforms: String,

        /// Artwork storage location: "source" (ROM source subfolder), "temp" (cache), or "sd" (SD card only)
        #[arg(long, default_value = "source")]
        art_location: String,

        /// Subfolder name when storing in ROM source folder (default: "Imgs")
        #[arg(long, default_value = "Imgs")]
        art_subfolder: String,

        /// Number of parallel threads for copying ROMs (default: 4)
        #[arg(long, default_value_t = 4)]
        copy_threads: usize,

        /// Number of parallel threads for syncing boxart (default: 6)
        #[arg(long, default_value_t = 6)]
        art_threads: usize,

        /// Only download/sync boxart without copying ROMs
        #[arg(long)]
        art_only: bool,

        /// Only copy ROMs without downloading boxart
        #[arg(long)]
        roms_only: bool,
    },

    /// Interactive step-by-step terminal wizard
    Wizard,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliFileSystem {
    Fat32,
    Exfat,
}

impl From<CliFileSystem> for FormatFileSystem {
    fn from(c: CliFileSystem) -> Self {
        match c {
            CliFileSystem::Fat32 => FormatFileSystem::Fat32,
            CliFileSystem::Exfat => FormatFileSystem::ExFat,
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliProfile {
    EsDe,
    Retroarch,
    AnbernicDs,
    GarlicOs,
    OnionOs,
    Daijisho,
    Pegasus,
    Batocera,
    Custom,
}

impl From<CliProfile> for ProfileId {
    fn from(p: CliProfile) -> Self {
        match p {
            CliProfile::EsDe => ProfileId::EmulationStationEsDe,
            CliProfile::Retroarch => ProfileId::RetroArch,
            CliProfile::AnbernicDs => ProfileId::AnbernicRgDsLauncher,
            CliProfile::GarlicOs => ProfileId::AnbernicOlderGarlicOs,
            CliProfile::OnionOs => ProfileId::MiyooMiniOnionOs,
            CliProfile::Daijisho => ProfileId::Daijisho,
            CliProfile::Pegasus => ProfileId::Pegasus,
            CliProfile::Batocera => ProfileId::BatoceraArkOs,
            CliProfile::Custom => ProfileId::Custom,
        }
    }
}

pub fn run_cli_command(command: Commands) {
    match command {
        Commands::ListDrives => {
            println!("\n=== Available Disks and Drives ===");
            let drives = get_available_drives();
            if drives.is_empty() {
                println!("No drives detected.");
            } else {
                for (i, d) in drives.iter().enumerate() {
                    let rem_tag = if d.is_system {
                        "[SYSTEM DRIVE - PROTECTED]"
                    } else if d.is_removable {
                        "[Removable SD/USB]"
                    } else {
                        "[Fixed Drive]"
                    };
                    let hidden_str = if d.has_hidden_partitions {
                        format!(" ⚠️ [Hidden Partitions Detected: Physical {}]", d.physical_gb_str())
                    } else {
                        String::new()
                    };
                    println!(
                        "  {}. {} (Label: '{}') - {} - Free: {} / {} {}{}",
                        i + 1,
                        d.letter,
                        d.label,
                        d.file_system,
                        d.free_gb_str(),
                        d.total_gb_str(),
                        rem_tag,
                        hidden_str
                    );
                }
            }
            println!();
        }

        Commands::Format {
            drive,
            fs,
            label,
            wipe_and_repartition,
            yes,
        } => {
            let clean = drive.trim_end_matches('\\').trim_end_matches('/');
            if clean.eq_ignore_ascii_case("C:") || clean.eq_ignore_ascii_case("C") {
                eprintln!("ERROR: Refusing to format system drive C:!");
                return;
            }

            if !yes {
                let action_desc = if wipe_and_repartition {
                    "WIPE ALL HIDDEN PARTITIONS & REPARTITION MBR"
                } else {
                    "FORMAT"
                };
                print!(
                    "WARNING: ALL DATA ON {} WILL BE PERMANENTLY ERASED.\nAction: {}\nAre you sure you want to proceed on {} as {} (Label: '{}')? [y/N]: ",
                    drive, action_desc, drive, format!("{:?}", fs), label
                );
                io::stdout().flush().unwrap();
                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Operation cancelled.");
                    return;
                }
            }

            if wipe_and_repartition {
                println!("Wiping hidden partitions and repartitioning {}...", drive);
                match crate::drives::wipe_and_repartition_drive(&drive, fs.into(), &label) {
                    Ok(msg) => println!("Success: {}", msg),
                    Err(err) => eprintln!("Error: {}", err),
                }
            } else {
                println!("Formatting {}...", drive);
                match format_drive(&drive, fs.into(), &label) {
                    Ok(msg) => println!("Success: {}", msg),
                    Err(err) => eprintln!("Error: {}", err),
                }
            }
        }

        Commands::Install {
            drive,
            dest_folder,
            source,
            profile,
            favorites_only,
            download_art,
            format,
            wipe_and_repartition,
            platforms,
            art_location,
            art_subfolder,
            copy_threads,
            art_threads,
            art_only,
            roms_only,
        } => {
            run_headless_install(
                &drive,
                &dest_folder,
                &source,
                profile.into(),
                favorites_only,
                download_art,
                format.map(Into::into),
                wipe_and_repartition,
                &platforms,
                &art_location,
                &art_subfolder,
                copy_threads,
                art_threads,
                art_only,
                roms_only,
            );
        }

        Commands::Wizard => {
            run_interactive_wizard();
        }
    }
}

fn run_headless_install(
    drive_letter: &str,
    dest_subfolder: &str,
    source_root: &Path,
    profile_id: ProfileId,
    favorites_only: bool,
    download_art: bool,
    format_opt: Option<FormatFileSystem>,
    wipe_and_repartition: bool,
    platforms_filter: &str,
    art_location_str: &str,
    art_subfolder_str: &str,
    copy_threads: usize,
    art_threads: usize,
    art_only: bool,
    roms_only: bool,
) {
    let clean_drive = drive_letter.trim_end_matches('\\').trim_end_matches('/');
    let target_base = if dest_subfolder.trim().is_empty() {
        PathBuf::from(format!("{}\\", clean_drive))
    } else {
        PathBuf::from(format!("{}\\{}", clean_drive, dest_subfolder.trim()))
    };

    let art_mode = match art_location_str.to_lowercase().as_str() {
        "sd" => ArtLocationMode::TargetDriveOnly,
        "temp" => ArtLocationMode::TempCacheFolder,
        _ => ArtLocationMode::RomSourceSubfolder,
    };

    let exec_mode = if art_only {
        InstallerExecutionMode::ArtOnly
    } else if roms_only {
        InstallerExecutionMode::RomsOnly
    } else {
        InstallerExecutionMode::FullInstall
    };

    println!("\n=== Starting Retro CardMaker QuickInstaller ===");
    println!("Target Drive: {}", clean_drive);
    println!("Destination: {}", target_base.display());
    println!("Source Directory: {}", source_root.display());
    println!("Profile: {:?}", profile_id);
    println!("Execution Mode: {:?}", exec_mode);
    println!("Favorites Only: {}", favorites_only);
    println!("Download Boxart: {} (Location: {:?}, Subfolder: '{}')", download_art && !roms_only, art_mode, art_subfolder_str);
    println!("Workers: Copy = {}, Art = {}", copy_threads, art_threads);
    if let Some(f) = format_opt {
        println!("Will Format as: {}", f.as_str());
    }

    let mut platform_configs = Vec::new();
    let filter_list: Vec<&str> = platforms_filter.split(',').map(|s| s.trim()).collect();
    let use_all = filter_list.contains(&"all");

    for p in PLATFORMS {
        if !use_all && !filter_list.iter().any(|&f| f.eq_ignore_ascii_case(p.id)) {
            continue;
        }

        // Look for matching directory in source_root
        let mut found_dir = None;
        for alias in p.folder_aliases {
            let candidate = source_root.join(alias);
            if candidate.is_dir() {
                found_dir = Some(candidate);
                break;
            }
        }

        if let Some(src_dir) = found_dir {
            let favs = load_favorites(&src_dir);
            platform_configs.push(PlatformInstallConfig {
                platform_id: p.id.to_string(),
                source_dir: src_dir,
                mode: if favorites_only {
                    CopyMode::FavoritesOnly
                } else {
                    CopyMode::AllRoms
                },
                favorites: favs,
            });
        }
    }

    if platform_configs.is_empty() {
        println!("No matching platform folders found in {}.", source_root.display());
        return;
    }

    println!("Detected {} platforms to process.", platform_configs.len());

    let config = InstallConfig {
        drive_letter: clean_drive.to_string(),
        destination_path: target_base,
        format_option: format_opt,
        wipe_and_repartition,
        volume_label: "RETRO".to_string(),
        profile_id,
        platforms: platform_configs,
        download_art: download_art && !roms_only,
        art_location_mode: art_mode,
        art_subfolder_name: art_subfolder_str.to_string(),
        copy_threads,
        art_threads,
        copy_roms_first: true,
        execution_mode: exec_mode,
    };

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();

    let runner_flag = cancel_flag.clone();
    std::thread::spawn(move || {
        InstallerEngine::run(config, runner_flag, tx);
    });

    while let Ok(event) = rx.recv() {
        match event {
            InstallerEvent::Phase(p) => println!("\n[Phase] {}", p),
            InstallerEvent::Log(msg) => println!("  {}", msg),
            InstallerEvent::Progress { current, total, item } => {
                print!("\r[Progress] ({}/{}) {}", current, total, item);
                io::stdout().flush().unwrap();
            }
            InstallerEvent::Success(summary) => {
                println!("\n\n=== Installation Succeeded! ===");
                println!("Total ROMs copied: {}", summary.total_roms_copied);
                println!("Total Art downloaded: {}", summary.total_art_downloaded);
                println!("Total Size: {:.2} MB", summary.total_bytes_copied as f64 / (1024.0 * 1024.0));
                if !summary.errors.is_empty() {
                    println!("Warnings ({}):", summary.errors.len());
                    for err in summary.errors.iter().take(5) {
                        println!("  - {}", err);
                    }
                }
                break;
            }
            InstallerEvent::Failed(err) => {
                eprintln!("\nInstallation Failed: {}", err);
                break;
            }
        }
    }
}

pub fn run_interactive_wizard() {
    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║       RETRO CARDMAKER - INTERACTIVE QUICKINSTALLER        ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");

    // 1. Select Drive
    let drives = get_available_drives();
    if drives.is_empty() {
        println!("No drives detected! Please plug in your SD-card or USB drive.");
        return;
    }

    println!("Detected Drives:");
    for (i, d) in drives.iter().enumerate() {
        let tag = if d.is_removable {
            "(Removable SD/USB)"
        } else if d.is_system {
            "(SYSTEM - PROTECTED)"
        } else {
            "(Fixed Disk)"
        };
        println!("  {}) {} [{}] - {} Free: {} {}", i + 1, d.letter, d.label, d.file_system, d.free_gb_str(), tag);
    }

    print!("\nSelect target drive number [1-{}]: ", drives.len());
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let choice: usize = input.trim().parse().unwrap_or(0);
    if choice == 0 || choice > drives.len() {
        println!("Invalid drive choice.");
        return;
    }

    let selected_drive = &drives[choice - 1];
    if selected_drive.is_system {
        println!("Error: System drive C: cannot be selected for safety.");
        return;
    }

    // 2. Format Option
    let (format_opt, wipe_and_repartition) = if !selected_drive.is_removable {
        println!("\nFormatting is disabled: drive {} is not a removable USB or SD card.", selected_drive.letter);
        (None, false)
    } else {
        println!("\nFormat SD Card?");
        println!("Guidance: FAT32 is best for older devices or <=32GB cards. exFAT is best for modern devices & 64GB+.");
        println!("  1) Skip format (keep existing files)");
        println!("  2) Format as exFAT (Recommended for modern handhelds & 64GB+)");
        println!("  3) Format as FAT32 (Best for Miyoo Mini, RG35XX GarlicOS & <=32GB)");
        print!("Choose [1-3] (default 1): ");
        io::stdout().flush().unwrap();
        input.clear();
        io::stdin().read_line(&mut input).unwrap();
        let f_opt = match input.trim() {
            "2" => Some(FormatFileSystem::ExFat),
            "3" => Some(FormatFileSystem::Fat32),
            _ => None,
        };

        let mut do_wipe = false;
        if f_opt.is_some() {
            if selected_drive.has_hidden_partitions {
                println!("\n⚠️ WARNING: Hidden/foreign partitions detected on this card ({} partition vs {} physical)!", selected_drive.total_gb_str(), selected_drive.physical_gb_str());
            }
            print!("\nPerform Full Repartition & Wipe (remove hidden/Linux partitions and restore full capacity)? [y/N]: ");
            io::stdout().flush().unwrap();
            input.clear();
            io::stdin().read_line(&mut input).unwrap();
            do_wipe = input.trim().eq_ignore_ascii_case("y");
        }
        (f_opt, do_wipe)
    };

    // 3. Select Launcher Profile
    println!("\nSelect Device & Frontend Profile:");
    for (i, prof) in PROFILES.iter().enumerate() {
        println!("  {}) {} - {}", i + 1, prof.name, prof.description);
    }
    print!("Choose profile [1-{}]: ", PROFILES.len());
    io::stdout().flush().unwrap();
    input.clear();
    io::stdin().read_line(&mut input).unwrap();
    let prof_idx: usize = input.trim().parse().unwrap_or(1);
    let selected_profile = PROFILES.get(prof_idx.saturating_sub(1)).unwrap_or(&PROFILES[0]);
    println!("\nProfile Guidance: {}", selected_profile.guidance);

    // 4. Source ROMs Folder
    print!("\nEnter path to your source ROMs folder (e.g. D:\\Emulation\\Roms): ");
    io::stdout().flush().unwrap();
    input.clear();
    io::stdin().read_line(&mut input).unwrap();
    let source_path = PathBuf::from(input.trim().trim_matches('"'));
    if !source_path.exists() {
        println!("Source directory '{}' does not exist.", source_path.display());
        return;
    }

    // 5. Favorites Only or Full?
    println!("\nCopy Mode:");
    println!("  1) Full ROM List (copy all discovered ROMs)");
    println!("  2) Curated Only (copy only games listed in favorites.json)");
    print!("Choose [1-2] (default 1): ");
    io::stdout().flush().unwrap();
    input.clear();
    io::stdin().read_line(&mut input).unwrap();
    let favorites_only = input.trim() == "2";

    // 6. Download Artwork?
    print!("\nDownload boxart covers automatically from Libretro CDN? [Y/n]: ");
    io::stdout().flush().unwrap();
    input.clear();
    io::stdin().read_line(&mut input).unwrap();
    let download_art = !input.trim().eq_ignore_ascii_case("n");

    // Execute
    run_headless_install(
        &selected_drive.letter,
        selected_profile.recommended_sd_subfolder,
        &source_path,
        selected_profile.id,
        favorites_only,
        download_art,
        format_opt,
        wipe_and_repartition,
        "all",
        "source",
        "Imgs",
        4,
        6,
        false,
        false,
    );
}
