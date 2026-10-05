use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::art_scraper::ArtType;
use crate::platforms::PlatformInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileId {
    EmulationStationEsDe,
    RetroArch,
    AnbernicRgDsLauncher,
    AnbernicOlderGarlicOs,
    MiyooMiniOnionOs,
    Daijisho,
    Pegasus,
    BatoceraArkOs,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherProfile {
    pub id: ProfileId,
    pub name: &'static str,
    pub description: &'static str,
    pub recommended_sd_subfolder: &'static str,
    pub guidance: &'static str,
}

pub static PROFILES: &[LauncherProfile] = &[
    LauncherProfile {
        id: ProfileId::EmulationStationEsDe,
        name: "EmulationStation (ES-DE)",
        description: "Standard layout for ES-DE on Android, Windows, Mac, Linux and Steam Deck.",
        recommended_sd_subfolder: "roms",
        guidance: "Creates 'roms/<system>/' directories. Artwork is placed in 'downloaded_media/<system>/covers/<rom>.png' or 'media/covers/', matching ES-DE standard scraping format.",
    },
    LauncherProfile {
        id: ProfileId::RetroArch,
        name: "RetroArch",
        description: "Official Libretro thumbnail and playlist directory hierarchy.",
        recommended_sd_subfolder: "roms",
        guidance: "Places ROMs in 'roms/<system>/' and art in 'thumbnails/<System_Name>/Named_Boxarts/<rom>.png' for seamless native RetroArch playlist scanning.",
    },
    LauncherProfile {
        id: ProfileId::AnbernicRgDsLauncher,
        name: "Anbernic RG DS Launcher / Modern Stock OS",
        description: "For Anbernic dual-screen handhelds (RG Cube, RG series DS launcher, Linux/Android stock).",
        recommended_sd_subfolder: "roms",
        guidance: "Uses Anbernic standard folder codes ('FC', 'SFC', 'GBA', 'NDS', 'MD', 'PS', etc.). Boxart is saved in 'Imgs/<rom>.png' inside each platform directory for immediate launcher display.",
    },
    LauncherProfile {
        id: ProfileId::AnbernicOlderGarlicOs,
        name: "Anbernic Older Devices / GarlicOS",
        description: "For RG350, RG351, RG35XX (GarlicOS 1.x / Stock OS / MinUI).",
        recommended_sd_subfolder: "Roms",
        guidance: "Creates 'Roms/<CODE>' folders with artwork in 'Roms/<CODE>/Imgs/<rom>.png'. Formats as FAT32 or exFAT depending on OS.",
    },
    LauncherProfile {
        id: ProfileId::MiyooMiniOnionOs,
        name: "Miyoo Mini / OnionOS",
        description: "OnionOS standard folder layout for Miyoo Mini and Mini Plus.",
        recommended_sd_subfolder: "Roms",
        guidance: "Creates 'Roms/<CODE>' folders (e.g. 'FC', 'SFC', 'GBA', 'PS') with artwork placed in 'Roms/<CODE>/Imgs/<rom>.png'.",
    },
    LauncherProfile {
        id: ProfileId::Daijisho,
        name: "Daijishō Launcher (Android)",
        description: "Popular Android frontend for Retroid Pocket, Odin, and Android devices.",
        recommended_sd_subfolder: "roms",
        guidance: "Organizes ROMs in 'roms/<system>/' and stores downloaded cover arts in 'roms/<system>/thumbnails/<rom>.png'.",
    },
    LauncherProfile {
        id: ProfileId::Pegasus,
        name: "Pegasus Frontend",
        description: "Cross-platform customizable frontend using standard metadata & media paths.",
        recommended_sd_subfolder: "roms",
        guidance: "Organizes ROMs in 'roms/<system>/' and stores cover art in 'roms/<system>/media/<rom>.png'. Generates compatible structure for Pegasus scraping.",
    },
    LauncherProfile {
        id: ProfileId::BatoceraArkOs,
        name: "Batocera / ArkOS / AmberELEC",
        description: "Standard Linux retro distribution layout for RK3326, RK3566, and PC handhelds.",
        recommended_sd_subfolder: "roms",
        guidance: "Organizes ROMs in 'roms/<system>/' with artwork saved in 'roms/<system>/images/<rom>-image.png'.",
    },
    LauncherProfile {
        id: ProfileId::Custom,
        name: "Generic / Custom Layout",
        description: "Clean simple platform directories with covers beside ROM files.",
        recommended_sd_subfolder: "roms",
        guidance: "Puts ROMs in '<folder>/<system>/' and artwork in '<folder>/<system>/<rom>.png'.",
    },
];

impl LauncherProfile {
    pub fn get_by_id(id: ProfileId) -> &'static LauncherProfile {
        PROFILES.iter().find(|p| p.id == id).unwrap_or(&PROFILES[0])
    }

    /// Returns the target ROM directory relative to the chosen base folder (e.g. "E:/" or "E:/roms")
    pub fn get_platform_folder(&self, platform: &PlatformInfo) -> String {
        match self.id {
            ProfileId::AnbernicRgDsLauncher => {
                match platform.id {
                    "nes" => "FC".to_string(),
                    "snes" => "SFC".to_string(),
                    "n64" => "N64".to_string(),
                    "gb" => "GB".to_string(),
                    "gbc" => "GBC".to_string(),
                    "gba" => "GBA".to_string(),
                    "nds" => "NDS".to_string(),
                    "sms" => "SMS".to_string(),
                    "megadrive" => "MD".to_string(),
                    "gamegear" => "GG".to_string(),
                    "saturn" => "SATURN".to_string(),
                    "dreamcast" => "DREAMCAST".to_string(),
                    "psx" => "PS".to_string(),
                    "ps2" => "PS2".to_string(),
                    "psp" => "PSP".to_string(),
                    "pce" => "PCE".to_string(),
                    "neogeo" => "NEOGEO".to_string(),
                    other => other.to_uppercase(),
                }
            }
            ProfileId::AnbernicOlderGarlicOs
            | ProfileId::MiyooMiniOnionOs => {
                // Short uppercase codes used by older GarlicOS / Miyoo
                match platform.id {
                    "nes" => "FC".to_string(),
                    "snes" => "SFC".to_string(),
                    "n64" => "N64".to_string(),
                    "gb" => "GB".to_string(),
                    "gbc" => "GBC".to_string(),
                    "gba" => "GBA".to_string(),
                    "nds" => "NDS".to_string(),
                    "sms" => "SMS".to_string(),
                    "megadrive" => "MD".to_string(),
                    "gamegear" => "GG".to_string(),
                    "saturn" => "SS".to_string(),
                    "dreamcast" => "DC".to_string(),
                    "psx" => "PS".to_string(),
                    "ps2" => "PS2".to_string(),
                    "psp" => "PSP".to_string(),
                    other => other.to_uppercase(),
                }
            }
            ProfileId::RetroArch => {
                // RetroArch can use clean ID or full Libretro name
                platform.id.to_string()
            }
            _ => platform.id.to_string(),
        }
    }

    /// Calculates the full destination path for a ROM file
    pub fn get_rom_destination(
        &self,
        base_dest: &Path,
        platform: &PlatformInfo,
        rom_filename: &str,
    ) -> PathBuf {
        let folder = self.get_platform_folder(platform);
        base_dest.join(folder).join(rom_filename)
    }

    /// Calculates the full destination path for the corresponding artwork image with specific ArtType
    pub fn get_typed_art_destination(
        &self,
        base_dest: &Path,
        platform: &PlatformInfo,
        rom_stem: &str,
        art_type: ArtType,
    ) -> PathBuf {
        let p_folder = self.get_platform_folder(platform);
        let filename = format!("{}.png", rom_stem);

        match self.id {
            ProfileId::EmulationStationEsDe => {
                let sub = match art_type {
                    ArtType::Boxart => "covers",
                    ArtType::Screenshot => "screenshots",
                    ArtType::TitleScreen => "titles",
                };
                base_dest
                    .join("downloaded_media")
                    .join(platform.id)
                    .join(sub)
                    .join(&filename)
            }
            ProfileId::RetroArch => {
                base_dest
                    .join("thumbnails")
                    .join(platform.libretro_name)
                    .join(art_type.libretro_folder())
                    .join(&filename)
            }
            ProfileId::AnbernicRgDsLauncher
            | ProfileId::AnbernicOlderGarlicOs
            | ProfileId::MiyooMiniOnionOs => {
                let sub = match art_type {
                    ArtType::Boxart => "Imgs",
                    ArtType::Screenshot => "snaps",
                    ArtType::TitleScreen => "titles",
                };
                base_dest
                    .join(&p_folder)
                    .join(sub)
                    .join(&filename)
            }
            ProfileId::Daijisho => {
                let sub = match art_type {
                    ArtType::Boxart => "thumbnails",
                    ArtType::Screenshot => "screenshots",
                    ArtType::TitleScreen => "titles",
                };
                base_dest
                    .join(&p_folder)
                    .join(sub)
                    .join(&filename)
            }
            ProfileId::Pegasus => {
                let sub = match art_type {
                    ArtType::Boxart => "media",
                    ArtType::Screenshot => "screenshots",
                    ArtType::TitleScreen => "titles",
                };
                base_dest
                    .join(&p_folder)
                    .join(sub)
                    .join(&filename)
            }
            ProfileId::BatoceraArkOs => {
                let suffix = match art_type {
                    ArtType::Boxart => "-image.png",
                    ArtType::Screenshot => "-snap.png",
                    ArtType::TitleScreen => "-title.png",
                };
                base_dest
                    .join(&p_folder)
                    .join("images")
                    .join(format!("{}{}", rom_stem, suffix))
            }
            ProfileId::Custom => {
                base_dest
                    .join(&p_folder)
                    .join(art_type.default_subfolder())
                    .join(&filename)
            }
        }
    }

    /// Calculates the full destination path for the corresponding artwork image (defaults to Boxart)
    pub fn get_art_destination(
        &self,
        base_dest: &Path,
        platform: &PlatformInfo,
        rom_stem: &str,
    ) -> PathBuf {
        self.get_typed_art_destination(base_dest, platform, rom_stem, ArtType::Boxart)
    }
}
