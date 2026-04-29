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
}
