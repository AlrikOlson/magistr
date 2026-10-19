use std::io::Write;

use flate2::{Compression, write::GzEncoder};
use serde_json::json;

use super::{DoctorRun, DoctorSectionId, redact_bundle_text, tar_writer};
use crate::response::Redactor;

/// The support bundle: the report's sections as machine-readable members
/// plus a manifest, every member passing the redaction choke point and the
/// credential redactor, packed as tar.gz by the in-tree ustar writer.
/// Structure and symptoms, never repo file contents — the probes never read
/// repo files, and redaction is defense in depth on top of that guarantee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorBundle {
    pub bytes: Vec<u8>,
    pub members: Vec<String>,
}

impl DoctorBundle {
    pub const FORMAT_VERSION: u32 = 1;
    pub const PREFIX: &'static str = "workspace-doctor";

    pub fn build(run: &DoctorRun) -> std::io::Result<Self> {
        let report = &run.report;
        let mut bodies: Vec<(String, String)> = vec![
            ("report.txt".into(), run.render()),
            ("config.txt".into(), run.config_text.clone()),
        ];
        for section in &report.sections {
            if section.id == DoctorSectionId::Config {
                continue; // config.txt carries the provenance text
            }
            let body = serde_json::to_string_pretty(&json!({
                "source": section.source,
                "findings": section.findings,
                "detail": section.detail,
            }))
            .map_err(std::io::Error::other)?;
            bodies.push((format!("{}.json", section.id.as_str()), body));
        }
        let member_names: Vec<String> = bodies
            .iter()
            .map(|(name, _)| format!("{}/{name}", Self::PREFIX))
            .collect();
        let mut members = vec![format!("{}/manifest.json", Self::PREFIX)];
        members.extend(member_names);
        let manifest = serde_json::to_string_pretty(&json!({
            "bundleFormatVersion": Self::FORMAT_VERSION,
            "toolVersion": report.tool_version,
            "createdAt": report.generated_at,
            "workspaceLabel": "<workspace>",
            "members": members,
        }))
        .map_err(std::io::Error::other)?;
        let mut tar_members = vec![tar_writer::TarMember {
            name: members[0].clone(),
            data: manifest.into_bytes(),
        }];
        for (name, body) in bodies {
            let redacted =
                Redactor::redact(&redact_bundle_text(&body, &report.workspace_root)).text;
            tar_members.push(tar_writer::TarMember {
                name: format!("{}/{name}", Self::PREFIX),
                data: redacted.into_bytes(),
            });
        }
        let mtime_seconds = run_mtime_seconds(&report.generated_at);
        let archive = tar_writer::tar_archive(&tar_members, mtime_seconds)?;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&archive)?;
        Ok(Self {
            bytes: encoder.finish()?,
            members,
        })
    }
}

/// The report's own timestamp as tar mtime; a malformed stamp yields the
/// epoch rather than a failed bundle.
fn run_mtime_seconds(generated_at: &str) -> u64 {
    let digits = |from: usize, len: usize| -> u64 {
        generated_at
            .get(from..from + len)
            .and_then(|slice| slice.parse::<u64>().ok())
            .unwrap_or(0)
    };
    let (year, month, day) = (digits(0, 4), digits(5, 2), digits(8, 2));
    let (hours, minutes, seconds) = (digits(11, 2), digits(14, 2), digits(17, 2));
    if year < 1970 || !(1..=12).contains(&month) || day == 0 {
        return 0;
    }
    // Days from civil (Howard Hinnant), inverse of the report's formatter.
    let (year, month) = if month <= 2 {
        (year - 1, month + 9)
    } else {
        (year, month - 3)
    };
    let era = year / 400;
    let yoe = year - era * 400;
    let doy = (153 * month + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    days * 86_400 + hours * 3600 + minutes * 60 + seconds
}

#[cfg(test)]
#[path = "doctor_bundle.tests.rs"]
mod tests;
