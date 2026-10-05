use std::path::Path;
use retro_cardmaker::art_scraper::ArtScraper;
use retro_cardmaker::drives::{format_drive, wipe_and_repartition_drive, FormatFileSystem};
use retro_cardmaker::favorites::{FavoritesList, load_favorites, save_favorites};
use retro_cardmaker::launcher_profiles::{LauncherProfile, ProfileId};
use retro_cardmaker::platforms::{find_platform_by_dir_name, find_platform_by_id, PLATFORMS};

#[test]
fn test_platforms_registry() {
    assert!(PLATFORMS.len() >= 14);

    // Verify key platforms exist
    let gba = find_platform_by_id("gba").expect("GBA platform missing");
    assert_eq!(gba.name, "Game Boy Advance");
    assert!(gba.extensions.contains(&".gba"));

    let nds = find_platform_by_id("nds").expect("NDS platform missing");
    assert_eq!(nds.name, "Nintendo DS");
    assert!(nds.extensions.contains(&".nds"));

    let snes = find_platform_by_id("snes").expect("SNES platform missing");
    assert!(snes.extensions.contains(&".sfc"));
    assert!(snes.extensions.contains(&".smc"));

    let psx = find_platform_by_id("psx").expect("PSX platform missing");
    assert!(psx.extensions.contains(&".chd"));
}

#[test]
fn test_platform_alias_resolution() {
    let p_fc = find_platform_by_dir_name("fc").expect("FC alias should resolve to NES");
    assert_eq!(p_fc.id, "nes");

    let p_sfc = find_platform_by_dir_name("sfc").expect("SFC alias should resolve to SNES");
    assert_eq!(p_sfc.id, "snes");

    let p_md = find_platform_by_dir_name("md").expect("MD alias should resolve to Mega Drive");
    assert_eq!(p_md.id, "megadrive");

    let p_gba = find_platform_by_dir_name("GBA").expect("GBA alias should resolve");
    assert_eq!(p_gba.id, "gba");

    let p_ds = find_platform_by_dir_name("Nintendo DS").expect("Nintendo DS alias should resolve");
    assert_eq!(p_ds.id, "nds");
}

#[test]
fn test_launcher_profiles_destinations() {
    let base = Path::new("E:\\roms");
    let gba = find_platform_by_id("gba").unwrap();

    // 1. EmulationStation / ES-DE
    let es_prof = LauncherProfile::get_by_id(ProfileId::EmulationStationEsDe);
    let rom_dest = es_prof.get_rom_destination(base, gba, "Pokemon - Emerald.gba");
    assert_eq!(rom_dest, base.join("gba").join("Pokemon - Emerald.gba"));
    let art_dest = es_prof.get_art_destination(base, gba, "Pokemon - Emerald");
    assert_eq!(
        art_dest,
        base.join("downloaded_media").join("gba").join("covers").join("Pokemon - Emerald.png")
    );

    // 2. Anbernic RG DS Launcher (uses short code 'GBA' and 'Imgs/')
    let anbernic_prof = LauncherProfile::get_by_id(ProfileId::AnbernicRgDsLauncher);
    let anbernic_rom = anbernic_prof.get_rom_destination(base, gba, "Pokemon - Emerald.gba");
    assert_eq!(anbernic_rom, base.join("GBA").join("Pokemon - Emerald.gba"));
    let anbernic_art = anbernic_prof.get_art_destination(base, gba, "Pokemon - Emerald");
    assert_eq!(anbernic_art, base.join("GBA").join("Imgs").join("Pokemon - Emerald.png"));

    // 3. RetroArch (thumbnails/System/Named_Boxarts/)
    let retroarch_prof = LauncherProfile::get_by_id(ProfileId::RetroArch);
    let ra_art = retroarch_prof.get_art_destination(base, gba, "Pokemon - Emerald");
    assert_eq!(
        ra_art,
        base.join("thumbnails")
            .join("Nintendo_-_Game_Boy_Advance")
            .join("Named_Boxarts")
            .join("Pokemon - Emerald.png")
    );

    // 4. Batocera / ArkOS (images/<rom>-image.png)
    let batocera_prof = LauncherProfile::get_by_id(ProfileId::BatoceraArkOs);
    let bato_art = batocera_prof.get_art_destination(base, gba, "Pokemon - Emerald");
    assert_eq!(
        bato_art,
        base.join("gba").join("images").join("Pokemon - Emerald-image.png")
    );

    // 5. Daijishō (thumbnails/<rom>.png)
    let daijisho_prof = LauncherProfile::get_by_id(ProfileId::Daijisho);
    let dai_art = daijisho_prof.get_art_destination(base, gba, "Pokemon - Emerald");
    assert_eq!(
        dai_art,
        base.join("gba").join("thumbnails").join("Pokemon - Emerald.png")
    );
}

#[test]
fn test_favorites_matching_and_io() {
    let favs = FavoritesList {
        platform: "gba".to_string(),
        title: "GBA Best Games".to_string(),
        games: vec![
            "Pokemon - Emerald Version (USA, Europe).gba".to_string(),
            "Metroid Fusion (USA).gba".to_string(),
        ],
        patterns: vec!["Minish Cap".to_string(), "Aria of Sorrow".to_string()],
    };

    // Exact filename match
    assert!(favs.is_match("Pokemon - Emerald Version (USA, Europe).gba"));

    // Stem match
    assert!(favs.is_match("Metroid Fusion (USA).zip"));

    // Pattern match
    assert!(favs.is_match("Legend of Zelda, The - The Minish Cap (USA).gba"));
    assert!(favs.is_match("Castlevania - Aria of Sorrow (USA).gba"));

    // Non-match
    assert!(!favs.is_match("Random Game 123.gba"));

    // Test Save and Load
    let temp_dir = std::env::temp_dir().join("retro_cardmaker_fav_test");
    let _ = std::fs::create_dir_all(&temp_dir);

    let save_res = save_favorites(&temp_dir, &favs);
    assert!(save_res.is_ok());

    let loaded = load_favorites(&temp_dir).expect("Failed to load favorites.json");
    assert_eq!(loaded.platform, "gba");
    assert_eq!(loaded.games.len(), 2);
    assert!(loaded.is_match("Metroid Fusion (USA).gba"));

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_art_scraper_libretro_normalization() {
    // Replaces illegal characters: & * / : ` < > ? \ | "
    let raw_name = "Mario & Luigi: Superstar Saga (USA).gba";
    let normalized = ArtScraper::normalize_for_libretro(raw_name);
    assert_eq!(normalized, "Mario _ Luigi_ Superstar Saga (USA)");

    let raw_2 = "Rockman X* / Mega Man X (Japan).sfc";
    let normalized_2 = ArtScraper::normalize_for_libretro(raw_2);
    assert_eq!(normalized_2, "Rockman X_ _ Mega Man X (Japan)");

    // Test fallback generation
    let fallbacks = ArtScraper::generate_fallback_names("Super Mario World (USA)");
    assert!(fallbacks.contains(&"Super Mario World".to_string()));
    assert!(fallbacks.contains(&"Super Mario World (USA)".to_string()));
}

#[test]
fn test_safety_guard_protects_system_drive() {
    // Attempting to format C: drive must be blocked immediately!
    let res = format_drive("C:", FormatFileSystem::ExFat, "TEST");
    assert!(res.is_err());
    let err = res.err().unwrap();
    assert!(err.contains("SAFETY VIOLATION"));

    let res_lower = format_drive("c:\\", FormatFileSystem::Fat32, "TEST");
    assert!(res_lower.is_err());
    assert!(res_lower.err().unwrap().contains("SAFETY VIOLATION"));
}

#[test]
fn test_safety_guard_protects_repartition() {
    // Attempting to wipe and repartition C: drive must be blocked immediately!
    let res = wipe_and_repartition_drive("C:", FormatFileSystem::ExFat, "TEST");
    assert!(res.is_err());
    let err = res.err().unwrap();
    assert!(err.contains("SAFETY VIOLATION"));

    let res_lower = wipe_and_repartition_drive("c:\\", FormatFileSystem::Fat32, "TEST");
    assert!(res_lower.is_err());
    assert!(res_lower.err().unwrap().contains("SAFETY VIOLATION"));
}

#[test]
fn test_user_requested_platform_aliases() {
    // User requested explicit natural folder names:
    // - gbc is game boy color
    // - gba is game boy advance
    // - sega master system is sega master system
    // - sega mega drive is sega mega drive
    // - sega saturn is sega saturn
    // - sega game gear is sega game gear
    let gbc = find_platform_by_dir_name("game boy color").expect("game boy color should resolve to gbc");
    assert_eq!(gbc.id, "gbc");

    let gba = find_platform_by_dir_name("game boy advance").expect("game boy advance should resolve to gba");
    assert_eq!(gba.id, "gba");

    let sms = find_platform_by_dir_name("sega master system").expect("sega master system should resolve to sms");
    assert_eq!(sms.id, "sms");

    let md = find_platform_by_dir_name("sega mega drive").expect("sega mega drive should resolve to megadrive");
    assert_eq!(md.id, "megadrive");

    let genesis = find_platform_by_dir_name("sega genesis").expect("sega genesis should resolve to megadrive");
    assert_eq!(genesis.id, "megadrive");

    let saturn = find_platform_by_dir_name("sega saturn").expect("sega saturn should resolve to saturn");
    assert_eq!(saturn.id, "saturn");

    let gg = find_platform_by_dir_name("sega game gear").expect("sega game gear should resolve to gamegear");
    assert_eq!(gg.id, "gamegear");

    let gc = find_platform_by_dir_name("gamecube").expect("gamecube should resolve to gamecube");
    assert_eq!(gc.id, "gamecube");

    let wii = find_platform_by_dir_name("nintendo wii").expect("nintendo wii should resolve to wii");
    assert_eq!(wii.id, "wii");
}

#[test]
fn test_font_loading() {
    let segoe_path = std::path::Path::new("C:\\Windows\\Fonts\\segoeui.ttf");
    if segoe_path.exists() {
        let mut fonts = eframe::egui::FontDefinitions::default();
        let data = std::fs::read(segoe_path).unwrap();
        fonts.font_data.insert("SegoeUI".to_owned(), std::sync::Arc::new(eframe::egui::FontData::from_owned(data)));
        fonts.families.entry(eframe::egui::FontFamily::Proportional).or_default().insert(0, "SegoeUI".to_owned());

        let emoji_path = std::path::Path::new("C:\\Windows\\Fonts\\seguiemj.ttf");
        if emoji_path.exists() {
            if let Ok(edata) = std::fs::read(emoji_path) {
                fonts.font_data.insert("SegoeUIEmoji".to_owned(), std::sync::Arc::new(eframe::egui::FontData::from_owned(edata)));
                fonts.families.entry(eframe::egui::FontFamily::Proportional).or_default().push("SegoeUIEmoji".to_owned());
            }
        }

        let ctx = eframe::egui::Context::default();
        ctx.set_fonts(fonts);
        let mut out = ctx.run_ui(Default::default(), |ui| {
            ui.label("🎮 Retro CardMaker 🚀 ⭐ 🛡️");
        });
        out.textures_delta.clear();
    }
}

#[test]
fn test_playstation_multifile_and_audio_track_filtering() {
    use retro_cardmaker::platforms::filter_primary_rom_files;

    let files = vec![
        "Crash Bandicoot (USA).cue".to_string(),
        "Crash Bandicoot (USA) (Track 1).bin".to_string(),
        "Crash Bandicoot (USA) (Track 2).bin".to_string(),
        "Ridge Racer (USA).cue".to_string(),
        "Ridge Racer (USA) (Track 1).bin".to_string(),
        "Ridge Racer (USA) (Track 2).wav".to_string(),
        "Ridge Racer (USA) (Track 3).flac".to_string(),
        "Tekken 3 (USA).chd".to_string(),
        "Castlevania - Symphony of the Night (USA).pbp".to_string(),
        "Spyro the Dragon (USA).bin".to_string(), // standalone orphan bin
        "Final Fantasy VII.m3u".to_string(),
    ];

    let filtered = filter_primary_rom_files("psx", &files);

    // Primary games must be retained:
    assert!(filtered.contains(&"Crash Bandicoot (USA).cue".to_string()));
    assert!(filtered.contains(&"Ridge Racer (USA).cue".to_string()));
    assert!(filtered.contains(&"Tekken 3 (USA).chd".to_string()));
    assert!(filtered.contains(&"Castlevania - Symphony of the Night (USA).pbp".to_string()));
    assert!(filtered.contains(&"Spyro the Dragon (USA).bin".to_string()));
    assert!(filtered.contains(&"Final Fantasy VII.m3u".to_string()));

    // Secondary data and audio tracks must be filtered out:
    assert!(!filtered.contains(&"Crash Bandicoot (USA) (Track 1).bin".to_string()));
    assert!(!filtered.contains(&"Crash Bandicoot (USA) (Track 2).bin".to_string()));
    assert!(!filtered.contains(&"Ridge Racer (USA) (Track 1).bin".to_string()));
    assert!(!filtered.contains(&"Ridge Racer (USA) (Track 2).wav".to_string()));
    assert!(!filtered.contains(&"Ridge Racer (USA) (Track 3).flac".to_string()));
}

#[test]
fn test_playstation_companion_files_resolution() {
    use retro_cardmaker::platforms::get_companion_files;
    use std::fs;

    let temp_dir = std::env::temp_dir().join(format!("test_psx_roms_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    let cue_path = temp_dir.join("Tomb Raider (USA).cue");
    let track1 = temp_dir.join("Tomb Raider (USA) (Track 1).bin");
    let track2 = temp_dir.join("Tomb Raider (USA) (Track 2).bin");
    let audio3 = temp_dir.join("Tomb Raider (USA) (Track 3).wav");

    let cue_content = "FILE \"Tomb Raider (USA) (Track 1).bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\nFILE \"Tomb Raider (USA) (Track 2).bin\" BINARY\n  TRACK 02 AUDIO\n    INDEX 01 00:00:00\n";
    fs::write(&cue_path, cue_content).unwrap();
    fs::write(&track1, b"data1").unwrap();
    fs::write(&track2, b"data2").unwrap();
    fs::write(&audio3, b"audio3").unwrap();

    let companions = get_companion_files(&cue_path);

    assert!(companions.contains(&track1));
    assert!(companions.contains(&track2));
    assert!(companions.contains(&audio3));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_privilege_check_elevation() {
    use retro_cardmaker::drives::is_elevated;
    let elevated = is_elevated();
    println!("Process elevation check: {}", elevated);
}

#[test]
fn test_art_location_mode_and_local_art_lookup() {
    use retro_cardmaker::installer::ArtLocationMode;
    use std::fs;

    assert_eq!(ArtLocationMode::RomSourceSubfolder.label(), "ROM Source Subfolder");
    assert_eq!(ArtLocationMode::TempCacheFolder.label(), "Temp Cache Folder");
    assert_eq!(ArtLocationMode::TargetDriveOnly.label(), "SD Card Only");

    let temp_dir = std::env::temp_dir().join(format!("test_art_lookup_{}", std::process::id()));
    let imgs_dir = temp_dir.join("Imgs");
    let _ = fs::create_dir_all(&imgs_dir);

    let art_file = imgs_dir.join("Super Mario World.png");
    let _ = fs::write(&art_file, b"fake png data");

    // 1. Exact stem lookup
    let found = ArtScraper::find_existing_local_art(&imgs_dir, "Super Mario World");
    assert_eq!(found, Some(art_file.clone()));

    // 2. Extension check with jpg
    let zelda_art = imgs_dir.join("Legend of Zelda, The.jpg");
    let _ = fs::write(&zelda_art, b"fake jpg data");
    let found_zelda = ArtScraper::find_existing_local_art(&imgs_dir, "Legend of Zelda, The");
    assert_eq!(found_zelda, Some(zelda_art));

    // 3. Fallback check (lookup with region tag "Super Mario World (USA)" finds "Super Mario World.png")
    let found_fallback = ArtScraper::find_existing_local_art(&imgs_dir, "Super Mario World (USA)");
    assert_eq!(found_fallback, Some(art_file));

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_installer_parallel_and_two_phase_config() {
    use retro_cardmaker::art_scraper::ArtType;
    use retro_cardmaker::dedup::RegionPreference;
    use retro_cardmaker::installer::{ArtLocationMode, InstallConfig, InstallerExecutionMode};
    use std::path::PathBuf;

    let config = InstallConfig {
        drive_letter: "E:".to_string(),
        destination_path: PathBuf::from("E:\\roms"),
        format_option: None,
        wipe_and_repartition: false,
        volume_label: "RETRO".to_string(),
        profile_id: ProfileId::AnbernicRgDsLauncher,
        platforms: vec![],
        download_art: true,
        art_types: vec![ArtType::Boxart, ArtType::Screenshot],
        art_location_mode: ArtLocationMode::RomSourceSubfolder,
        art_subfolder_name: "Imgs".to_string(),
        region_preference: RegionPreference::UsaFirst,
        exclude_betas: true,
        copy_threads: 4,
        art_threads: 6,
        copy_roms_first: true,
        execution_mode: InstallerExecutionMode::FullInstall,
    };

    assert_eq!(config.copy_threads, 4);
    assert_eq!(config.art_threads, 6);
    assert!(config.copy_roms_first);
    assert_eq!(config.execution_mode, InstallerExecutionMode::FullInstall);
    assert_eq!(config.art_location_mode, ArtLocationMode::RomSourceSubfolder);
    assert_eq!(config.art_types.len(), 2);
    assert_eq!(config.region_preference, RegionPreference::UsaFirst);
}

#[test]
fn test_1g1r_deduplication_regional_preferences() {
    use retro_cardmaker::dedup::{filter_roms_1g1r, RegionPreference};

    let roms = vec![
        "Pokemon - Emerald Version (USA, Europe).gba".to_string(),
        "Pokemon - Emerald Version (Europe) (En,Fr,De,Es,It).gba".to_string(),
        "Pokemon - Emerald Version (Japan).gba".to_string(),
        "Super Mario World (USA).sfc".to_string(),
        "Super Mario World (Europe).sfc".to_string(),
        "Super Mario World (Japan).sfc".to_string(),
        "Super Mario World (USA) (Beta).sfc".to_string(),
    ];

    // 1. USA First (exclude betas)
    let res_usa = filter_roms_1g1r(&roms, RegionPreference::UsaFirst, true);
    assert_eq!(res_usa.kept_files.len(), 2);
    assert!(res_usa.kept_files.contains(&"Pokemon - Emerald Version (USA, Europe).gba".to_string()));
    assert!(res_usa.kept_files.contains(&"Super Mario World (USA).sfc".to_string()));
    assert_eq!(res_usa.duplicates_filtered, 4);
    assert_eq!(res_usa.betas_filtered, 1);

    // 2. Europe First (exclude betas)
    let res_eu = filter_roms_1g1r(&roms, RegionPreference::EuropeFirst, true);
    assert_eq!(res_eu.kept_files.len(), 2);
    assert!(res_eu.kept_files.contains(&"Pokemon - Emerald Version (Europe) (En,Fr,De,Es,It).gba".to_string()));
    assert!(res_eu.kept_files.contains(&"Super Mario World (Europe).sfc".to_string()));

    // 3. Japan First (exclude betas)
    let res_jp = filter_roms_1g1r(&roms, RegionPreference::JapanFirst, true);
    assert_eq!(res_jp.kept_files.len(), 2);
    assert!(res_jp.kept_files.contains(&"Pokemon - Emerald Version (Japan).gba".to_string()));
    assert!(res_jp.kept_files.contains(&"Super Mario World (Japan).sfc".to_string()));

    // 4. None (Keep All Clones, but exclude betas)
    let res_all = filter_roms_1g1r(&roms, RegionPreference::None, true);
    assert_eq!(res_all.kept_files.len(), 6);
    assert!(!res_all.kept_files.contains(&"Super Mario World (USA) (Beta).sfc".to_string()));
}

#[test]
fn test_multi_art_type_routing_launcher_profiles() {
    use retro_cardmaker::art_scraper::ArtType;
    use std::path::Path;

    let base = Path::new("E:\\roms");
    let gba = find_platform_by_id("gba").unwrap();

    // 1. EmulationStation ES-DE: covers, screenshots, titles
    let es_prof = LauncherProfile::get_by_id(ProfileId::EmulationStationEsDe);
    let boxart_dest = es_prof.get_typed_art_destination(base, gba, "Pokemon - Emerald", ArtType::Boxart);
    assert_eq!(boxart_dest, base.join("downloaded_media").join("gba").join("covers").join("Pokemon - Emerald.png"));

    let snap_dest = es_prof.get_typed_art_destination(base, gba, "Pokemon - Emerald", ArtType::Screenshot);
    assert_eq!(snap_dest, base.join("downloaded_media").join("gba").join("screenshots").join("Pokemon - Emerald.png"));

    let title_dest = es_prof.get_typed_art_destination(base, gba, "Pokemon - Emerald", ArtType::TitleScreen);
    assert_eq!(title_dest, base.join("downloaded_media").join("gba").join("titles").join("Pokemon - Emerald.png"));

    // 2. Anbernic RG DS: Imgs, snaps, titles
    let anbernic = LauncherProfile::get_by_id(ProfileId::AnbernicRgDsLauncher);
    let a_box = anbernic.get_typed_art_destination(base, gba, "Pokemon - Emerald", ArtType::Boxart);
    assert_eq!(a_box, base.join("GBA").join("Imgs").join("Pokemon - Emerald.png"));

    let a_snap = anbernic.get_typed_art_destination(base, gba, "Pokemon - Emerald", ArtType::Screenshot);
    assert_eq!(a_snap, base.join("GBA").join("snaps").join("Pokemon - Emerald.png"));

    let a_title = anbernic.get_typed_art_destination(base, gba, "Pokemon - Emerald", ArtType::TitleScreen);
    assert_eq!(a_title, base.join("GBA").join("titles").join("Pokemon - Emerald.png"));
}

#[test]
fn test_anbernic_rg_ds_folder_mappings_matching_sd_card() {
    let anbernic = LauncherProfile::get_by_id(ProfileId::AnbernicRgDsLauncher);

    let check = |id: &str, expected: &str| {
        let p = find_platform_by_id(id).unwrap_or_else(|| panic!("Platform {} not found", id));
        assert_eq!(anbernic.get_platform_folder(p), expected, "Platform {} mapping failed", id);
    };

    check("gbc", "GBC");
    check("gba", "GBA");
    check("gb", "GB");
    check("nds", "NDS");
    check("nes", "FC");
    check("snes", "SFC");
    check("n64", "N64");
    check("sms", "SMS");
    check("megadrive", "MD");
    check("gamegear", "GG");
    check("saturn", "SATURN");
    check("dreamcast", "DREAMCAST");
    check("psx", "PS");
    check("psp", "PSP");
}



