# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
