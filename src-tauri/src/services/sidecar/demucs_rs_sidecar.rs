use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    app::error::{AppError, AppResult},
    domain::voice_separation::VoiceSeparationModel,
};

use super::{command_available, command_version, write_log, SidecarPaths};

#[derive(Debug, Clone)]
pub struct DemucsRsSidecar {
    binary_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct DemucsRunOutput {
    pub vocals: PathBuf,
    pub drums: PathBuf,
    pub bass: PathBuf,
    pub other: PathBuf,
}

impl Default for DemucsRsSidecar {
    fn default() -> Self {
        Self::new(SidecarPaths::default().demucs_rs_path())
    }
}

impl DemucsRsSidecar {
    pub fn new(binary_path: impl Into<PathBuf>) -> Self {
        Self {
            binary_path: binary_path.into(),
        }
    }

    pub fn available(&self) -> bool {
        command_available(&self.binary_path, "--help")
    }

    pub fn version(&self) -> Option<String> {
        command_version(&self.binary_path, "--help")
    }

    pub fn separate(
        &self,
        input: &Path,
        output_dir: &Path,
        model: &VoiceSeparationModel,
        stdout_log: &Path,
        stderr_log: &Path,
    ) -> AppResult<DemucsRunOutput> {
        std::fs::create_dir_all(output_dir)
            .map_err(|source| AppError::io("creating demucs output directory", source))?;
        let output = Command::new(&self.binary_path)
            .args([
                "--model",
                model.as_demucs_model(),
                "--output",
                &output_dir.to_string_lossy(),
                &input.to_string_lossy(),
            ])
            .output()
            .map_err(|source| AppError::io("starting demucs-rs sidecar", source))?;
        write_log(stdout_log, &output.stdout)?;
        write_log(stderr_log, &output.stderr)?;
        if !output.status.success() {
            return Err(AppError::offline_job(format!(
                "demucs-rs failed while separating audio (exit: {:?})",
                output.status.code()
            )));
        }
        find_demucs_stems(output_dir)
    }
}

fn find_demucs_stems(output_dir: &Path) -> AppResult<DemucsRunOutput> {
    let wavs = collect_wavs(output_dir)?;
    let vocals = find_stem(&wavs, "vocals")?;
    let drums = find_stem(&wavs, "drums")?;
    let bass = find_stem(&wavs, "bass")?;
    let other = find_stem(&wavs, "other")?;
    Ok(DemucsRunOutput {
        vocals,
        drums,
        bass,
        other,
    })
}

fn collect_wavs(path: &Path) -> AppResult<Vec<PathBuf>> {
    let mut wavs = Vec::new();
    if !path.exists() {
        return Ok(wavs);
    }
    for entry in std::fs::read_dir(path).map_err(|source| AppError::io("reading demucs output directory", source))? {
        let entry = entry.map_err(|source| AppError::io("reading demucs output entry", source))?;
        let path = entry.path();
        if path.is_dir() {
            wavs.extend(collect_wavs(&path)?);
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("wav"))
            == Some(true)
        {
            wavs.push(path);
        }
    }
    Ok(wavs)
}

fn find_stem(wavs: &[PathBuf], stem: &str) -> AppResult<PathBuf> {
    wavs.iter()
        .find(|path| {
            path.file_stem()
                .and_then(|name| name.to_str())
                .map(|name| name.eq_ignore_ascii_case(stem) || name.to_ascii_lowercase().contains(&format!(".{stem}")))
                .unwrap_or(false)
        })
        .cloned()
        .ok_or_else(|| AppError::offline_job(format!("demucs-rs did not produce required {stem}.wav stem")))
}
