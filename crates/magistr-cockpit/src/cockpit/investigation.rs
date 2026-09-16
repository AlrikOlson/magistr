//! Recorded side investigations without range-based membership or invented results.

use super::Trace;

/// One question's recorded decisions and the evidence connecting it to other work.
#[derive(Clone, Debug, PartialEq)]
pub struct Investigation<'a> {
    pub id: &'a str,
    pub question: &'a str,
    pub outcome: Option<&'a str>,
    pub status: &'a str,
    pub from: Option<u32>,
    pub members: Vec<u32>,
    pub indices: Vec<usize>,
    /// External decisions with an explicit non-refuting dependency on a member.
    pub used_by: Vec<u32>,
    /// Existing merge inference: first later main-line step, not exact provenance.
    pub merged_near: Option<u32>,
    pub lane: u8,
}

impl<'a> Investigation<'a> {
    #[must_use]
    pub fn all(trace: &'a Trace) -> Vec<Self> {
        let mut investigations = Vec::new();
        let merges = trace.merges();
        let lanes = trace.lanes();
        for (index, first) in trace.steps().iter().enumerate() {
            let Some(id) = first.branch_id.as_deref() else {
                continue;
            };
            if investigations.iter().any(|known: &Self| known.id == id) {
                continue;
            }
            let indices: Vec<_> = trace
                .steps()
                .iter()
                .enumerate()
                .filter_map(|(i, step)| (step.branch_id.as_deref() == Some(id)).then_some(i))
                .collect();
            let members: Vec<_> = indices.iter().map(|&i| trace.steps()[i].number).collect();
            let question = indices
                .iter()
                .filter_map(|&i| trace.steps()[i].branch_name.as_deref())
                .find(|name| !name.trim().is_empty())
                .or_else(|| (!first.purpose.trim().is_empty()).then_some(first.purpose.as_str()))
                .unwrap_or("Question not recorded");
            let outcome = indices
                .iter()
                .rev()
                .map(|&i| trace.steps()[i].outcome.as_str())
                .find(|text| !text.trim().is_empty());
            let from = indices.iter().find_map(|&i| trace.steps()[i].branch_from);
            let used_by = trace
                .steps()
                .iter()
                .filter(|step| step.branch_id.as_deref() != Some(id))
                .filter(|step| {
                    step.dependencies
                        .iter()
                        .any(|n| members.contains(n) && !step.refutes.contains(n))
                })
                .map(|step| step.number)
                .collect();
            let merged_near = merges
                .iter()
                .find(|(last, _)| indices.last() == Some(last))
                .map(|(_, next)| trace.steps()[*next].number);
            investigations.push(Self {
                id,
                question,
                outcome,
                status: trace.branch_status(id),
                from,
                members,
                indices,
                used_by,
                merged_near,
                lane: lanes.get(index).map_or(0, |(_, lane)| *lane),
            });
        }
        investigations
    }

    /// Only recorded membership and edges associate a selection with a question.
    #[must_use]
    pub fn contains(&self, number: u32) -> bool {
        self.members.contains(&number)
            || self.from == Some(number)
            || self.used_by.contains(&number)
            || self.merged_near == Some(number)
    }

    #[must_use]
    pub fn status_label(&self) -> &'static str {
        match self.status {
            "merged" => "Findings used",
            "abandoned" => "Investigation stopped",
            _ => "Still exploring",
        }
    }

    /// Exact member numbers. A range is only shorthand for contiguous numbers.
    #[must_use]
    pub fn member_label(&self) -> String {
        match self.members.as_slice() {
            [] => String::new(),
            [one] => format!("{one:02}"),
            numbers
                if numbers
                    .windows(2)
                    .all(|pair| pair[0].checked_add(1) == Some(pair[1])) =>
            {
                format!("{:02}–{:02}", numbers[0], numbers[numbers.len() - 1])
            }
            numbers => numbers
                .iter()
                .map(|n| format!("{n:02}"))
                .collect::<Vec<_>>()
                .join(", "),
        }
    }

    #[must_use]
    pub fn relationship(&self) -> String {
        let mut parts = Vec::new();
        if let Some(from) = self.from {
            parts.push(format!("Explored after {from:02}"));
        }
        if !self.used_by.is_empty() {
            parts.push(format!(
                "answer used in {}",
                self.used_by
                    .iter()
                    .map(|n| format!("{n:02}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        } else if let Some(next) = self.merged_near {
            parts.push(format!("findings included near {next:02}"));
        } else {
            parts.push(self.status_label().to_owned());
        }
        parts.join(" · ")
    }
}

#[cfg(test)]
#[path = "investigation.tests.rs"]
mod tests;
