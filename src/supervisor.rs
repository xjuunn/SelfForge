use crate::evolution::{EvolutionEngine, EvolutionError, EvolutionReport};
use crate::layout::{BootstrapReport, ForgeError, SelfForge, ValidationReport};
use crate::runtime::Runtime;
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

    pub fn prepare_next_version(&self, goal: &str) -> Result<EvolutionReport, EvolutionError> {
        EvolutionEngine::new(self.forge.root()).prepare_next_version(goal)
    }
}
