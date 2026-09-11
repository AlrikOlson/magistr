use super::{
    DoctorOptions, DoctorReport, DoctorSectionId, DoctorSeverity, doctor_probes, utc_timestamp,
};

/// One doctor run: every probe composed into the report plus the config
/// provenance text embedded verbatim. Read-only by construction — no probe
/// repairs, takes a lock, or creates a file; a missing or unreadable store
/// is a finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorRun {
    pub report: DoctorReport,
    /// The `doctor config` text, embedded verbatim in the report.
    pub config_text: String,
}

impl DoctorRun {
    #[must_use]
    pub fn observe(options: &DoctorOptions) -> Self {
        let (config, config_text) = doctor_probes::config(options);
        let (quarantine, entries) = doctor_probes::quarantine(&options.state_dir);
        let sections = vec![
            doctor_probes::environment(options),
            doctor_probes::index(&options.state_dir),
            config,
            doctor_probes::plugins(&options.root, &options.state_dir),
            quarantine,
            doctor_probes::crashes(&entries),
        ];
        let findings_present = sections.iter().any(|section| {
            section
                .findings
                .iter()
                .any(|finding| finding.severity == DoctorSeverity::Unhealthy)
        });
        Self {
            report: DoctorReport {
                workspace_root: options.root.display().to_string(),
                generated_at: utc_timestamp(options.now_unix_ms),
                tool_version: options.tool_version.clone(),
                sections,
                findings_present,
            },
            config_text,
        }
    }

    /// Sections in contract order, a `source:` line on every section, a
    /// `remediation:` line on every unhealthy finding, the config text
    /// embedded rather than forked.
    #[must_use]
    pub fn render(&self) -> String {
        let report = &self.report;
        let mut lines = vec![
            format!("workbench doctor — workspace {}", report.workspace_root),
            format!(
                "generated {} by magistr-workspace {}",
                report.generated_at, report.tool_version
            ),
        ];
        for section in &report.sections {
            lines.push(String::new());
            lines.push(format!("[{}]", section.id.as_str()));
            lines.push(format!("  source: {}", section.source));
            for finding in &section.findings {
                lines.push(format!(
                    "  {} {}: {}",
                    finding.severity.mark(),
                    finding.store,
                    finding.symptom
                ));
                if let Some(remediation) = &finding.remediation {
                    lines.push(format!("     remediation: {remediation}"));
                }
            }
            if section.id == DoctorSectionId::Config && !self.config_text.is_empty() {
                lines.extend(self.config_text.split('\n').map(|line| format!("  {line}")));
            }
        }
        lines.push(String::new());
        lines.push(
            if report.findings_present {
                "result: findings present (exit 1)"
            } else {
                "result: all probes healthy (exit 0)"
            }
            .into(),
        );
        lines.join("\n")
    }
}

#[cfg(test)]
#[path = "doctor_run.tests.rs"]
mod tests;
