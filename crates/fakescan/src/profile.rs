use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub label: String,
    pub connection: String,
    pub optical: Option<Optical>,
    pub feeder: FeederCaps,
    pub dpi: Vec<u32>,
    pub formats: Vec<String>,
    pub page_sizes: Vec<String>,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Optical {
    pub x_dpi: u32,
    pub y_dpi: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeederCaps {
    pub sticky_ready: bool,
    pub pages_min: u32,
    pub pages_max: u32,
}

impl Profile {
    pub fn load(dir: &Path, id: &str) -> Result<Self> {
        let path = dir.join(format!("{id}.json"));
        if !path.is_file() {
            bail!("profile not found: {}", path.display());
        }
        let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let profile: Profile =
            serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        if profile.id != id {
            bail!(
                "profile id mismatch: file {} declares id {:?}",
                path.display(),
                profile.id
            );
        }
        Ok(profile)
    }

    pub fn list(dir: &Path) -> Result<Vec<String>> {
        let mut ids = Vec::new();
        if !dir.is_dir() {
            return Ok(ids);
        }
        for entry in fs::read_dir(dir).with_context(|| format!("list {}", dir.display()))? {
            let entry = entry?;
            let path: PathBuf = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                ids.push(stem.to_string());
            }
        }
        ids.sort();
        Ok(ids)
    }
}
