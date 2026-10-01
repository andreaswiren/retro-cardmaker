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
