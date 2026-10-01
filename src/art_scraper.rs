use std::fs::File;
use std::io::Write;
use std::path::Path;
use regex::Regex;
use reqwest::blocking::Client;
use crate::platforms::PlatformInfo;

pub struct ArtScraper {
    client: Client,
}

impl ArtScraper {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent("RetroCardMaker/1.0 (Windows; x86_64)")
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { client }
    }

    /// Normalizes ROM file stem according to Libretro thumbnail naming guidelines
    pub fn normalize_for_libretro(rom_filename: &str) -> String {
        let clean_str = rom_filename.trim_end_matches(['/', '\\']);
        let stem = if let Some(dot_idx) = clean_str.rfind('.') {
            if clean_str.len() - dot_idx <= 6 {
                &clean_str[..dot_idx]
            } else {
                clean_str
            }
        } else {
            clean_str
        };

        // Replace illegal filesystem / URL characters as libretro specification mandates:
        // & * / : ` < > ? \ | "
        let mut clean = String::with_capacity(stem.len());
        for ch in stem.chars() {
            match ch {
                '&' | '*' | '/' | ':' | '`' | '<' | '>' | '?' | '\\' | '|' | '"' => {
                    clean.push('_');
                }
                _ => clean.push(ch),
            }
        }

        // Clean up multi-spaces or trim
        clean.trim().to_string()
    }

    /// Generates fallback search names (e.g., removing "(USA)", "[!]", etc.)
    pub fn generate_fallback_names(normalized_stem: &str) -> Vec<String> {
        let mut candidates = Vec::new();

        // 1. Regex to remove [tags] and (tags)
        let re_tags = Regex::new(r"\s*(\([^\)]*\)|\[[^\]]*\])").unwrap();
        let stripped = re_tags.replace_all(normalized_stem, "").trim().to_string();

        if !stripped.is_empty() && stripped != normalized_stem {
            // Append stripped name
            candidates.push(stripped.clone());
            // Add USA variation if not present
            candidates.push(format!("{} (USA)", stripped));
            candidates.push(format!("{} (Europe)", stripped));
            candidates.push(format!("{} (Japan)", stripped));
        }

        // 2. Also try replacing underscores with spaces if user had underscores
        if normalized_stem.contains('_') {
            candidates.push(normalized_stem.replace('_', " "));
        }

        candidates
    }

    /// Attempts to download boxart for a ROM and save it to `target_path`.
    /// Returns Ok(true) if newly downloaded, Ok(false) if already present, or Err on failure.
    pub fn download_boxart(
        &self,
        platform: &PlatformInfo,
        rom_filename: &str,
        target_path: &Path,
    ) -> Result<bool, String> {
        if target_path.exists() {
            return Ok(false); // Already exists
        }

        if let Some(parent) = target_path.parent() {
            if !parent.exists() {
                let _ = std::fs::create_dir_all(parent);
            }
        }

        let base_name = Self::normalize_for_libretro(rom_filename);
        let mut names_to_try = vec![base_name.clone()];
        names_to_try.extend(Self::generate_fallback_names(&base_name));

        // De-duplicate candidates
        names_to_try.dedup();

        for name in &names_to_try {
            let encoded_name = urlencoding_encode(name);
            let url = format!(
                "https://raw.githubusercontent.com/libretro-thumbnails/{}/master/Named_Boxarts/{}.png",
                platform.libretro_name,
                encoded_name
            );

            if let Ok(resp) = self.client.get(&url).send() {
                if resp.status().is_success() {
                    if let Ok(bytes) = resp.bytes() {
                        if !bytes.is_empty() {
                            if let Ok(mut file) = File::create(target_path) {
                                if file.write_all(&bytes).is_ok() {
                                    return Ok(true);
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(format!("Boxart not found on Libretro CDN for {}", rom_filename))
    }
}

/// Simple percent-encoding for URL query/path component
fn urlencoding_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.as_bytes() {
        match *b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            b' ' => out.push_str("%20"),
            b'(' => out.push_str("%28"),
            b')' => out.push_str("%29"),
            b',' => out.push_str("%2C"),
            b'\'' => out.push_str("%27"),
            b'!' => out.push_str("%21"),
            b => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}
