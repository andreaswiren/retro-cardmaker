use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::platforms::PlatformInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoritesList {
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub games: Vec<String>,
    #[serde(default)]
    pub patterns: Vec<String>,
}

impl FavoritesList {
    #[allow(dead_code)]
    pub fn new(platform: &str, title: &str) -> Self {
        Self {
            platform: platform.to_string(),
            title: title.to_string(),
            games: Vec::new(),
            patterns: Vec::new(),
        }
    }

    pub fn from_default_platform(platform: &PlatformInfo) -> Self {
        Self {
            platform: platform.id.to_string(),
            title: format!("{} Essential Classics", platform.name),
            games: platform
                .default_favorites
                .iter()
                .map(|&s| s.to_string())
                .collect(),
            patterns: platform
                .default_favorites
                .iter()
                .map(|&s| s.to_string())
                .collect(),
        }
    }

    /// Checks if a ROM filename matches any favorite entry
    pub fn is_match(&self, rom_filename: &str) -> bool {
        let clean_filename = rom_filename.to_lowercase();
        let stem = Path::new(rom_filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(rom_filename)
            .to_lowercase();

        // 1. Direct match on games list
        for game in &self.games {
            let g = game.to_lowercase();
            if clean_filename == g || stem == g {
                return true;
            }
            // Substring match on game name
            if clean_filename.contains(&g) || stem.contains(&g) || g.contains(&stem) {
                return true;
            }
        }

        // 2. Pattern match
        for pattern in &self.patterns {
            let p = pattern.to_lowercase();
            if clean_filename.contains(&p) || stem.contains(&p) {
                return true;
            }
        }

        false
    }
}

/// Attempts to load `favorites.json` from a platform directory
pub fn load_favorites(folder: &Path) -> Option<FavoritesList> {
    let fav_path = folder.join("favorites.json");
    if !fav_path.exists() {
        return None;
    }

    let mut file = File::open(&fav_path).ok()?;
    let mut content = String::new();
    file.read_to_string(&mut content).ok()?;

    // Try parsing as structured FavoritesList
    if let Ok(list) = serde_json::from_str::<FavoritesList>(&content) {
        return Some(list);
    }

    // Fallback: Try parsing as simple JSON array of strings: ["Game 1", "Game 2"]
    if let Ok(simple_array) = serde_json::from_str::<Vec<String>>(&content) {
        return Some(FavoritesList {
            platform: "".to_string(),
            title: "Favorites List".to_string(),
            games: simple_array.clone(),
            patterns: simple_array,
        });
    }

    None
}

/// Saves `favorites.json` to a platform directory
pub fn save_favorites(folder: &Path, favorites: &FavoritesList) -> Result<(), String> {
    let fav_path = folder.join("favorites.json");
    let content = serde_json::to_string_pretty(favorites)
        .map_err(|e| format!("Failed to serialize favorites: {}", e))?;

    let mut file = File::create(&fav_path)
        .map_err(|e| format!("Failed to create favorites.json: {}", e))?;

    file.write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write favorites.json: {}", e))?;

    Ok(())
}
