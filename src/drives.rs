use std::path::Path;
use std::process::Command;
use sysinfo::Disks;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveInfo {
    pub letter: String,           // e.g. "E:"
    pub mount_point: String,      // e.g. "E:\\"
    pub label: String,            // e.g. "NO NAME" or "RETRO"
    pub file_system: String,      // e.g. "FAT32", "exFAT", "NTFS"
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub is_removable: bool,
    pub is_system: bool,
}

impl DriveInfo {
    pub fn total_gb_str(&self) -> String {
        let gb = self.total_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        format!("{:.1} GB", gb)
    }

    pub fn free_gb_str(&self) -> String {
        let gb = self.available_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        format!("{:.1} GB", gb)
    }

    pub fn display_summary(&self) -> String {
        let type_tag = if self.is_removable {
            "Removable SD/USB"
        } else if self.is_system {
            "SYSTEM DRIVE (Protected)"
        } else {
            "Fixed Disk"
        };

        let label_part = if self.label.is_empty() {
            "No Label".to_string()
        } else {
            self.label.clone()
        };

        format!(
            "{} [{}] - {} ({}) - Free: {} / {}",
            self.letter,
            label_part,
            self.file_system,
            type_tag,
            self.free_gb_str(),
            self.total_gb_str()
        )
    }
}

pub fn get_available_drives() -> Vec<DriveInfo> {
    let disks = Disks::new_with_refreshed_list();
    let mut result = Vec::new();

    for disk in &disks {
        let mount_str = disk.mount_point().to_string_lossy().to_string();
        let letter = if mount_str.len() >= 2 && mount_str.as_bytes()[1] == b':' {
            mount_str[0..2].to_uppercase()
        } else {
            mount_str.clone()
        };

        let is_system = letter.eq_ignore_ascii_case("C:");
        let label = disk.name().to_string_lossy().to_string();
        let fs = disk.file_system().to_string_lossy().to_string();

        result.push(DriveInfo {
            letter,
            mount_point: mount_str,
            label,
            file_system: fs,
            total_bytes: disk.total_space(),
            available_bytes: disk.available_space(),
            is_removable: disk.is_removable(),
            is_system,
        });
    }

    // Sort drives: Removable first, then alphabetical by drive letter
    result.sort_by(|a, b| {
        b.is_removable
            .cmp(&a.is_removable)
            .then(a.letter.cmp(&b.letter))
    });

    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatFileSystem {
    Fat32,
    ExFat,
}

impl FormatFileSystem {
    pub fn as_str(&self) -> &'static str {
        match self {
            FormatFileSystem::Fat32 => "FAT32",
            FormatFileSystem::ExFat => "exFAT",
        }
    }
}

/// Executes a fast format on the specified drive with security safeguards.
/// Refuses to format C: drive.
pub fn format_drive(
    drive_letter: &str,
    fs: FormatFileSystem,
    volume_label: &str,
) -> Result<String, String> {
    let clean_letter = drive_letter.trim_end_matches('\\').trim_end_matches('/');
    if clean_letter.eq_ignore_ascii_case("C:") || clean_letter.eq_ignore_ascii_case("C") {
        return Err("SAFETY VIOLATION: Cannot format system drive C:!".to_string());
    }

    let letter_char = clean_letter.chars().next().ok_or("Invalid drive letter")?;
    let target = format!("{}:", letter_char.to_ascii_uppercase());
    let fs_str = fs.as_str();
    let label = if volume_label.trim().is_empty() {
        "RETRO"
    } else {
        volume_label.trim()
    };

    // Try Windows native format.com first
    let output = Command::new("cmd")
        .args([
            "/C",
            "format",
            &target,
            &format!("/FS:{}", fs_str),
            "/Q",
            &format!("/V:{}", label),
            "/Y",
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let msg = String::from_utf8_lossy(&out.stdout).to_string();
            Ok(format!("Successfully formatted {} as {}: {}", target, fs_str, msg.trim()))
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            // Fallback to PowerShell Format-Volume
            let ps_script = format!(
                "Format-Volume -DriveLetter '{}' -FileSystem '{}' -NewFileSystemLabel '{}' -Confirm:$false",
                letter_char.to_ascii_uppercase(),
                fs_str,
                label
            );

            let ps_output = Command::new("powershell")
                .args(["-NoProfile", "-Command", &ps_script])
                .output();

            match ps_output {
                Ok(ps_out) if ps_out.status.success() => {
                    Ok(format!("Successfully formatted {} using PowerShell.", target))
                }
                Ok(ps_out) => {
                    let ps_err = String::from_utf8_lossy(&ps_out.stderr).to_string();
                    Err(format!(
                        "Format failed. CMD error: {} {}. PowerShell error: {}",
                        stderr, stdout, ps_err
                    ))
                }
                Err(e) => Err(format!("Format command failed: {}. Details: {}", e, stderr)),
            }
        }
        Err(e) => Err(format!("Could not execute format tool: {}", e)),
    }
}

#[allow(dead_code)]
pub fn ensure_directory_exists(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}
