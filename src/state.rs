use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForgeState {
    pub current_version: String,
    pub parent_version: Option<String>,
    pub status: String,
    pub workspace: String,
    pub last_verified: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_workspace: Option<String>,
}

#[derive(Debug)]
pub enum StateError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    Serialize {
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl ForgeState {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, StateError> {
        let path = state_path(root.as_ref());
        let contents = fs::read_to_string(&path).map_err(|source| StateError::Io {
            path: path.clone(),
            source,
        })?;
        serde_json::from_str(&contents).map_err(|source| StateError::Parse { path, source })
    }

    pub fn save(&self, root: impl AsRef<Path>) -> Result<(), StateError> {
        let path = state_path(root.as_ref());
        let contents =
            serde_json::to_string_pretty(self).map_err(|source| StateError::Serialize {
                path: path.clone(),
                source,
            })? + "\n";

        fs::write(&path, contents).map_err(|source| StateError::Io { path, source })
    }
}

impl fmt::Display for StateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateError::Io { path, source } => write!(formatter, "{}: {}", path.display(), source),
            StateError::Parse { path, source } => {
                write!(formatter, "failed to parse {}: {}", path.display(), source)
            }
            StateError::Serialize { path, source } => {
                write!(
                    formatter,
                    "failed to serialize {}: {}",
                    path.display(),
                    source
                )
            }
        }
    }
}

impl Error for StateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            StateError::Io { source, .. } => Some(source),
            StateError::Parse { source, .. } => Some(source),
            StateError::Serialize { source, .. } => Some(source),
        }
    }
}

fn state_path(root: &Path) -> PathBuf {
    root.join("state").join("state.json")
}
