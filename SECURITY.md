# Security Policy

## Supported Versions

Security updates and patches are provided for the current active release line:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

---

## Safety Safeguards in Retro CardMaker

Retro CardMaker interacts directly with physical and removable disk devices. To prevent accidental data loss:

1. **System Drive Protection**:
   - The primary system drive (`C:`) and Windows operating system partition are hardcoded as strictly non-formattable.
   - Any attempt to run formatting on `C:` will be rejected with an immediate safety violation error.
2. **Explicit Format Confirmations**:
   - Formats require explicit double-confirmation check in both the GUI and CLI.
   - Default install mode skips formatting to preserve existing data unless the user explicitly chooses to format.
3. **No Unauthenticated Remote Code**:
   - Artwork is retrieved strictly over HTTPS from the verified Libretro Thumbnails public GitHub CDN.
   - No executable binaries or scripts are downloaded from remote sources.

---

## Reporting a Vulnerability

If you discover a security vulnerability or critical safety bug in Retro CardMaker, please report it responsibly:

1. **Do not create a public issue**.
2. Email security concerns to `andreas.wiren@gmail.com` (or submit a Private Vulnerability Advisory on GitHub).
3. Include:
   - A detailed description of the vulnerability or risk.
   - Steps to reproduce or proof-of-concept scenario.
   - Operating system and environment details.

You will receive an acknowledgment within 48 hours, followed by an assessment and patch schedule.
