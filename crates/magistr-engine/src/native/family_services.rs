//! The four magistr family services, composed once.

use std::sync::{Arc, Mutex};

use magistr_workflow::decision::DecisionService;
use magistr_workflow::decision::application::DecisionUseCases;
use magistr_workflow::decision::engine::ReasoningServer;
use magistr_workflow::mcp::UnifiedService;
use magistr_workflow::plan::application::PlanUseCases;
use magistr_workflow::plan::{PlanEngine, PlanService};
use magistr_workflow::request::application::RequestUseCases;
use magistr_workflow::request::{RequestEngine, RequestService};
use magistr_workflow::run::RunService;
use magistr_workflow::run::application::EvidencePort;
use magistr_workflow::run::application::RunUseCases;
use magistr_workflow::run::engine::RunEngine;

/// An engine as its service holds it: shared, locked per call.
type Shared<T> = Arc<Mutex<T>>;

/// Native use cases composed once under project admission. Transports are
/// disposable views over these capabilities, with no separate workflow state.
#[derive(Clone)]
pub(crate) struct FamilyServices {
    pub(crate) decision: DecisionUseCases,
    pub(crate) run: RunUseCases,
    pub(crate) plan: PlanUseCases,
    pub(crate) request: RequestUseCases,
}

impl FamilyServices {
    /// Wrap freshly opened engines in their services and wire the
    /// reciprocal handles: plan ↔ request, and run as the plan's
    /// completion authority, so a task completes only on the run's
    /// current receipts whichever adapter asks. The host's evidence
    /// capability, when composed, is what lets those receipts certify a
    /// source; without it the run family refuses every completion.
    pub(crate) fn compose(
        decision: ReasoningServer,
        run: RunEngine,
        plan: PlanEngine,
        request: RequestEngine,
        evidence: Option<Arc<dyn EvidencePort>>,
    ) -> Self {
        let decision = DecisionUseCases::new(Arc::new(Mutex::new(decision)));
        let mut run = RunUseCases::new(Arc::new(Mutex::new(run)));
        if let Some(evidence) = evidence {
            run = run.with_evidence(evidence);
        }
        let plan = PlanUseCases::new(Arc::new(Mutex::new(plan)), run.clone());
        let request = RequestUseCases::new(Arc::new(Mutex::new(request))).with_plan(plan.engine());
        let plan = plan.with_request(request.engine());
        Self {
            decision,
            run,
            plan,
            request,
        }
    }

    pub(crate) fn for_execution(
        &self,
        engine: RunEngine,
        evidence: Option<Arc<dyn EvidencePort>>,
    ) -> Self {
        let mut run = RunUseCases::new(Arc::new(Mutex::new(engine)));
        if let Some(evidence) = evidence {
            run = run.with_evidence(evidence);
        }
        let plan =
            PlanUseCases::new(self.plan.engine(), run.clone()).with_request(self.request.engine());
        Self {
            decision: self.decision.clone(),
            run,
            plan,
            request: self.request.clone(),
        }
    }

    pub(crate) fn bind_feed(
        &self,
        feed: &magistr_workflow::infra::Broadcaster,
        execution_key: &str,
    ) -> std::io::Result<()> {
        let (decision, run, plan, request) = self.engines();
        decision
            .lock()
            .map_err(|_| std::io::Error::other("The reasoning publisher could not be attached."))?
            .set_broadcaster(
                magistr_workflow::decision::broadcast::Broadcaster::from_engine(feed.clone()),
            );
        run.lock()
            .map_err(|_| std::io::Error::other("The task publisher could not be attached."))?
            .set_broadcaster(magistr_workflow::run::broadcast::Broadcaster::from_engine(
                feed.clone().with_execution(execution_key.to_owned()),
            ));
        plan.lock()
            .map_err(|_| std::io::Error::other("The plan publisher could not be attached."))?
            .set_broadcaster(magistr_workflow::plan::broadcast::Broadcaster::from_engine(
                feed.clone(),
            ));
        request
            .lock()
            .map_err(|_| std::io::Error::other("The request publisher could not be attached."))?
            .set_broadcaster(
                magistr_workflow::request::broadcast::Broadcaster::from_engine(feed.clone()),
            );
        Ok(())
    }

    /// The MCP transport's view of the same services.
    pub(crate) fn unified(&self) -> UnifiedService {
        UnifiedService::new(
            DecisionService::from_use_cases(self.decision.clone()),
            RunService::from_use_cases(self.run.clone()),
            PlanService::from_use_cases(self.plan.clone()),
            RequestService::from_use_cases(self.request.clone()),
        )
    }

    /// The shared engine handles the services act on — what the reader
    /// queries, so reads and native mutations share the same state.
    pub(crate) fn engines(
        &self,
    ) -> (
        Shared<ReasoningServer>,
        Shared<RunEngine>,
        Shared<PlanEngine>,
        Shared<RequestEngine>,
    ) {
        (
            self.decision.engine(),
            self.run.engine(),
            self.plan.engine(),
            self.request.engine(),
        )
    }
}
