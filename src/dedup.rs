use regex::Regex;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RegionPreference {
    UsaFirst,
    EuropeFirst,
    JapanFirst,
    WorldFirst,
    None,
}

impl Default for RegionPreference {
    fn default() -> Self {
        Self::UsaFirst
    }
}

impl RegionPreference {
    pub fn label(&self) -> &'static str {
        match self {
            Self::UsaFirst => "USA / NTSC-U",
            Self::EuropeFirst => "Europe / PAL",
            Self::JapanFirst => "Japan / NTSC-J",
            Self::WorldFirst => "World / Global",
            Self::None => "Keep All (No Filter)",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::UsaFirst => "Prefers USA versions, then World, Europe, and Japan.",
            Self::EuropeFirst => "Prefers European/PAL versions, then World, USA, and Japan.",
            Self::JapanFirst => "Prefers Japanese versions, then World, USA, and Europe.",
            Self::WorldFirst => "Prefers World releases, then USA, Europe, and Japan.",
            Self::None => "No duplicate filtering: all regional variants and revisions are kept.",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeduplicationResult {
    pub kept_files: Vec<String>,
    pub total_input: usize,
    pub duplicates_filtered: usize,
    pub betas_filtered: usize,
}

/// Extracts a canonical game key from a filename by stripping tags
pub fn extract_canonical_title(filename: &str) -> String {
    let clean = filename.trim_end_matches(['/', '\\']);
    let stem = if let Some(dot_idx) = clean.rfind('.') {
        if clean.len() - dot_idx <= 6 {
            &clean[..dot_idx]
        } else {
            clean
        }
    } else {
        clean
    };

    // Remove tags in parentheses (e.g. "(USA)", "(Rev 1)", "(En,Fr)") and brackets (e.g. "[!]")
    let re_tags = Regex::new(r"\s*(\([^\)]*\)|\[[^\]]*\])").unwrap();
    let stripped = re_tags.replace_all(stem, "").trim().to_string();

    if stripped.is_empty() {
        stem.to_lowercase()
    } else {
        stripped.to_lowercase()
    }
}

/// Checks if a file is a beta, prototype, sample, demo, or unreleased dump
pub fn is_beta_or_proto(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    lower.contains("(beta")
        || lower.contains("(proto")
        || lower.contains("(sample")
        || lower.contains("(demo")
        || lower.contains("(kiosk")
        || lower.contains("(unl")
        || lower.contains("(test program")
        || lower.contains("[b]")
}

/// Scores a file according to region preference and quality indicators
fn score_rom_candidate(filename: &str, preference: RegionPreference) -> i32 {
    let lower = filename.to_lowercase();
    let mut score = 0;

    // Bad dumps heavily penalized
    if lower.contains("[b") {
        score -= 200;
    }
    // Hacks and trainers penalized
    if lower.contains("[h") || lower.contains("[t") {
        score -= 50;
    }
    // Verified good dump bonus
    if lower.contains("[!]") {
        score += 15;
    }

    // Revision bonuses (prefer latest official revisions)
    if lower.contains("(rev 3") || lower.contains("(v1.3") {
        score += 8;
    } else if lower.contains("(rev 2") || lower.contains("(v1.2") {
        score += 6;
    } else if lower.contains("(rev 1") || lower.contains("(v1.1") || lower.contains("(rev a") {
        score += 4;
    }

    // Region scores based on user preference
    match preference {
        RegionPreference::UsaFirst => {
            if lower.contains("(usa") || lower.contains("(u)") || lower.contains("(north america") {
                score += 100;
            } else if lower.contains("(world") || lower.contains("(w)") {
                score += 80;
            } else if lower.contains("(europe") || lower.contains("(eu)") || lower.contains("(e)") || lower.contains("(en") {
                score += 60;
            } else if lower.contains("(japan") || lower.contains("(jp)") || lower.contains("(j)") {
                score += 40;
            } else {
                score += 20;
            }
        }
        RegionPreference::EuropeFirst => {
            if lower.contains("(europe") || lower.contains("(eu)") || lower.contains("(e)") || lower.contains("(pal") {
                score += 100;
            } else if lower.contains("(world") || lower.contains("(w)") {
                score += 80;
            } else if lower.contains("(usa") || lower.contains("(u)") || lower.contains("(en") {
                score += 60;
            } else if lower.contains("(japan") || lower.contains("(jp)") || lower.contains("(j)") {
                score += 40;
            } else {
                score += 20;
            }
        }
        RegionPreference::JapanFirst => {
            if lower.contains("(japan") || lower.contains("(jp)") || lower.contains("(j)") {
                score += 100;
            } else if lower.contains("(world") || lower.contains("(w)") {
                score += 80;
            } else if lower.contains("(usa") || lower.contains("(u)") {
                score += 60;
            } else if lower.contains("(europe") || lower.contains("(eu)") || lower.contains("(e)") {
                score += 40;
            } else {
                score += 20;
            }
        }
        RegionPreference::WorldFirst => {
            if lower.contains("(world") || lower.contains("(w)") {
                score += 100;
            } else if lower.contains("(usa") || lower.contains("(u)") {
                score += 80;
            } else if lower.contains("(europe") || lower.contains("(eu)") || lower.contains("(e)") {
                score += 60;
            } else if lower.contains("(japan") || lower.contains("(jp)") || lower.contains("(j)") {
                score += 40;
            } else {
                score += 20;
            }
        }
        RegionPreference::None => {
            // No region preference
            score += 50;
        }
    }

    score
}

/// Filters a list of ROM filenames to remove regional clones (1G1R) and optional betas
pub fn filter_roms_1g1r(
    filenames: &[String],
    preference: RegionPreference,
    exclude_betas: bool,
) -> DeduplicationResult {
    let total_input = filenames.len();

    if preference == RegionPreference::None && !exclude_betas {
        return DeduplicationResult {
            kept_files: filenames.to_vec(),
            total_input,
            duplicates_filtered: 0,
            betas_filtered: 0,
        };
    }

    let mut betas_filtered = 0;
    let mut candidates_to_process = Vec::new();

    for f in filenames {
        if exclude_betas && is_beta_or_proto(f) {
            betas_filtered += 1;
            continue;
        }
        candidates_to_process.push(f.clone());
    }

    if preference == RegionPreference::None {
        return DeduplicationResult {
            kept_files: candidates_to_process,
            total_input,
            duplicates_filtered: 0,
            betas_filtered,
        };
    }

    // Group candidates by canonical game title
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for f in candidates_to_process {
        let key = extract_canonical_title(&f);
        groups.entry(key).or_default().push(f);
    }

    let mut kept_files = Vec::new();
    let mut duplicates_filtered = 0;

    for (_key, mut variants) in groups {
        if variants.len() == 1 {
            kept_files.push(variants.pop().unwrap());
        } else {
            // Sort variants descending by score
            variants.sort_by(|a, b| {
                let score_b = score_rom_candidate(b, preference);
                let score_a = score_rom_candidate(a, preference);
                score_b.cmp(&score_a)
            });

            duplicates_filtered += variants.len() - 1;
            let best_match = variants.remove(0);
            kept_files.push(best_match);
        }
    }

    // Keep natural alphabetical order
    kept_files.sort();

    DeduplicationResult {
        kept_files,
        total_input,
        duplicates_filtered,
        betas_filtered,
    }
}
