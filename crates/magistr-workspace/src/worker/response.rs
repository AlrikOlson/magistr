use serde::{Deserialize, Serialize};

#[cfg(feature = "lang-pack")]
use super::WorkerLanguageRegistration;
#[cfg(feature = "sandbox")]
use super::{WorkerGuestEnd, WorkerLocation, WorkerRankingFeature, WorkerRunReport};
use super::{WorkerSearchHit, WorkerStructuralHit, WorkerSymbol, WorkerTestRange};

/// Owned typed results crossing the process boundary without native pointers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum WorkerResponse {
    Pong,
    Parse {
        language: String,
        root_kind: String,
        node_count: u32,
        error_count: u32,
        has_error: bool,
    },
    DataValidated {
        diagnostic: Option<String>,
    },
    Search {
        hits: Vec<WorkerSearchHit>,
    },
    Structural {
        hits: Vec<WorkerStructuralHit>,
    },
    Rewrite {
        output: String,
        match_count: u32,
    },
    Extract {
        symbols: Vec<WorkerSymbol>,
        imports: Vec<String>,
        test_ranges: Vec<WorkerTestRange>,
    },
    #[cfg(feature = "sandbox")]
    Component {
        end: WorkerGuestEnd,
        fuel_used: u64,
        results: Vec<String>,
        observations: Vec<u32>,
    },
    #[cfg(feature = "sandbox")]
    Wasi {
        end: WorkerGuestEnd,
        fuel_used: u64,
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[cfg(feature = "sandbox")]
    InvokeComponent {
        end: WorkerGuestEnd,
        fuel_used: u64,
        result: Option<String>,
    },
    #[cfg(feature = "sandbox")]
    Resolver {
        end: WorkerGuestEnd,
        fuel_used: u64,
        locations: Vec<WorkerLocation>,
        sessions_opened: u32,
        sessions_reused: u32,
    },
    #[cfg(feature = "lang-pack")]
    LanguageRegistered {
        registration: WorkerLanguageRegistration,
    },
    #[cfg(all(feature = "sandbox", feature = "lang-pack"))]
    LanguagePackLoaded {
        end: WorkerGuestEnd,
        fuel_used: u64,
        registration: Option<WorkerLanguageRegistration>,
    },
    #[cfg(feature = "sandbox")]
    LanguagePackClassified {
        end: WorkerGuestEnd,
        fuel_used: u64,
        classes: Vec<Option<String>>,
    },
    #[cfg(feature = "sandbox")]
    RankingContributed {
        end: WorkerGuestEnd,
        fuel_used: u64,
        /// Parallel to the input batch; shorter on a mid-batch failure.
        contributions: Vec<Vec<WorkerRankingFeature>>,
    },
    #[cfg(feature = "sandbox")]
    RunnerParsed {
        end: WorkerGuestEnd,
        fuel_used: u64,
        claimed: bool,
        report: Option<WorkerRunReport>,
    },
}
