use super::{WorkerError, WorkerRequest, WorkerResponse};
use crate::engine::{self, CoreError};

impl WorkerRequest {
    /// Execute only in the native worker process after protocol admission.
    pub(crate) fn execute(self) -> Result<WorkerResponse, WorkerError> {
        match self {
            Self::Ping => {
                let report = engine::self_test();
                if !report.all_ok() {
                    return Err(WorkerError::Unavailable(format!(
                        "native engine self-test failed: {}",
                        report.details.join("; ")
                    )));
                }
                Ok(WorkerResponse::Pong)
            }
            Self::Parse {
                language, source, ..
            } => {
                let summary = engine::parse(&language, &source).map_err(core_error)?;
                Ok(WorkerResponse::Parse {
                    language: summary.language,
                    root_kind: summary.root_kind,
                    node_count: summary.node_count,
                    error_count: summary.error_count,
                    has_error: summary.has_error,
                })
            }
            Self::ValidateData { format, source, .. } => Ok(WorkerResponse::DataValidated {
                diagnostic: super::data_validation::validate_data(format, &source),
            }),
            Self::Search {
                pattern,
                haystack,
                case_insensitive,
                ..
            } => {
                let hits =
                    engine::search(&pattern, &haystack, case_insensitive).map_err(core_error)?;
                Ok(WorkerResponse::Search {
                    hits: hits.into_iter().map(Into::into).collect(),
                })
            }
            Self::Structural {
                language,
                source,
                pattern,
                ..
            } => {
                let hits =
                    engine::structural_search(&language, &source, &pattern).map_err(core_error)?;
                Ok(WorkerResponse::Structural {
                    hits: hits.into_iter().map(Into::into).collect(),
                })
            }
            Self::Rewrite {
                language,
                source,
                pattern,
                replacement,
                ..
            } => {
                let outcome = engine::rewrite(&language, &source, &pattern, &replacement)
                    .map_err(core_error)?;
                Ok(WorkerResponse::Rewrite {
                    output: outcome.output,
                    match_count: outcome.match_count,
                })
            }
            Self::Extract {
                language, source, ..
            } => {
                let extraction = crate::indexer::extract(&language, &source).map_err(core_error)?;
                Ok(WorkerResponse::Extract {
                    symbols: extraction.symbols.into_iter().map(Into::into).collect(),
                    imports: extraction.imports,
                    test_ranges: extraction.test_ranges.into_iter().map(Into::into).collect(),
                })
            }
            #[cfg(any(feature = "sandbox", feature = "lang-pack"))]
            request => request.execute_extensions(),
        }
    }

    #[cfg(feature = "sandbox")]
    fn execute_sandbox(self) -> Result<WorkerResponse, WorkerError> {
        match self {
            #[cfg(feature = "sandbox")]
            Self::Component {
                guest,
                export,
                args,
                capabilities,
                policy,
            } => {
                let outcome = crate::sandbox::run_component(
                    &guest,
                    &export,
                    &args,
                    &capabilities,
                    policy.admitted()?,
                )
                .map_err(sandbox_error)?;
                Ok(WorkerResponse::Component {
                    end: outcome.end.into(),
                    fuel_used: outcome.fuel_used,
                    results: outcome.results,
                    observations: outcome.observations,
                })
            }
            #[cfg(feature = "sandbox")]
            Self::Wasi {
                guest,
                argv,
                policy,
            } => {
                let outcome = crate::sandbox::run_wasi_module(&guest, &argv, policy.admitted()?)
                    .map_err(sandbox_error)?;
                Ok(WorkerResponse::Wasi {
                    end: outcome.end.into(),
                    fuel_used: outcome.fuel_used,
                    exit_code: outcome.exit_code,
                    stdout: outcome.stdout,
                    stderr: outcome.stderr,
                })
            }
            #[cfg(feature = "sandbox")]
            Self::InvokeComponent {
                guest,
                export,
                payload,
                capabilities,
                policy,
                subprocess_specs,
                subprocess_root,
            } => {
                let specs: Vec<_> = subprocess_specs.into_iter().map(Into::into).collect();
                let outcome = crate::sandbox::invoke_component_json(
                    &guest,
                    &export,
                    &payload,
                    &capabilities,
                    policy.admitted()?,
                    &specs,
                    &subprocess_root,
                )
                .map_err(sandbox_error)?;
                Ok(WorkerResponse::InvokeComponent {
                    end: outcome.end.into(),
                    fuel_used: outcome.fuel_used,
                    result: outcome.result,
                })
            }
            #[cfg(feature = "sandbox")]
            Self::Resolver {
                guest,
                symbol,
                capabilities,
                policy,
                subprocess_specs,
                subprocess_root,
            } => {
                let specs: Vec<_> = subprocess_specs.into_iter().map(Into::into).collect();
                let outcome = crate::sandbox::invoke_resolver_references(
                    &guest,
                    &symbol,
                    &capabilities,
                    policy.admitted()?,
                    &specs,
                    &subprocess_root,
                )
                .map_err(sandbox_error)?;
                Ok(WorkerResponse::Resolver {
                    end: outcome.end.into(),
                    fuel_used: outcome.fuel_used,
                    locations: outcome.locations.into_iter().map(Into::into).collect(),
                    sessions_opened: outcome.sessions_opened,
                    sessions_reused: outcome.sessions_reused,
                })
            }
            _ => Err(WorkerError::Protocol(
                "unexpected sandbox dispatch".to_owned(),
            )),
        }
    }

    #[cfg(any(feature = "sandbox", feature = "lang-pack"))]
    fn execute_extensions(self) -> Result<WorkerResponse, WorkerError> {
        match self {
            #[cfg(feature = "lang-pack")]
            Self::RegisterLanguage {
                id,
                extensions,
                grammar_wasm,
                tag_queries,
                blessed,
            } => {
                let registration = crate::lang_registry::register(
                    &id,
                    &extensions,
                    &grammar_wasm,
                    &tag_queries,
                    blessed,
                )
                .map_err(core_error)?;
                Ok(WorkerResponse::LanguageRegistered {
                    registration: registration.into(),
                })
            }
            #[cfg(all(feature = "sandbox", feature = "lang-pack"))]
            Self::LoadLanguagePack {
                guest,
                capabilities,
                policy,
                blessed,
            } => {
                let outcome = crate::sandbox::invoke_lang_pack_load(
                    &guest,
                    &capabilities,
                    policy.admitted()?,
                )
                .map_err(sandbox_error)?;
                let registration = outcome
                    .bundle
                    .map(|bundle| {
                        crate::lang_registry::register(
                            &bundle.name,
                            &bundle.extensions,
                            &bundle.grammar_wasm,
                            &bundle.tag_queries,
                            blessed,
                        )
                        .map(Into::into)
                    })
                    .transpose()
                    .map_err(core_error)?;
                Ok(WorkerResponse::LanguagePackLoaded {
                    end: outcome.end.into(),
                    fuel_used: outcome.fuel_used,
                    registration,
                })
            }
            #[cfg(feature = "sandbox")]
            Self::ClassifyLanguagePack {
                guest,
                paths,
                capabilities,
                policy,
            } => {
                let outcome = crate::sandbox::invoke_lang_pack_classify(
                    &guest,
                    &paths,
                    &capabilities,
                    policy.admitted()?,
                )
                .map_err(sandbox_error)?;
                Ok(WorkerResponse::LanguagePackClassified {
                    end: outcome.end.into(),
                    fuel_used: outcome.fuel_used,
                    classes: outcome.classes,
                })
            }
            #[cfg(feature = "sandbox")]
            request => request.execute_sandbox(),
            #[cfg(not(feature = "sandbox"))]
            _ => Err(WorkerError::Protocol(
                "unexpected extension dispatch".to_owned(),
            )),
        }
    }
}

fn core_error(error: CoreError) -> WorkerError {
    let code = match &error {
        CoreError::UnknownLanguage { .. } | CoreError::LanguageRejected { .. } => {
            "LANGUAGE_UNAVAILABLE"
        }
        CoreError::ParseFailed { .. } => "PARSE_REJECTED",
        CoreError::BadSearchPattern { .. }
        | CoreError::BadRewritePattern { .. }
        | CoreError::BadStructuralPattern { .. } => "PATTERN_INVALID",
        CoreError::SearchFailed { .. }
        | CoreError::NonUtf8Output
        | CoreError::ScanFailed { .. } => "ENGINE_UNAVAILABLE",
    };
    WorkerError::Remote {
        code: code.to_owned(),
        message: error.to_string(),
    }
}

#[cfg(feature = "sandbox")]
fn sandbox_error(error: crate::sandbox::SandboxError) -> WorkerError {
    WorkerError::Remote {
        code: "SANDBOX_UNAVAILABLE".to_owned(),
        message: error.to_string(),
    }
}

#[cfg(test)]
#[path = "dispatch.tests.rs"]
mod tests;
