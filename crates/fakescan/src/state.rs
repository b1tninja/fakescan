use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::image_gen;
use crate::profile::Profile;

#[derive(Debug, Clone)]
pub struct ScanJob {
    pub id: i32,
    pub token: String,
    pub format: String,
    pub input_source: String,
    pub dpi: u32,
    pub width_px: u32,
    pub height_px: u32,
    pub pages_left: u32,
    pub page_index: u32,
}

#[derive(Debug)]
pub struct World {
    profiles_dir: PathBuf,
    profile: Profile,
    feeder_ready: bool,
    /// Synthetic pages available in the feeder (0 = empty / glass-only).
    pages_remaining: u32,
    next_job_id: i32,
    jobs: HashMap<i32, ScanJob>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub profile_id: String,
    pub connection: String,
    pub feeder_ready: bool,
    pub sticky_ready: bool,
    pub pages_remaining: u32,
    pub active_jobs: usize,
    pub optical: Option<crate::profile::Optical>,
    pub dpi: Vec<u32>,
    pub formats: Vec<String>,
    pub available_profiles: Vec<String>,
    pub note: &'static str,
}

#[derive(Debug, Deserialize)]
pub struct FeederBody {
    pub ready: bool,
}

#[derive(Debug, Deserialize)]
pub struct PagesBody {
    pub count: u32,
}

impl World {
    pub fn new(profiles_dir: PathBuf, profile: Profile) -> Self {
        let sticky = profile.feeder.sticky_ready;
        Self {
            profiles_dir,
            profile,
            feeder_ready: sticky,
            pages_remaining: 0,
            next_job_id: 1,
            jobs: HashMap::new(),
        }
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    pub fn feeder_ready(&self) -> bool {
        self.feeder_ready
    }

    pub fn pages_remaining(&self) -> u32 {
        self.pages_remaining
    }

    pub fn status(&self) -> Status {
        let available_profiles = Profile::list(&self.profiles_dir).unwrap_or_default();
        Status {
            profile_id: self.profile.id.clone(),
            connection: self.profile.connection.clone(),
            feeder_ready: self.feeder_ready,
            sticky_ready: self.profile.feeder.sticky_ready,
            pages_remaining: self.pages_remaining,
            active_jobs: self.jobs.len(),
            optical: self.profile.optical.clone(),
            dpi: self.profile.dpi.clone(),
            formats: self.profile.formats.clone(),
            available_profiles,
            note: "WS-Scan GetScannerElements / CreateScanJob / RetrieveImage implemented",
        }
    }

    pub fn load_profile(&mut self, id: &str) -> Result<()> {
        let profile = Profile::load(&self.profiles_dir, id)?;
        let sticky = profile.feeder.sticky_ready;
        self.profile = profile;
        self.feeder_ready = sticky;
        self.pages_remaining = 0;
        self.jobs.clear();
        Ok(())
    }

    pub fn set_feeder(&mut self, ready: bool) {
        self.feeder_ready = ready;
    }

    pub fn set_pages(&mut self, count: u32) {
        let max = self.profile.feeder.pages_max;
        self.pages_remaining = count.min(max);
        if self.pages_remaining > 0 {
            self.feeder_ready = true;
        } else if !self.profile.feeder.sticky_ready {
            self.feeder_ready = false;
        }
    }

    pub fn reset(&mut self) {
        self.feeder_ready = self.profile.feeder.sticky_ready;
        self.pages_remaining = 0;
        self.jobs.clear();
    }

    /// Create a scan job from a CreateScanJobRequest body (raw SOAP XML).
    pub fn create_job(&mut self, body: &str) -> Result<ScanJob> {
        let input_source = extract_tag(body, "InputSource").unwrap_or_else(|| {
            if self.feeder_ready && self.pages_remaining > 0 {
                "ADF".into()
            } else {
                "Platen".into()
            }
        });
        let dpi = extract_resolution(body)
            .or_else(|| self.profile.dpi.first().copied())
            .unwrap_or(200);
        let format = map_format(
            &extract_tag(body, "Format").unwrap_or_else(|| preferred_format(&self.profile)),
            &self.profile,
        )?;

        let pages = if input_source.eq_ignore_ascii_case("ADF")
            || input_source.eq_ignore_ascii_case("Feeder")
        {
            if self.pages_remaining == 0 && !self.feeder_ready {
                bail!("ADF selected but feeder empty");
            }
            // Consume the feeder stack into this job.
            let n = self.pages_remaining.max(1);
            self.pages_remaining = 0;
            if !self.profile.feeder.sticky_ready {
                self.feeder_ready = false;
            }
            n
        } else {
            1
        };

        let (width_px, height_px) = image_gen::page_pixels(dpi);
        let id = self.next_job_id;
        self.next_job_id += 1;
        let job = ScanJob {
            id,
            token: format!("job-{}", Uuid::new_v4().simple()),
            format,
            input_source,
            dpi,
            width_px,
            height_px,
            pages_left: pages,
            page_index: 0,
        };
        self.jobs.insert(id, job.clone());
        Ok(job)
    }

    pub fn cancel_job(&mut self, job_id: i32) -> Result<()> {
        if self.jobs.remove(&job_id).is_none() {
            bail!("job {job_id} not found");
        }
        Ok(())
    }

    /// Pull one page image for RetrieveImage.
    pub fn retrieve_image(&mut self, job_id: i32, token: &str) -> Result<(ScanJob, Vec<u8>)> {
        let job = self
            .jobs
            .get_mut(&job_id)
            .ok_or_else(|| anyhow::anyhow!("job {job_id} not found"))?;
        if job.token != token {
            bail!("job token mismatch");
        }
        if job.pages_left == 0 {
            bail!("no images left on job {job_id}");
        }
        let bytes = image_gen::jpeg_page(job.dpi, job.page_index)?;
        job.page_index += 1;
        job.pages_left -= 1;
        let snapshot = job.clone();
        if snapshot.pages_left == 0 {
            self.jobs.remove(&job_id);
        }
        Ok((snapshot, bytes))
    }
}

fn preferred_format(profile: &Profile) -> String {
    profile
        .formats
        .first()
        .cloned()
        .unwrap_or_else(|| "jpeg".into())
}

fn map_format(requested: &str, profile: &Profile) -> Result<String> {
    let requested_lc = requested.to_ascii_lowercase();
    let wsd = match requested_lc.as_str() {
        "jfif" | "jpeg" | "jpg" => "jfif",
        "tiff-single-uncompressed" | "tiff" | "tif" => "tiff-single-uncompressed",
        "dib" | "bmp" => "dib",
        "png" => "png",
        other => other,
    };
    let local = match wsd {
        "jfif" => "jpeg",
        "tiff-single-uncompressed" => "tiff",
        "dib" => "bmp",
        other => other,
    };
    if profile
        .formats
        .iter()
        .any(|f| f.eq_ignore_ascii_case(local))
        || profile.formats.iter().any(|f| f.eq_ignore_ascii_case(wsd))
    {
        return Ok(wsd.into());
    }
    // Prefer first profile format mapped to WSD.
    Ok(match preferred_format(profile).as_str() {
        "jpeg" => "jfif".into(),
        "tiff" => "tiff-single-uncompressed".into(),
        "bmp" => "dib".into(),
        other => other.into(),
    })
}

pub(crate) fn extract_tag(xml: &str, local: &str) -> Option<String> {
    // Match `<prefix:local>` or `<local>` (not attributes / closing tags).
    let mut idx = 0;
    while let Some(rel) = xml[idx..].find(local) {
        let at = idx + rel;
        let before = xml.as_bytes().get(at.wrapping_sub(1)).copied();
        let after = xml.get(at + local.len()..).unwrap_or("");
        if matches!(before, Some(b'<') | Some(b':')) && after.starts_with('>') {
            let value_start = &after[1..];
            if let Some(end) = value_start.find('<') {
                let value = value_start[..end].trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
        idx = at + local.len();
    }
    None
}

fn extract_resolution(xml: &str) -> Option<u32> {
    // Prefer Width inside Resolution.
    if let Some(res) = xml.find("Resolution") {
        let slice = &xml[res..].get(..400).unwrap_or(&xml[res..]);
        if let Some(w) = extract_tag(slice, "Width") {
            return w.trim().parse().ok();
        }
    }
    extract_tag(xml, "Width")?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_input_source() {
        let xml = r#"<wscn:InputSource>Platen</wscn:InputSource>"#;
        assert_eq!(extract_tag(xml, "InputSource").as_deref(), Some("Platen"));
    }
}
