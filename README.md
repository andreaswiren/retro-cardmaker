# 🎮 Retro CardMaker

[![Rust](https://img.shields.io/badge/Rust-1.85+-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/Platform-Windows-blue.svg)](https://microsoft.com/windows)
[![License](https://img.shields.io/badge/License-MIT%2FApache--2.0-green.svg)](LICENSE)
[![Release](https://img.shields.io/badge/Release-v0.2.1-brightgreen.svg)](https://github.com/andreaswiren/retro-cardmaker/releases)

**Retro CardMaker** is a high-performance Windows application and QuickInstaller designed to format SD-cards, organize ROM directories, manage curated "Best of" game shortlists (`favorites.json`), and automatically scrape high-resolution boxart formatted specifically for retro handheld devices and frontends.

---

## 🌟 Key Features

### 1. 💾 Safe SD-Card Formatting, Repartitioning & Guidance
- **Removable-Only Drive Formatting Guard**: Formatting is strictly restricted to removable USB flash drives and SD cards (`is_removable == true`). System disk `C:` and fixed internal NVMe SSDs are locked against accidental formatting in both GUI and CLI.
- **⚡ Full Repartition & Clean Wipe (Diskpart MBR)**: Completely wipes all partition tables, removing hidden OEM, recovery, and Linux `ext4` partitions left over from prior handheld OS installations (GarlicOS, OnionOS, ArkOS, Batocera). Reclaims 100% of physical storage into a single active MBR primary partition.
- **Drive Auto-Detection & Trapped Storage Alerts**: Discovers connected disks, volume labels, physical disk sizes, and flags when unallocated or foreign partitions are trapping storage capacity.
- **Filesystem Sizing & Limits Guidance Engine**:
  - `exFAT`: Recommended for modern handhelds (Anbernic RG Cube, Odin, Steam Deck, Android) and SD cards 64GB+. No 4GB file size limit (essential for PS2, Wii, PSP ISOs, and multi-disc CHD files).
  - `FAT32`: Recommended for legacy handhelds (Anbernic RG350, RG35XX GarlicOS, Miyoo Mini OnionOS) and cards $\le$ 32GB. Enforces awareness of the strict 4GB single file size limit and Windows 32GB volume boundary.
- **Dynamic Smart Recommendations**: Analyzes actual physical card capacity and displays contextual advice for optimal filesystem selection.
- **Customizable Target Subfolder**: Install ROMs to SD-card root or dedicated folders like `roms/` or `Roms/`.

### 2. 🕹️ Device & Frontend Presets
Pre-configured directory layouts and artwork naming rules matching leading emulation environments:
- **EmulationStation (ES-DE)**: `roms/<system>/` with `downloaded_media/<system>/covers/<rom>.png`.
- **RetroArch**: `roms/<system>/` with `thumbnails/<Libretro_System>/Named_Boxarts/<rom>.png`.
- **Anbernic RG DS Launcher / Modern Stock OS**: Standard codes (`FC`, `SFC`, `GBA`, `NDS`, `MD`, `PS`, etc.) with `Imgs/<rom>.png` for instant dual-screen handheld display.
- **Anbernic Older Devices / GarlicOS**: `Roms/<CODE>/` with `Roms/<CODE>/Imgs/<rom>.png`.
- **Miyoo Mini / OnionOS**: `Roms/<CODE>/` with `Roms/<CODE>/Imgs/<rom>.png`.
- **Daijishō Launcher (Android)**: `roms/<system>/thumbnails/<rom>.png`.
- **Pegasus Frontend**: `roms/<system>/media/<rom>.png` with auto-generated `metadata.pegasus.txt`.
- **Batocera / ArkOS / AmberELEC**: `roms/<system>/images/<rom>-image.png`.
- **Custom**: User-definable layout.

### 3. 📂 Supported Platforms & Natural Folder Detection
Built-in platform scanner supporting flexible, case-insensitive, and normalized folder naming:
- **Nintendo**:
  - NES / Famicom (`nes`, `fc`, `famicom`, `nintendo entertainment system`)
  - Super Nintendo (`snes`, `sfc`, `super nintendo`, `super nintendo entertainment system`)
  - Nintendo 64 (`n64`, `nintendo 64`)
  - Game Boy (`gb`, `game boy`, `gameboy`)
  - Game Boy Color (`gbc`, `game boy color`, `gameboy color`, `gb color`)
  - Game Boy Advance (`gba`, `game boy advance`, `gameboy advance`, `gb advance`)
  - Nintendo DS (`nds`, `ds`, `nintendo ds`)
  - Nintendo GameCube (`gamecube`, `gc`, `nintendo gamecube`)
  - Nintendo Wii (`wii`, `nintendo wii`)
- **Sega**:
  - Sega Master System (`sms`, `master system`, `sega master system`)
  - Sega Mega Drive / Genesis (`megadrive`, `genesis`, `md`, `sega mega drive`, `sega genesis`)
  - Sega Game Gear (`gamegear`, `gg`, `sega game gear`)
  - Sega Saturn (`saturn`, `ss`, `sega saturn`)
  - Sega Dreamcast (`dreamcast`, `dc`, `sega dreamcast`)
- **Sony**:
  - PlayStation 1 / PSX (`psx`, `ps1`, `playstation`, `sony playstation`)
  - PlayStation 2 (`ps2`, `playstation 2`, `sony playstation 2`)
  - PlayStation Portable (`psp`, `playstation portable`, `sony playstation portable`)

### 4. ⭐ Curated Games & `favorites.json`
- Supports copying either the **Full ROM library** or only a **Curated Shortlist** of essential classics.
- Reads and writes `favorites.json` in each platform folder:
  ```json
  {
    "platform": "gba",
    "title": "Essential GBA Classics",
    "games": [
      "Pokemon - Emerald Version (USA, Europe).gba",
      "Legend of Zelda, The - The Minish Cap (USA).gba",
      "Metroid Fusion (USA).gba",
      "Castlevania - Aria of Sorrow (USA).gba"
    ],
    "patterns": [
      "Pokemon - Emerald",
      "Minish Cap",
      "Metroid Fusion"
    ]
  }
  ```
- **Built-in Favorites Manager**: Includes pre-seeded "Top Classics" recommendations for every system. Generate a curated shortlist in 1 click or customize via the interactive GUI!
- **Fuzzy & Pattern Matching**: Matches exact filenames, clean stems, and search patterns (e.g. `"Minish Cap"` matches `0543 - Legend of Zelda - The Minish Cap (U).zip`).

### 5. 🖼️ Automatic Boxart Scraper
- Direct integration with the **Libretro Thumbnails** open-source repository.
- **100% Free**: No API keys, no accounts, and no rate limits.
- **Smart Title Normalization**: Strips illegal filesystem characters (`&`, `*`, `/`, `:`, `?`, etc.) according to Libretro standards and falls back to clean base names without region tags if needed.
- **Automatic Renaming**: Boxarts are saved in the exact folder and naming pattern expected by your selected frontend.

### 6. 🖥️ Dual Mode: Rich Desktop GUI + Terminal CLI Wizard
- **Desktop GUI**: Built with `egui` / `eframe` featuring a retro dark mode, live progress bars, scrolling activity console, and interactive modals.
- **CLI Wizard**: Run headless or in terminal via `--cli` or commands (`list-drives`, `format`, `install`, `wizard`).

---

## 🚀 Quick Start

### Running the Desktop GUI
Simply launch the executable:
```bash
retro-cardmaker
```

### Running the Terminal Wizard
For headless or terminal-only environments:
```bash
retro-cardmaker --cli
# or
retro-cardmaker wizard
```

### CLI Subcommands
List connected drives and removable SD cards:
```bash
retro-cardmaker list-drives
```

Format an SD-card:
```bash
retro-cardmaker format --drive E: --fs exfat --label RETRO
```

Automated QuickInstall:
```bash
retro-cardmaker install \
  --drive E: \
  --dest-folder roms \
  --source D:\Emulation\Roms \
  --profile es-de \
  --favorites-only \
  --download-art
```

---

## 🛠️ Building from Source

### Prerequisites
- [Rust toolchain](https://rustup.rs/) (1.85 or newer recommended)
- Windows 10 / 11 (x86_64)

### Build
```bash
git clone https://github.com/andreaswiren/retro-cardmaker.git
cd retro-cardmaker
cargo build --release
```
The compiled binary will be located at `target\release\retro-cardmaker.exe`.

### Running Tests
```bash
cargo test
```

---

## 📁 Repository Structure

```
retro-cardmaker/
├── src/
│   ├── main.rs              # Entry point (CLI parser & GUI launcher)
│   ├── lib.rs               # Library root
│   ├── gui.rs               # Desktop GUI (egui / eframe)
│   ├── cli.rs               # Command-line interface & interactive wizard
│   ├── drives.rs            # Drive detection & safe formatting engine
│   ├── platforms.rs         # 14+ retro platforms registry & top classics
│   ├── launcher_profiles.rs # EmulationStation, RetroArch, Anbernic, Daijisho, Pegasus profiles
│   ├── favorites.rs         # favorites.json loader, saver & pattern matcher
│   ├── art_scraper.rs       # Libretro CDN boxart downloader & title normalizer
│   └── installer.rs         # Background QuickInstaller execution engine
├── tests/
│   └── cardmaker_tests.rs   # Comprehensive integration test suite
├── Cargo.toml
├── README.md
├── CHANGELOG.md
└── SECURITY.md
```

---

## 📄 License
This project is licensed under the MIT License or Apache-2.0 at your option.
