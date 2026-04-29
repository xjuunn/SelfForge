use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentationReport {
    pub checked_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentationViolation {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug)]
pub enum DocumentationError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Violations {
        violations: Vec<DocumentationViolation>,
    },
}

pub fn validate_chinese_markdown(
    root: impl AsRef<Path>,
) -> Result<DocumentationReport, DocumentationError> {
    let root = root.as_ref();
    let mut files = Vec::new();
    collect_markdown_files(root, &mut files)?;
    files.sort();

    let mut violations = Vec::new();
    for file in &files {
        let contents = fs::read_to_string(file).map_err(|source| DocumentationError::Io {
            path: file.clone(),
            source,
        })?;
        if !contains_chinese(&contents) {
            violations.push(DocumentationViolation {
                path: file.clone(),
                reason: "文档缺少中文内容".to_string(),
            });
            continue;
        }
        if contains_mojibake(&contents) {
            violations.push(DocumentationViolation {
                path: file.clone(),
                reason: "文档包含明显乱码标记".to_string(),
            });
        }
        if contains_emoji(&contents) {
            violations.push(DocumentationViolation {
                path: file.clone(),
                reason: "文档包含 Emoji".to_string(),
            });
        }
    }

    if violations.is_empty() {
        Ok(DocumentationReport {
            checked_files: files,
        })
    } else {
        Err(DocumentationError::Violations { violations })
    }
}

impl fmt::Display for DocumentationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentationError::Io { path, source } => {
                write!(formatter, "{}: {}", path.display(), source)
            }
            DocumentationError::Violations { violations } => {
                write!(formatter, "documentation policy violations: ")?;
                for (index, violation) in violations.iter().enumerate() {
                    if index > 0 {
                        write!(formatter, ", ")?;
                    }
                    write!(
                        formatter,
                        "{} ({})",
                        violation.path.display(),
                        violation.reason
                    )?;
                }
                Ok(())
            }
        }
    }
}

impl Error for DocumentationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            DocumentationError::Io { source, .. } => Some(source),
            DocumentationError::Violations { .. } => None,
        }
    }
}

fn collect_markdown_files(
    directory: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), DocumentationError> {
    if should_skip(directory) {
        return Ok(());
    }

    let entries = fs::read_dir(directory).map_err(|source| DocumentationError::Io {
        path: directory.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| DocumentationError::Io {
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_markdown_files(&path, files)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("md") {
            files.push(path);
        }
    }

    Ok(())
}

fn should_skip(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, ".git" | "target"))
}

fn contains_chinese(contents: &str) -> bool {
    contents
        .chars()
        .any(|character| ('\u{4e00}'..='\u{9fff}').contains(&character))
}

fn contains_mojibake(contents: &str) -> bool {
    const MARKERS: [&str; 10] = ["鐗", "浣", "锛", "銆", "鈫", "圥", "坒", "闅", "绯", "璁"];

    MARKERS.iter().any(|marker| contents.contains(marker))
}

fn contains_emoji(contents: &str) -> bool {
    contents.chars().any(|character| {
        matches!(
            character,
            '\u{1f000}'..='\u{1faff}' | '\u{2600}'..='\u{27bf}' | '\u{fe0f}'
        )
    })
}
