use crate::evolution::{
    CycleReport, EvolutionEngine, EvolutionError, EvolutionReport, PromotionReport, RollbackReport,
};
use crate::layout::{BootstrapReport, ForgeError, SelfForge, ValidationReport};
use crate::runtime::{ExecutionError, ExecutionReport, RunIndexEntry, Runtime};
use crate::version::VersionBump;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Supervisor {
    forge: SelfForge,
    runtime: Runtime,
}

impl Supervisor {
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref();
        Self {
            forge: SelfForge::new(root),
            runtime: Runtime::new(root),
        }
    }

    pub fn initialize_current_version(&self) -> Result<BootstrapReport, ForgeError> {
        self.forge.bootstrap()
    }

    pub fn verify_current_version(&self) -> Result<ValidationReport, ForgeError> {
        self.runtime.verify_layout()
    }

    pub fn verify_version(&self, version: impl AsRef<str>) -> Result<ValidationReport, ForgeError> {
        self.runtime.verify_layout_for_version(version)
    }

    pub fn execute_in_workspace(
        &self,
        version: impl AsRef<str>,
        program: impl AsRef<str>,
        args: &[String],
        timeout_ms: u64,
    ) -> Result<ExecutionReport, ExecutionError> {
        self.runtime
            .execute_in_workspace(version, program, args, timeout_ms)
    }

    pub fn list_runs(
        &self,
        version: impl AsRef<str>,
        limit: usize,
    ) -> Result<Vec<RunIndexEntry>, ExecutionError> {
        self.runtime.list_runs(version, limit)
    }

    pub fn prepare_next_version(&self, goal: &str) -> Result<EvolutionReport, EvolutionError> {
        EvolutionEngine::new(self.forge.root()).prepare_next_version(goal)
    }

    pub fn prepare_next_version_with_bump(
        &self,
        goal: &str,
        bump: VersionBump,
    ) -> Result<EvolutionReport, EvolutionError> {
        EvolutionEngine::new(self.forge.root()).prepare_next_version_with_bump(goal, bump)
    }

    pub fn promote_candidate(&self) -> Result<PromotionReport, EvolutionError> {
        EvolutionEngine::new(self.forge.root()).promote_candidate()
    }

    pub fn rollback_candidate(&self, reason: &str) -> Result<RollbackReport, EvolutionError> {
        EvolutionEngine::new(self.forge.root()).rollback_candidate(reason)
    }

    pub fn run_candidate_cycle(&self) -> Result<CycleReport, EvolutionError> {
        EvolutionEngine::new(self.forge.root()).run_candidate_cycle()
    }
}
