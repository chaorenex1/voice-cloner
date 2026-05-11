pub mod demucs_rs_sidecar;
pub mod ffmpeg_sidecar;

use std::{env, path::PathBuf, process::Command};

use crate::app::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct SidecarPaths {
    ffmpeg_path: PathBuf,
    demucs_rs_path: PathBuf,
}

impl Default for SidecarPaths {
    fn default() -> Self {
        Self {
            ffmpeg_path: resolve_binary("VOICE_CLONER_FFMPEG_PATH", "ffmpeg"),
            demucs_rs_path: resolve_binary("VOICE_CLONER_DEMUCS_RS_PATH", "demucs-rs"),
        }
    }
}

impl SidecarPaths {
    pub fn ffmpeg_path(&self) -> PathBuf {
        self.ffmpeg_path.clone()
    }

    pub fn demucs_rs_path(&self) -> PathBuf {
        self.demucs_rs_path.clone()
    }
}

pub fn command_available(path: &PathBuf, version_arg: &str) -> bool {
    Command::new(path)
        .arg(version_arg)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn command_version(path: &PathBuf, version_arg: &str) -> Option<String> {
    let output = Command::new(path).arg(version_arg).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    stdout
        .lines()
        .chain(stderr.lines())
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
}

fn resolve_binary(env_name: &str, base_name: &str) -> PathBuf {
    if let Some(path) = env::var_os(env_name) {
        return PathBuf::from(path);
    }

    for candidate in local_binary_candidates(base_name) {
        if usable_binary_candidate(&candidate) {
            return candidate;
        }
    }

    PathBuf::from(binary_file_name(base_name))
}

fn local_binary_candidates(base_name: &str) -> Vec<PathBuf> {
    let file_name = binary_file_name(base_name);
    let target_file_name = target_binary_file_name(base_name);
    let mut candidates = Vec::new();
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(&file_name));
            candidates.push(dir.join(&target_file_name));
            candidates.push(dir.join("binaries").join(&file_name));
            candidates.push(dir.join("binaries").join(&target_file_name));
        }
    }
    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join("binaries").join(&file_name));
        candidates.push(cwd.join("binaries").join(&target_file_name));
        candidates.push(cwd.join("src-tauri").join("binaries").join(&file_name));
        candidates.push(cwd.join("src-tauri").join("binaries").join(&target_file_name));
    }
    candidates
}

fn target_binary_file_name(base_name: &str) -> String {
    if cfg!(windows) {
        format!("{base_name}-x86_64-pc-windows-msvc.exe")
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        format!("{base_name}-aarch64-apple-darwin")
    } else if cfg!(target_os = "macos") {
        format!("{base_name}-x86_64-apple-darwin")
    } else {
        format!("{base_name}-x86_64-unknown-linux-gnu")
    }
}

fn binary_file_name(base_name: &str) -> String {
    if cfg!(windows) {
        format!("{base_name}.exe")
    } else {
        base_name.to_string()
    }
}

fn usable_binary_candidate(path: &PathBuf) -> bool {
    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

pub fn write_log(path: impl Into<PathBuf>, bytes: &[u8]) -> AppResult<()> {
    let path = path.into();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| AppError::io("creating sidecar log directory", source))?;
    }
    std::fs::write(path, bytes).map_err(|source| AppError::io("writing sidecar log", source))
}
