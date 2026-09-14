//! Paths and persistence for Lazy Herd config/state.

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub state_dir: PathBuf,
}

impl Paths {
    pub fn resolve() -> Self {
        let config_dir = std::env::var_os("HERDR_PLUGIN_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| fallback_dir("config"));
        let state_dir = std::env::var_os("HERDR_PLUGIN_STATE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| fallback_dir("state"));
        let _ = fs::create_dir_all(&config_dir);
        let _ = fs::create_dir_all(&state_dir);
        Self {
            config_dir,
            state_dir,
        }
    }

    pub fn join_config(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.config_dir.join(rel)
    }

    pub fn join_state(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.state_dir.join(rel)
    }

    pub fn ensure_subdir(&self, under_config: bool, name: &str) -> Result<PathBuf> {
        let dir = if under_config {
            self.join_config(name)
        } else {
            self.join_state(name)
        };
        fs::create_dir_all(&dir)
            .with_context(|| format!("create directory {}", dir.display()))?;
        Ok(dir)
    }
}

fn fallback_dir(kind: &str) -> PathBuf {
    if let Some(dirs) = ProjectDirs::from("", "herdr", "plugins") {
        let base = match kind {
            "state" => dirs.data_local_dir().join("lazy-herd"),
            _ => dirs.config_dir().join("lazy-herd"),
        };
        return base;
    }
    PathBuf::from(".").join(format!(".lazy-herd-{kind}"))
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    let value = serde_json::from_str(&raw)
        .with_context(|| format!("parse JSON {}", path.display()))?;
    Ok(Some(value))
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(value)?;
    fs::write(path, raw).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn read_toml<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    let value = toml::from_str(&raw).with_context(|| format!("parse TOML {}", path.display()))?;
    Ok(Some(value))
}

pub fn write_toml<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = toml::to_string_pretty(value)?;
    fs::write(path, raw).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn list_toml_stem_files(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::tempdir;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Sample {
        name: String,
        n: u32,
    }

    #[test]
    fn json_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("x.json");
        let sample = Sample {
            name: "a".into(),
            n: 3,
        };
        write_json(&path, &sample).unwrap();
        let loaded: Sample = read_json(&path).unwrap().unwrap();
        assert_eq!(loaded, sample);
    }

    #[test]
    fn toml_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("x.toml");
        let sample = Sample {
            name: "b".into(),
            n: 9,
        };
        write_toml(&path, &sample).unwrap();
        let loaded: Sample = read_toml(&path).unwrap().unwrap();
        assert_eq!(loaded, sample);
    }
}
