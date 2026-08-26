use std::{
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredVolume {
    pub volume_id: String,
    pub root_path: String,
    pub label: Option<String>,
    pub filesystem_type: Option<String>,
    pub total_bytes: Option<u64>,
    pub free_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathProperties {
    pub path: String,
    pub is_directory: bool,
    pub size: u64,
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
    pub read_only: bool,
}

pub fn normalize_path(input: &str) -> Result<String> {
    let expanded = input.trim().replace('/', "\\");
    if expanded.is_empty() {
        bail!("Der Pfad ist leer");
    }
    let path = Path::new(&expanded);
    let mut output = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                output.pop();
            }
            other => output.push(other.as_os_str()),
        }
    }
    let mut value = output.to_string_lossy().to_string();
    if value.len() >= 2 && value.as_bytes()[1] == b':' {
        value.replace_range(0..1, &value[..1].to_ascii_uppercase());
        if value.len() == 2 {
            value.push('\\');
        }
    }
    while value.len() > 3 && value.ends_with('\\') {
        value.pop();
    }
    Ok(value)
}

#[cfg(windows)]
pub fn discover_volumes() -> Result<Vec<DiscoveredVolume>> {
    use windows_sys::Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetLogicalDrives, GetVolumeInformationW,
    };
    let mask = unsafe { GetLogicalDrives() };
    let mut volumes = Vec::new();
    for index in 0..26 {
        if mask & (1 << index) == 0 {
            continue;
        }
        let root = format!("{}:\\", (b'A' + index as u8) as char);
        let wide = wide(&root);
        let mut label = vec![0_u16; 261];
        let mut filesystem = vec![0_u16; 64];
        let mut serial = 0_u32;
        let mut max_component = 0_u32;
        let mut flags = 0_u32;
        let info_ok = unsafe {
            GetVolumeInformationW(
                wide.as_ptr(),
                label.as_mut_ptr(),
                label.len() as u32,
                &mut serial,
                &mut max_component,
                &mut flags,
                filesystem.as_mut_ptr(),
                filesystem.len() as u32,
            )
        } != 0;
        let mut free_available = 0_u64;
        let mut total = 0_u64;
        let mut total_free = 0_u64;
        let space_ok = unsafe {
            GetDiskFreeSpaceExW(
                wide.as_ptr(),
                &mut free_available,
                &mut total,
                &mut total_free,
            )
        } != 0;
        volumes.push(DiscoveredVolume {
            volume_id: if info_ok {
                format!("{:08X}", serial)
            } else {
                root.clone()
            },
            root_path: root,
            label: info_ok.then(|| from_wide(&label)).filter(|v| !v.is_empty()),
            filesystem_type: info_ok
                .then(|| from_wide(&filesystem))
                .filter(|v| !v.is_empty()),
            total_bytes: space_ok.then_some(total),
            free_bytes: space_ok.then_some(total_free),
        });
    }
    Ok(volumes)
}

#[cfg(not(windows))]
pub fn discover_volumes() -> Result<Vec<DiscoveredVolume>> {
    Ok(vec![DiscoveredVolume {
        volume_id: "root".into(),
        root_path: "/".into(),
        label: None,
        filesystem_type: None,
        total_bytes: None,
        free_bytes: None,
    }])
}

pub fn open_path(path: &str) -> Result<()> {
    opener::open(path).with_context(|| format!("{} konnte nicht geöffnet werden", path))?;
    Ok(())
}

pub fn reveal_path(path: &str) -> Result<()> {
    #[cfg(windows)]
    {
        if Path::new(path).is_dir() {
            Command::new("explorer.exe").arg(path).spawn()?;
        } else {
            Command::new("explorer.exe")
                .arg(format!("/select,{path}"))
                .spawn()?;
        }
    }
    #[cfg(not(windows))]
    {
        if let Some(parent) = Path::new(path).parent() {
            opener::open(parent)?;
        }
    }
    Ok(())
}

pub fn validate_drag_path(path: &str) -> Result<()> {
    let path_value = Path::new(path);
    if !path_value.is_absolute() {
        bail!("{} ist kein absoluter Pfad", path);
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            Storage::FileSystem::GetDriveTypeW, System::WindowsProgramming::DRIVE_REMOTE,
        };

        if path.starts_with("\\\\") {
            bail!("Dateien auf Netzwerkpfaden können derzeit nicht herausgezogen werden");
        }
        if path.len() >= 2 && path.as_bytes()[1] == b':' {
            let root = format!("{}\\", &path[..2]);
            if unsafe { GetDriveTypeW(wide(&root).as_ptr()) } == DRIVE_REMOTE {
                bail!("Dateien auf Netzlaufwerken können derzeit nicht herausgezogen werden");
            }
        }
    }
    if !path_value.exists() {
        bail!("{} ist nicht mehr vorhanden", path);
    }
    Ok(())
}

pub fn create_directory(parent: &str, name: &str) -> Result<PathBuf> {
    validate_name(name)?;
    let target = Path::new(parent).join(name);
    fs::create_dir(&target)
        .with_context(|| format!("Ordner {} konnte nicht erstellt werden", target.display()))?;
    Ok(target)
}

pub fn rename_path(source: &str, new_name: &str) -> Result<PathBuf> {
    validate_name(new_name)?;
    let source = Path::new(source);
    let parent = source
        .parent()
        .context("Der Quellpfad hat keinen übergeordneten Ordner")?;
    let destination = parent.join(new_name);
    fs::rename(source, &destination)
        .with_context(|| format!("{} konnte nicht umbenannt werden", source.display()))?;
    Ok(destination)
}

pub fn delete_to_trash(paths: &[String]) -> Result<()> {
    trash::delete_all(paths).context(
        "Die ausgewählten Einträge konnten nicht vollständig in den Papierkorb verschoben werden",
    )?;
    Ok(())
}

pub fn copy_paths(sources: &[String], destination: &str) -> Result<Vec<(PathBuf, PathBuf)>> {
    let destination = Path::new(destination);
    if !destination.is_dir() {
        bail!("Das Kopierziel ist kein Ordner");
    }
    let mut copied = Vec::new();
    for source in sources {
        let source_path = Path::new(source);
        let name = source_path.file_name().context("Ungültiger Quellpfad")?;
        let target = destination.join(name);
        if target.exists() {
            bail!("{} existiert bereits", target.display());
        }
        copy_tree(source_path, &target)?;
        copied.push((source_path.to_path_buf(), target));
    }
    Ok(copied)
}

pub fn move_paths(sources: &[String], destination: &str) -> Result<Vec<(PathBuf, PathBuf)>> {
    let destination = Path::new(destination);
    if !destination.is_dir() {
        bail!("Das Verschiebeziel ist kein Ordner");
    }
    let mut moved = Vec::new();
    for source in sources {
        let source_path = Path::new(source);
        let target = destination.join(source_path.file_name().context("Ungültiger Quellpfad")?);
        if target.exists() {
            bail!("{} existiert bereits", target.display());
        }
        match fs::rename(source_path, &target) {
            Ok(()) => {}
            Err(error) if error.raw_os_error() == Some(17) => {
                copy_tree(source_path, &target)?;
                if source_path.is_dir() {
                    fs::remove_dir_all(source_path)?;
                } else {
                    fs::remove_file(source_path)?;
                }
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("{} konnte nicht verschoben werden", source_path.display())
                })
            }
        }
        moved.push((source_path.to_path_buf(), target));
    }
    Ok(moved)
}

pub fn properties(path: &str) -> Result<PathProperties> {
    let metadata = fs::metadata(path)?;
    Ok(PathProperties {
        path: path.to_string(),
        is_directory: metadata.is_dir(),
        size: metadata.len(),
        created_at: to_timestamp(metadata.created().ok()),
        modified_at: to_timestamp(metadata.modified().ok()),
        read_only: metadata.permissions().readonly(),
    })
}

fn validate_name(name: &str) -> Result<()> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || trimmed.chars().any(|c| "<>:\"/\\|?*".contains(c))
    {
        bail!("Ungültiger Datei- oder Ordnername");
    }
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        bail!(
            "Symbolische Verknüpfungen werden noch nicht kopiert: {}",
            source.display()
        );
    }
    if metadata.is_file() {
        fs::copy(source, destination)?;
        return Ok(());
    }
    fs::create_dir(destination)?;
    let mut stack = vec![(source.to_path_buf(), destination.to_path_buf())];
    while let Some((current_source, current_destination)) = stack.pop() {
        for child in fs::read_dir(&current_source)? {
            let child = child?;
            let child_source = child.path();
            let child_target = current_destination.join(child.file_name());
            let child_meta = child.file_type()?;
            if child_meta.is_symlink() {
                continue;
            }
            if child_meta.is_dir() {
                fs::create_dir(&child_target)?;
                stack.push((child_source, child_target));
            } else if child_meta.is_file() {
                fs::copy(child_source, child_target)?;
            }
        }
    }
    Ok(())
}

fn to_timestamp(value: Option<SystemTime>) -> Option<String> {
    value
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| DateTime::<Utc>::from_timestamp(d.as_secs() as i64, d.subsec_nanos()))
        .map(|d| d.to_rfc3339())
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
#[cfg(windows)]
fn from_wide(value: &[u16]) -> String {
    String::from_utf16_lossy(&value[..value.iter().position(|v| *v == 0).unwrap_or(value.len())])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_windows_paths() {
        assert_eq!(normalize_path("c:/Users/../Data/").unwrap(), "C:\\Data");
        assert_eq!(normalize_path("d:").unwrap(), "D:\\");
    }

    #[test]
    fn rejects_unsafe_names() {
        assert!(validate_name("hello.txt").is_ok());
        assert!(validate_name("../oops").is_err());
        assert!(validate_name("bad:name").is_err());
    }

    #[test]
    fn rejects_relative_drag_paths() {
        assert!(validate_drag_path("relative\\file.txt").is_err());
    }
}
