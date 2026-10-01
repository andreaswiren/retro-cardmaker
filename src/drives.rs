use std::collections::HashMap;
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
    pub disk_number: Option<u32>,
    pub physical_disk_size: Option<u64>,
    pub has_hidden_partitions: bool,
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

    pub fn physical_gb_str(&self) -> String {
        if let Some(bytes) = self.physical_disk_size {
            let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
            format!("{:.1} GB", gb)
        } else {
            self.total_gb_str()
        }
    }

    pub fn display_summary(&self) -> String {
        let type_tag = if self.is_system {
            "SYSTEM DRIVE (Protected)"
        } else if self.is_removable {
            "Removable SD/USB"
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

    // Query partition-to-disk mapping via PowerShell
    let ps_out = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-Partition | Where-Object DriveLetter | Select-Object DriveLetter, DiskNumber | ConvertTo-Csv -NoTypeInformation",
        ])
        .output();

    let mut letter_to_disk: HashMap<String, u32> = HashMap::new();
    if let Ok(out) = ps_out {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines().skip(1) {
            let parts: Vec<&str> = line.split(',').map(|s| s.trim().trim_matches('"')).collect();
            if parts.len() >= 2 {
                let letter = parts[0].to_uppercase();
                if let Ok(num) = parts[1].parse::<u32>() {
                    letter_to_disk.insert(letter, num);
                }
            }
        }
    }

    // Query physical disks to find physical capacity, BusType, and system flags
    let ps_disk_out = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-Disk | Select-Object Number, Size, BusType, IsBoot, IsSystem | ConvertTo-Csv -NoTypeInformation",
        ])
        .output();

    let mut disk_info_map: HashMap<u32, (u64, String, bool, bool)> = HashMap::new();
    if let Ok(out) = ps_disk_out {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines().skip(1) {
            let parts: Vec<&str> = line.split(',').map(|s| s.trim().trim_matches('"')).collect();
            if parts.len() >= 5 {
                if let Ok(num) = parts[0].parse::<u32>() {
                    let size = parts[1].parse::<u64>().unwrap_or(0);
                    let bus = parts[2].to_string();
                    let is_boot = parts[3].eq_ignore_ascii_case("True");
                    let is_sys = parts[4].eq_ignore_ascii_case("True");
                    disk_info_map.insert(num, (size, bus, is_boot, is_sys));
                }
            }
        }
    }

    for disk in &disks {
        let mount_str = disk.mount_point().to_string_lossy().to_string();
        let letter = if mount_str.len() >= 2 && mount_str.as_bytes()[1] == b':' {
            mount_str[0..2].to_uppercase()
        } else {
            mount_str.clone()
        };

        let raw_char = letter.trim_end_matches(':').to_uppercase();
        let disk_num = letter_to_disk.get(&raw_char).copied();
        let (phys_size, bus_type, is_boot, is_sys_disk) = disk_num
            .and_then(|num| disk_info_map.get(&num).cloned())
            .unwrap_or((0, String::new(), false, false));

        let is_system = letter.eq_ignore_ascii_case("C:") || is_boot || is_sys_disk;
        let label = disk.name().to_string_lossy().to_string();
        let fs = disk.file_system().to_string_lossy().to_string();

        let is_removable = (disk.is_removable() || bus_type.eq_ignore_ascii_case("USB")) && !is_system;
        let has_hidden_partitions = is_removable
            && phys_size > 0
            && (phys_size > disk.total_space() + (800 * 1024 * 1024)); // >800MB hidden/unallocated

        result.push(DriveInfo {
            letter,
            mount_point: mount_str,
            label,
            file_system: fs,
            total_bytes: disk.total_space(),
            available_bytes: disk.available_space(),
            is_removable,
            is_system,
            disk_number: disk_num,
            physical_disk_size: if phys_size > 0 { Some(phys_size) } else { None },
            has_hidden_partitions,
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

/// Executes a fast format on the specified drive with strict security safeguards.
/// Formatting is strictly restricted to removable USB/SD drives. Refuses C: and system drives.
pub fn format_drive(
    drive_letter: &str,
    fs: FormatFileSystem,
    volume_label: &str,
) -> Result<String, String> {
    let clean_letter = drive_letter.trim_end_matches('\\').trim_end_matches('/');
    if clean_letter.eq_ignore_ascii_case("C:") || clean_letter.eq_ignore_ascii_case("C") {
        return Err("SAFETY VIOLATION: Cannot format system drive C:!".to_string());
    }

    let available = get_available_drives();
    if let Some(drive) = available.iter().find(|d| d.letter.eq_ignore_ascii_case(clean_letter)) {
        if !drive.is_removable {
            return Err("SAFETY VIOLATION: Formatting is strictly restricted to removable USB and SD-card drives.".to_string());
        }
        if drive.is_system {
            return Err("SAFETY VIOLATION: Selected drive is marked as a Windows System disk. Aborting!".to_string());
        }
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

/// Fully wipes all partition tables, hidden partitions, and recovery structures on the removable disk,
/// converts to MBR, creates a single active full-size primary partition, and formats it.
pub fn wipe_and_repartition_drive(
    drive_letter: &str,
    fs: FormatFileSystem,
    volume_label: &str,
) -> Result<String, String> {
    let clean_letter = drive_letter.trim_end_matches('\\').trim_end_matches('/');
    if clean_letter.eq_ignore_ascii_case("C:") || clean_letter.eq_ignore_ascii_case("C") {
        return Err("SAFETY VIOLATION: Cannot repartition system drive C:!".to_string());
    }

    let available = get_available_drives();
    let drive = available
        .iter()
        .find(|d| d.letter.eq_ignore_ascii_case(clean_letter))
        .ok_or_else(|| format!("Drive {} not found in system drive list.", clean_letter))?;

    if !drive.is_removable {
        return Err("SAFETY VIOLATION: Repartitioning is strictly restricted to removable USB and SD-card drives.".to_string());
    }

    if drive.is_system {
        return Err("SAFETY VIOLATION: Drive is flagged as a Windows System disk. Aborting!".to_string());
    }

    let disk_num = drive.disk_number.ok_or_else(|| {
        format!("Could not determine physical disk number for drive {}.", clean_letter)
    })?;

    // Pre-flight safety check via PowerShell: Ensure physical disk is NOT boot or system disk
    let verify_ps = format!(
        "$d = Get-Disk -Number {}; if ($d.IsBoot -or $d.IsSystem) {{ exit 101 }} else {{ exit 0 }}",
        disk_num
    );
    let verify_out = Command::new("powershell")
        .args(["-NoProfile", "-Command", &verify_ps])
        .output();

    if let Ok(out) = verify_out {
        if out.status.code() == Some(101) {
            return Err("SAFETY VIOLATION: Target disk is a Windows Boot or System disk. Operation aborted!".to_string());
        }
    }

    let letter_char = clean_letter
        .chars()
        .next()
        .ok_or("Invalid drive letter")?
        .to_ascii_uppercase();
    let fs_str = fs.as_str().to_lowercase();
    let label = if volume_label.trim().is_empty() {
        "RETRO"
    } else {
        volume_label.trim()
    };

    // Generate diskpart script
    let temp_script = std::env::temp_dir().join(format!("cardmaker_wipe_{}_{}.txt", disk_num, std::process::id()));
    let script_content = format!(
        "select disk {}\nclean\nconvert mbr\ncreate partition primary\nselect partition 1\nactive\nformat fs={} quick label=\"{}\"\nassign letter={}\n",
        disk_num, fs_str, label, letter_char
    );

    std::fs::write(&temp_script, script_content)
        .map_err(|e| format!("Failed to create temporary diskpart script: {}", e))?;

    let output = Command::new("diskpart")
        .args(["/s", &temp_script.to_string_lossy()])
        .output();

    let _ = std::fs::remove_file(&temp_script);

    match output {
        Ok(out) if out.status.success() => {
            let msg = String::from_utf8_lossy(&out.stdout).to_string();
            Ok(format!(
                "Successfully wiped hidden partitions, repartitioned MBR, and formatted {}: on Disk {}: {}",
                letter_char, disk_num, msg.trim()
            ))
        }
        Ok(out) => {
            let err_stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let err_stderr = String::from_utf8_lossy(&out.stderr).to_string();
            Err(format!("diskpart repartition failed: {} {}", err_stdout, err_stderr))
        }
        Err(e) => Err(format!("Could not execute diskpart utility: {}", e)),
    }
}

#[allow(dead_code)]
pub fn ensure_directory_exists(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}
