# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.3] - 2026-10-02

### Added
- **Headless Visual Verification Pipeline**:
  - Added `--screenshot <path>`, `--screenshot-tab <tab>`, and `--screenshot-modal` CLI flags to capture exact pixel-perfect UI states headlessly for automated verification.
- **Interactive Styled Button Widgets**:
  - Replaced unstyled label buttons with authentic interactive `egui::Button` widgets featuring elevated backgrounds, borders, hover transitions, and rounded corners across the entire application.

### Fixed
- **Curated List Horizontal Blowout**:
  - Fixed infinite width layout blowout caused by long keyword lists and large ROM selections.
  - Converted keyword pattern chips into auto-wrapping pill buttons with multi-row layout.
- **Live Preview Metadata Text Wrapping**:
  - Fixed character-by-character vertical text collapse in the boxart live preview metadata panel.
- **Unicode Font Rendering**:
  - Replaced unsupported glyphs (`\u{2715}`, `\u{2355}`) with standard Latin-1 symbols (`×`, clean labels) to eliminate missing square glyph boxes (`\u{25A1}`).
- **Modal Dialog Sizing & Footer Visibility**:
  - Refined modal dialog dimensions (`940x640px`) and positioning (`130, 40`) to ensure 100% visibility of the lime green `💾 Save favorites.json` button, `Sync Boxart` neon toggle, and `Close` button.
- **Windows Subsystem Fix (No Black Console Window)**:
  - Added `#![cfg_attr(windows, windows_subsystem = "windows")]` to compile as a native Windows GUI application (`IMAGE_SUBSYSTEM_WINDOWS_GUI`), completely eliminating the unwanted black command prompt window when launching `retro-cardmaker.exe`.
  - Added dynamic console attachment via `AttachConsole` so CLI commands (`--help`, `--cli`) still print output to the terminal when explicitly invoked from the console.

## [0.2.2] - 2026-10-02

### Added
- **Full AAA UX Desktop Redesign (Unified Studio Cockpit)**:
  - Replaced isolated wizard steps with the approved unified 3-tier Master Studio Cockpit:
    - **Tier 1 (Storage & Profiles)**: `1. SD Card & Format` with dropdown, filesystem indicator, `[🛡️ SAFETY LOCK]` badge, format toggles, Diskpart MBR clean wipe, and `2. Device Profile` with horizontal launcher cards.
    - **Tier 2 (Consoles & ROMs)**: `3. Consoles Selection` with vertical list, ROM count badges, active row highlight, and glowing `[ ⭐ CURATED FAVORITES ]` buttons, paired with `4. Live ROMs & Boxart Preview` with search, thumbnails, file sizes, and `Top Classic` badges.
    - **Tier 3 (Cockpit Progress & Logs)**: `5. QuickInstaller Cockpit` with glowing neon cyan progress bars (`PLATFORM SYNC` and `FILE COPY`) and giant launch CTA, paired with `Terminal Console (terminal.log)` with retro dots and syntax-highlighted live execution stream.
  - Windows 11 style header bar with navigation pills: `[ ⊞ Dashboard ]`, `[ 🖵 Consoles ]`, `[ ⭐ Favorites ]`, `[ ⚙ Settings ]` and target storage status pill.
  - 5-step Chevron Ribbon (`❶ SD CARD & FORMAT` ➔ `❷ DEVICE PROFILE` ➔ `❸ ROMS & FAVORITES` ➔ `❹ BOXART SCRAPING` ➔ `❺ QUICKINSTALL`).
- **Curated Favorites Modal Dialog**:
  - Direct 1-click launch from any console row via `[ ⭐ CURATED FAVORITES ]`.
  - 2-column modal layout with live artwork preview, metadata inspection, keyword rules, and lime green `[ Save favorites.json ]` button.
- **Full-Screen Curated Favorites View**:
  - Complete favorites manager accessible from the top navigation bar with platform switcher chips and real-time syntax-formatted `favorites.json Code Preview`.

## [0.2.1] - 2026-10-02

### Added
- **Removable-Only Drive Formatting Guard**:
  - Formatting operations are strictly locked to removable USB flash drives and SD-cards (`is_removable == true`).
  - Fixed internal drives (NVMe SSDs, internal hard disks) and system drives (`C:`) are locked against accidental formatting in both GUI and CLI.
- **Full Repartition & Clean Wipe (Diskpart MBR Engine)**:
  - Added full disk wiping via automated `diskpart clean` to eradicate foreign Linux ext4/swap/OEM hidden partitions left by handheld operating systems (GarlicOS, OnionOS, ArkOS, Batocera).
  - Converts disks to standard MBR and creates a single active full-capacity primary partition, recovering 100% of physical storage.
  - Automatic detection of trapped capacity with visual alert indicators.
- **Filesystem Sizing & Limits Guidance Engine**:
  - Comprehensive side-by-side selection between `exFAT` (recommended for 64GB+, supports games >4GB like PS2/GameCube/Wii/PSP ISOs) and `FAT32` (strict 4GB file size limit, 32GB volume boundary, required for legacy microcontrollers).
  - Dynamic smart recommendation banner analyzing actual physical card capacity.
- **Natural Folder Resolution & Aliases**:
  - Added support for natural directory names: `gbc` = `game boy color`, `gba` = `game boy advance`, `sms` = `sega master system`, `megadrive` = `sega mega drive` / `sega genesis`, `saturn` = `sega saturn`, `gamegear` = `sega game gear`, `gamecube`, and `wii`.
  - Normalized, case-insensitive, punctuation-agnostic directory scanner matching user folder structures reliably.

## [0.2.0] - 2026-10-01

### Added
- **Refined Compact Desktop Interface**:
  - High-density dark layout tailored for desktop power users with reduced font scales (11.5px - 13.5px).
  - High-contrast visual accents, subtle borders, and space-efficient control panels.
- **Enhanced Interactive Favorites Curation (`favorites.json`)**:
  - Real-time search filtering across full ROM collections.
  - Quick selection actions: "⭐ Top Essentials", "Select All Filtered", "Invert Selection", and "Clear All".
  - "Show Selected Only" filter mode to inspect and curate shortlists.
  - Dynamic keyword pattern rules editor to automatically capture game franchises (e.g. "Pokemon", "Zelda", "Mario").
  - Live collapsible `favorites.json` JSON code preview for real-time verification before saving.
  - Instant 1-click `favorites.json` generation and persistence directly in ROM directories.
- **Deep ROM Library Discovery**:
  - Auto-discovery of standard libraries (e.g. `C:\Users\Andreas\OneDrive\Roms`).
  - Recursive scanner matching nested subfolders up to 3 levels deep (e.g. `gameboy advance/roms/`, `gameboy advance/saves/`).
  - Detection and handling of existing local artwork (`Imgs/`, `covers/`) and battery save files (`.sav`).
- **Interactive Prototyping Suite**:
  - Interactive HTML/CSS UX prototype with instant mockup testing and visual previews.

## [0.1.0] - 2026-10-01

### Added
- **Initial Release** of Retro CardMaker for Windows.
- **Drive Manager & Safe SD-Card Formatter**:
  - Automatic detection of drives and removable storage (SD-cards, USB drives).
  - Safety protection preventing any accidental formatting of system partitions (`C:`).
  - Format support for `exFAT` (modern handhelds, 64GB+) and `FAT32` (legacy handhelds, $\le$ 32GB).
  - Option to install without formatting.
- **Multi-Device & Launcher Profiles**:
  - EmulationStation (ES-DE)
  - RetroArch
  - Anbernic RG DS Launcher & Modern Stock OS
  - Anbernic Older Devices / GarlicOS (RG350, RG35XX)
  - Miyoo Mini / OnionOS
  - Daijishō Launcher (Android)
  - Pegasus Frontend (with automatic `metadata.pegasus.txt` generation)
  - Batocera / ArkOS / AmberELEC
  - Custom / Generic platform directory layout
- **Platform Engine & Detection**:
  - Support for 14+ retro platforms: NES, SNES, N64, GB, GBC, GBA, NDS, Sega Master System, Mega Drive/Genesis, Sega Saturn, Dreamcast, PlayStation 1 (PSX), PlayStation 2 (PS2), PlayStation Portable (PSP).
  - Directory alias scanning and extension matching.
- **Curated Favorites & Shortlist (`favorites.json`)**:
  - Support for full ROM copying or curated shortlist copying via `favorites.json`.
  - Built-in curated Top Classics recommendations for all platforms.
  - Interactive favorites manager modal with pattern matching and 1-click generation.
- **Libretro Thumbnails Boxart Scraper**:
  - Integration with Libretro Thumbnails public GitHub CDN (no API keys required).
  - Automatic character normalization and region tag stripping for maximum match rate.
  - Profile-specific artwork destination naming (`downloaded_media/`, `thumbnails/`, `Imgs/`, `media/`, `images/`).
- **Dual Mode Interface**:
  - Native Windows desktop GUI built with `egui` and `eframe`.
  - Full CLI suite and interactive terminal wizard (`retro-cardmaker --cli`).
- **Testing & Quality Assurance**:
  - Unit and integration test suite covering profile routing, favorites parsing, art title normalization, and drive safety.
