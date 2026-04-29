use crate::layout::{ForgeError, SelfForge, ValidationReport};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Runtime {
    forge: SelfForge,
}

impl Runtime {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            forge: SelfForge::new(root),
        }
    }

    pub fn verify_layout(&self) -> Result<ValidationReport, ForgeError> {
        self.forge.validate()
    }
}
