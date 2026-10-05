use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub manufacturer: &'static str,
    pub libretro_name: &'static str,
    pub extensions: &'static [&'static str],
    pub folder_aliases: &'static [&'static str],
    pub default_favorites: &'static [&'static str],
}

pub static PLATFORMS: &[PlatformInfo] = &[
    PlatformInfo {
        id: "nes",
        name: "Nintendo Entertainment System (NES)",
        manufacturer: "Nintendo",
        libretro_name: "Nintendo_-_Nintendo_Entertainment_System",
        extensions: &[".nes", ".zip", ".7z", ".unf", ".fds"],
        folder_aliases: &[
            "nes",
            "NES",
            "fc",
            "FC",
            "famicom",
            "Nintendo - Nintendo Entertainment System",
            "nintendo entertainment system",
            "Nintendo",
        ],
        default_favorites: &[
            "Super Mario Bros. 3",
            "Super Mario Bros.",
            "Legend of Zelda, The",
            "Zelda II - The Adventure of Link",
            "Mega Man 2",
            "Mega Man 3",
            "Castlevania",
            "Castlevania III - Dracula's Curse",
            "Metroid",
            "Contra",
            "Super C",
            "DuckTales",
            "Kirby's Adventure",
            "Mike Tyson's Punch-Out!!",
            "Ninja Gaiden",
        ],
    },
    PlatformInfo {
        id: "snes",
        name: "Super Nintendo (SNES)",
        manufacturer: "Nintendo",
        libretro_name: "Nintendo_-_Super_Nintendo_Entertainment_System",
        extensions: &[".smc", ".sfc", ".zip", ".7z", ".fig"],
        folder_aliases: &[
            "snes",
            "SNES",
            "sfc",
            "SFC",
            "superfamicom",
            "Nintendo - Super Nintendo Entertainment System",
            "super nintendo",
            "super nintendo entertainment system",
        ],
        default_favorites: &[
            "Super Mario World",
            "Super Mario World 2 - Yoshi's Island",
            "Legend of Zelda, The - A Link to the Past",
            "Super Metroid",
            "Chrono Trigger",
            "Donkey Kong Country",
            "Donkey Kong Country 2 - Diddy's Kong Quest",
            "Mega Man X",
            "Super Mario Kart",
            "Final Fantasy III",
            "EarthBound",
            "Street Fighter II Turbo",
            "Super Castlevania IV",
            "Star Fox",
            "F-Zero",
        ],
    },
    PlatformInfo {
        id: "n64",
        name: "Nintendo 64",
        manufacturer: "Nintendo",
        libretro_name: "Nintendo_-_Nintendo_64",
        extensions: &[".n64", ".z64", ".v64", ".zip", ".7z"],
        folder_aliases: &[
            "n64",
            "N64",
            "Nintendo 64",
            "nintendo 64",
            "Nintendo - Nintendo 64",
        ],
        default_favorites: &[
            "Super Mario 64",
            "Legend of Zelda, The - Ocarina of Time",
            "Legend of Zelda, The - Majora's Mask",
            "Super Smash Bros.",
            "GoldenEye 007",
            "Mario Kart 64",
            "Banjo-Kazooie",
            "Paper Mario",
            "Star Fox 64",
            "Perfect Dark",
            "Diddy Kong Racing",
            "F-Zero X",
        ],
    },
    PlatformInfo {
        id: "gb",
        name: "Game Boy",
        manufacturer: "Nintendo",
        libretro_name: "Nintendo_-_Game_Boy",
        extensions: &[".gb", ".zip", ".7z"],
        folder_aliases: &[
            "gb",
            "GB",
            "gameboy",
            "Game Boy",
            "game boy",
            "Nintendo - Game Boy",
        ],
        default_favorites: &[
            "Pokemon - Red Version",
            "Pokemon - Blue Version",
            "Pokemon - Yellow Version",
            "Legend of Zelda, The - Link's Awakening",
            "Super Mario Land",
            "Super Mario Land 2 - 6 Golden Coins",
            "Tetris",
            "Kirby's Dream Land",
            "Metroid II - Return of Samus",
            "Donkey Kong",
            "Wario Land - Super Mario Land 3",
        ],
    },
    PlatformInfo {
        id: "gbc",
        name: "Game Boy Color",
        manufacturer: "Nintendo",
        libretro_name: "Nintendo_-_Game_Boy_Color",
        extensions: &[".gbc", ".zip", ".7z"],
        folder_aliases: &[
            "gbc",
            "GBC",
            "gameboycolor",
            "gameboy color",
            "game boy color",
            "Game Boy Color",
            "Nintendo - Game Boy Color",
            "gb color",
            "gb_color",
        ],
        default_favorites: &[
            "Pokemon - Crystal Version",
            "Pokemon - Gold Version",
            "Pokemon - Silver Version",
            "Legend of Zelda, The - Oracle of Ages",
            "Legend of Zelda, The - Oracle of Seasons",
            "Super Mario Bros. Deluxe",
            "Wario Land 3",
            "Shantae",
            "Metal Gear Solid",
            "Mario Golf",
            "Dragon Warrior III",
        ],
    },
    PlatformInfo {
        id: "gba",
        name: "Game Boy Advance",
        manufacturer: "Nintendo",
        libretro_name: "Nintendo_-_Game_Boy_Advance",
        extensions: &[".gba", ".zip", ".7z"],
        folder_aliases: &[
            "gba",
            "GBA",
            "gameboyadvance",
            "gameboy advance",
            "game boy advance",
            "Game Boy Advance",
            "Nintendo - Game Boy Advance",
            "gb advance",
            "gb_advance",
        ],
        default_favorites: &[
            "Pokemon - Emerald Version",
            "Pokemon - FireRed Version",
            "Pokemon - LeafGreen Version",
            "Legend of Zelda, The - The Minish Cap",
            "Metroid Fusion",
            "Metroid - Zero Mission",
            "Castlevania - Aria of Sorrow",
            "Castlevania - Circle of the Moon",
            "Golden Sun",
            "Golden Sun - The Lost Age",
            "Advance Wars",
            "Advance Wars 2 - Black Hole Rising",
            "Mario Kart - Super Circuit",
            "Fire Emblem",
            "Final Fantasy VI Advance",
        ],
    },
    PlatformInfo {
        id: "nds",
        name: "Nintendo DS",
        manufacturer: "Nintendo",
        libretro_name: "Nintendo_-_Nintendo_DS",
        extensions: &[".nds", ".zip", ".7z"],
        folder_aliases: &[
            "nds",
            "NDS",
            "ds",
            "DS",
            "Nintendo DS",
            "nintendo ds",
            "Nintendo - Nintendo DS",
        ],
        default_favorites: &[
            "Pokemon - HeartGold Version",
            "Pokemon - Platinum Version",
            "Pokemon - Black Version",
            "New Super Mario Bros.",
            "Mario Kart DS",
            "Chrono Trigger",
            "Castlevania - Dawn of Sorrow",
            "Castlevania - Order of Ecclesia",
            "Legend of Zelda, The - Phantom Hourglass",
            "Grand Theft Auto - Chinatown Wars",
            "Radiant Historia",
            "Professor Layton and the Curious Village",
            "Phoenix Wright - Ace Attorney",
            "The World Ends with You",
        ],
    },
    PlatformInfo {
        id: "sms",
        name: "Sega Master System",
        manufacturer: "Sega",
        libretro_name: "Sega_-_Master_System_-_Mark_III",
        extensions: &[".sms", ".zip", ".7z", ".bin"],
        folder_aliases: &[
            "sms",
            "SMS",
            "mastersystem",
            "master system",
            "Master System",
            "sega master system",
            "sega_master_system",
            "Sega Master System",
            "Sega - Master System - Mark III",
        ],
        default_favorites: &[
            "Alex Kidd in Miracle World",
            "Phantasy Star",
            "Sonic The Hedgehog",
            "Wonder Boy III - The Dragon's Trap",
            "Castle of Illusion Starring Mickey Mouse",
            "Shinobi",
            "Out Run",
            "R-Type",
            "Golden Axe Warrior",
            "Golvellius - Valley of Doom",
        ],
    },
    PlatformInfo {
        id: "megadrive",
        name: "Sega Mega Drive / Genesis",
        manufacturer: "Sega",
        libretro_name: "Sega_-_Mega_Drive_-_Genesis",
        extensions: &[".md", ".gen", ".smd", ".bin", ".zip", ".7z"],
        folder_aliases: &[
            "megadrive",
            "genesis",
            "MD",
            "GENESIS",
            "sega mega drive",
            "sega megadrive",
            "sega_mega_drive",
            "sega genesis",
            "Sega Mega Drive",
            "Sega Genesis",
            "Sega - Mega Drive - Genesis",
        ],
        default_favorites: &[
            "Sonic The Hedgehog 2",
            "Sonic 3 & Knuckles",
            "Streets of Rage 2",
            "Gunstar Heroes",
            "Shinobi III - Return of the Ninja Master",
            "Phantasy Star IV",
            "Castlevania - Bloodlines",
            "Contra - Hard Corps",
            "Comix Zone",
            "Aladdin",
            "Earthworm Jim",
            "Road Rash II",
            "Ristar",
        ],
    },
    PlatformInfo {
        id: "gamegear",
        name: "Sega Game Gear",
        manufacturer: "Sega",
        libretro_name: "Sega_-_Game_Gear",
        extensions: &[".gg", ".zip", ".7z", ".bin"],
        folder_aliases: &[
            "gamegear",
            "sega game gear",
            "sega_game_gear",
            "Game Gear",
            "game gear",
            "gg",
            "GG",
            "Sega - Game Gear",
        ],
        default_favorites: &[
            "Sonic The Hedgehog",
            "Sonic Chaos",
            "Shinobi",
            "Defenders of Oasis",
            "Columns",
            "Castle of Illusion",
            "Tails Adventure",
        ],
    },
    PlatformInfo {
        id: "saturn",
        name: "Sega Saturn",
        manufacturer: "Sega",
        libretro_name: "Sega_-_Saturn",
        extensions: &[".chd", ".iso", ".cue", ".bin", ".zip", ".m3u"],
        folder_aliases: &[
            "saturn",
            "ss",
            "SATURN",
            "SS",
            "sega saturn",
            "sega_saturn",
            "Sega Saturn",
            "Sega - Saturn",
        ],
        default_favorites: &[
            "Nights Into Dreams...",
            "Panzer Dragoon Saga",
            "Panzer Dragoon II Zwei",
            "Radiant Silvergun",
            "Sega Rally Championship",
            "Virtua Fighter 2",
            "Guardian Heroes",
            "Saturn Bomberman",
            "Dragon Force",
            "Shining Force III",
        ],
    },
    PlatformInfo {
        id: "dreamcast",
        name: "Sega Dreamcast",
        manufacturer: "Sega",
        libretro_name: "Sega_-_Dreamcast",
        extensions: &[".chd", ".gdi", ".cdi", ".iso", ".zip", ".m3u"],
        folder_aliases: &[
            "dreamcast",
            "dc",
            "DC",
            "DREAMCAST",
            "sega dreamcast",
            "Sega Dreamcast",
            "Sega - Dreamcast",
        ],
        default_favorites: &[
            "Crazy Taxi",
            "Sonic Adventure 2",
            "Shenmue",
            "Jet Set Radio",
            "Soulcalibur",
            "Marvel vs. Capcom 2 - New Age of Heroes",
            "Skies of Arcadia",
            "Ikaruga",
            "Power Stone 2",
            "Resident Evil - Code - Veronica",
            "Virtua Tennis",
        ],
    },
    PlatformInfo {
        id: "psx",
        name: "Sony PlayStation (PS1 / PSX)",
        manufacturer: "Sony",
        libretro_name: "Sony_-_PlayStation",
        extensions: &[".chd", ".cue", ".bin", ".iso", ".pbp", ".zip", ".m3u"],
        folder_aliases: &[
            "psx",
            "ps1",
            "PSX",
            "PS1",
            "ps",
            "PS",
            "PlayStation",
            "playstation",
            "Sony - PlayStation",
            "sony playstation",
        ],
        default_favorites: &[
            "Castlevania - Symphony of the Night",
            "Final Fantasy VII",
            "Final Fantasy IX",
            "Metal Gear Solid",
            "Resident Evil 2",
            "Silent Hill",
            "Crash Bandicoot - Warped",
            "Spyro - Year of the Dragon",
            "Gran Turismo 2",
            "Tekken 3",
            "Chrono Cross",
            "Tony Hawk's Pro Skater 2",
            "Xenogears",
            "Suikoden II",
        ],
    },
    PlatformInfo {
        id: "ps2",
        name: "Sony PlayStation 2 (PS2)",
        manufacturer: "Sony",
        libretro_name: "Sony_-_PlayStation_2",
        extensions: &[".chd", ".iso", ".cso", ".bin", ".gz"],
        folder_aliases: &[
            "ps2",
            "PS2",
            "PlayStation 2",
            "playstation 2",
            "Sony - PlayStation 2",
            "sony playstation 2",
        ],
        default_favorites: &[
            "Grand Theft Auto - San Andreas",
            "God of War II",
            "Metal Gear Solid 3 - Snake Eater",
            "Shadow of the Colossus",
            "Kingdom Hearts II",
            "Final Fantasy X",
            "Silent Hill 2",
            "Burnout 3 - Takedown",
            "Resident Evil 4",
            "Devil May Cry 3 - Dante's Awakening",
            "Okami",
            "Persona 4",
            "Ratchet & Clank - Up Your Arsenal",
            "Jak and Daxter - The Precursor Legacy",
        ],
    },
    PlatformInfo {
        id: "psp",
        name: "Sony PlayStation Portable (PSP)",
        manufacturer: "Sony",
        libretro_name: "Sony_-_PlayStation_Portable",
        extensions: &[".cso", ".iso", ".pbp", ".chd"],
        folder_aliases: &[
            "psp",
            "PSP",
            "PlayStation Portable",
            "playstation portable",
            "Sony - PlayStation Portable",
            "sony playstation portable",
        ],
        default_favorites: &[
            "God of War - Ghost of Sparta",
            "God of War - Chains of Olympus",
            "Crisis Core - Final Fantasy VII",
            "Metal Gear Solid - Peace Walker",
            "Grand Theft Auto - Liberty City Stories",
            "Persona 3 Portable",
            "Monster Hunter Freedom Unite",
            "Kingdom Hearts - Birth by Sleep",
            "Castlevania - The Dracula X Chronicles",
            "Daxter",
            "Lumines",
            "Tekken - Dark Resurrection",
        ],
    },
    PlatformInfo {
        id: "gamecube",
        name: "Nintendo GameCube",
        manufacturer: "Nintendo",
        libretro_name: "Nintendo_-_GameCube",
        extensions: &[".iso", ".cso", ".gcm", ".rvz", ".zip"],
        folder_aliases: &[
            "gamecube",
            "gc",
            "GC",
            "Nintendo GameCube",
            "nintendo gamecube",
            "Nintendo - GameCube",
        ],
        default_favorites: &[
            "Super Smash Bros. Melee",
            "Legend of Zelda, The - The Wind Waker",
            "Super Mario Sunshine",
            "Metroid Prime",
            "Mario Kart - Double Dash!!",
            "Resident Evil 4",
            "Luigi's Mansion",
        ],
    },
    PlatformInfo {
        id: "wii",
        name: "Nintendo Wii",
        manufacturer: "Nintendo",
        libretro_name: "Nintendo_-_Wii",
        extensions: &[".wbfs", ".iso", ".rvz", ".cso"],
        folder_aliases: &[
            "wii",
            "Wii",
            "Nintendo Wii",
            "nintendo wii",
            "Nintendo - Wii",
        ],
        default_favorites: &[
            "Super Mario Galaxy",
            "Super Mario Galaxy 2",
            "Legend of Zelda, The - Twilight Princess",
            "Xenoblade Chronicles",
            "Super Smash Bros. Brawl",
            "Mario Kart Wii",
            "Donkey Kong Country Returns",
        ],
    },
];

pub fn find_platform_by_id(id: &str) -> Option<&'static PlatformInfo> {
    PLATFORMS.iter().find(|p| p.id.eq_ignore_ascii_case(id))
}

pub fn find_platform_by_dir_name(dir_name: &str) -> Option<&'static PlatformInfo> {
    let clean = dir_name.trim();
    let normalized = clean.to_lowercase().replace(['_', '-'], " ");
    let compact = normalized.replace(' ', "");

    for platform in PLATFORMS {
        for alias in platform.folder_aliases {
            if alias.eq_ignore_ascii_case(clean) {
                return Some(platform);
            }
            let alias_norm = alias.to_lowercase().replace(['_', '-'], " ");
            if alias_norm == normalized {
                return Some(platform);
            }
            let alias_compact = alias_norm.replace(' ', "");
            if !alias_compact.is_empty() && alias_compact == compact {
                return Some(platform);
            }
        }
    }
    None
}

/// Checks if a platform commonly uses CD-ROM disc images with multiple files and audio tracks
pub fn is_cd_platform(platform_id: &str) -> bool {
    matches!(
        platform_id,
        "psx" | "saturn" | "dreamcast" | "segacd" | "pcecd" | "3do" | "neogeocd"
    )
}

/// Determines whether a filename corresponds to a secondary data track or CD audio track
pub fn is_track_or_companion_file(filename: &str) -> bool {
    let lower = filename.to_lowercase();

    // Dedicated CD audio and subchannel track extensions
    if lower.ends_with(".wav")
        || lower.ends_with(".mp3")
        || lower.ends_with(".ogg")
        || lower.ends_with(".flac")
        || lower.ends_with(".ape")
        || lower.ends_with(".sub")
        || lower.ends_with(".raw")
    {
        return true;
    }

    // Typical CD track naming patterns: "(Track 1).bin", "Track 02.bin", "track01.bin", etc.
    if lower.ends_with(".bin") || lower.ends_with(".img") || lower.ends_with(".iso") {
        if lower.contains("track ")
            || lower.contains("track_")
            || lower.contains("(track")
            || lower.contains("[track")
            || lower.contains("track0")
            || lower.contains("track1")
            || lower.contains("track2")
            || lower.contains("track3")
            || lower.contains("track4")
            || lower.contains("track5")
            || lower.contains("track6")
            || lower.contains("track7")
            || lower.contains("track8")
            || lower.contains("track9")
        {
            return true;
        }
    }

    false
}

/// Filters a list of ROM filenames to return only primary game launch entries,
/// suppressing individual audio tracks and secondary binary data tracks for CD platforms.
pub fn filter_primary_rom_files(platform_id: &str, files: &[String]) -> Vec<String> {
    if !is_cd_platform(platform_id) {
        return files.to_vec();
    }

    // Collect base stems of all descriptor files (.cue, .m3u, .gdi, .ccd, .chd, .pbp)
    let mut descriptor_stems = std::collections::HashSet::new();
    for f in files {
        let lower = f.to_lowercase();
        if lower.ends_with(".cue")
            || lower.ends_with(".m3u")
            || lower.ends_with(".gdi")
            || lower.ends_with(".ccd")
            || lower.ends_with(".chd")
            || lower.ends_with(".pbp")
        {
            if let Some(stem) = Path::new(f).file_stem().and_then(|s| s.to_str()) {
                descriptor_stems.insert(stem.to_lowercase());
            }
        }
    }

    let mut result = Vec::new();
    for f in files {
        let lower = f.to_lowercase();

        // Secondary audio tracks (.wav, .flac, .mp3, .ape) are never primary launch entries
        if is_track_or_companion_file(f) {
            continue;
        }

        // If a matching descriptor file (.cue, .m3u) exists for this game, hide the .bin/.img
        if lower.ends_with(".bin") || lower.ends_with(".img") {
            if let Some(stem) = Path::new(f).file_stem().and_then(|s| s.to_str()) {
                let stem_lower = stem.to_lowercase();
                if descriptor_stems.contains(&stem_lower)
                    || descriptor_stems.iter().any(|d| stem_lower.starts_with(d))
                {
                    continue;
                }
            }
        }

        result.push(f.clone());
    }

    result
}

/// Resolves all companion files (data tracks, CD audio tracks .wav/.mp3/.flac/.bin, subchannel files)
/// that belong to a primary ROM (such as a .cue, .m3u, or .gdi).
pub fn get_companion_files(primary_path: &Path) -> Vec<PathBuf> {
    let mut companions = Vec::new();
    let parent = match primary_path.parent() {
        Some(p) => p,
        None => return companions,
    };

    let ext = primary_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    // 1. If .cue sheet, parse FILE declarations
    if ext == "cue" {
        if let Ok(content) = std::fs::read_to_string(primary_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.to_uppercase().starts_with("FILE ") {
                    // Quoted filename: FILE "Crash Bandicoot (Track 1).bin" BINARY
                    if let Some(first_quote) = trimmed.find('"') {
                        if let Some(second_quote) = trimmed[first_quote + 1..].find('"') {
                            let track_file = &trimmed[first_quote + 1..first_quote + 1 + second_quote];
                            let track_path = parent.join(track_file);
                            if track_path.exists() && track_path != primary_path {
                                companions.push(track_path);
                            }
                        }
                    } else {
                        // Unquoted filename: FILE track1.bin BINARY
                        let parts: Vec<&str> = trimmed.split_whitespace().collect();
                        if parts.len() >= 2 {
                            let track_path = parent.join(parts[1]);
                            if track_path.exists() && track_path != primary_path {
                                companions.push(track_path);
                            }
                        }
                    }
                }
            }
        }
    } else if ext == "m3u" {
        // 2. If .m3u playlist, read referenced disc files and their companion tracks
        if let Ok(content) = std::fs::read_to_string(primary_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                let disc_path = parent.join(trimmed);
                if disc_path.exists() {
                    if disc_path != primary_path {
                        companions.push(disc_path.clone());
                    }
                    for sub_comp in get_companion_files(&disc_path) {
                        companions.push(sub_comp);
                    }
                }
            }
        }
    }

    // 3. Sibling scan fallback: find any track or audio file matching the primary stem prefix
    if let Some(stem) = primary_path.file_stem().and_then(|s| s.to_str()) {
        let stem_lower = stem.to_lowercase();
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.filter_map(|e| e.ok()) {
                let p = entry.path();
                if p == primary_path {
                    continue;
                }
                if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                    let name_lower = name.to_lowercase();
                    if (name_lower.starts_with(&stem_lower) || stem_lower.starts_with(&name_lower))
                        && is_track_or_companion_file(name)
                    {
                        if !companions.contains(&p) {
                            companions.push(p);
                        }
                    }
                }
            }
        }
    }

    companions.sort();
    companions.dedup();
    companions
}
