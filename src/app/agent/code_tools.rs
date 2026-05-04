use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

const DEFAULT_CODE_READ_BYTES: usize = 16 * 1024;
const MAX_CODE_READ_BYTES: usize = 64 * 1024;
const DEFAULT_CODE_SEARCH_LIMIT: usize = 20;
const MAX_CODE_SEARCH_LIMIT: usize = 100;
const CODE_SEARCH_FILE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCodeReadReport {
    pub path: String,
    pub bytes_read: usize,
    pub truncated: bool,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCodeSearchReport {
    pub query: String,
    pub scanned_file_count: usize,
    pub matched_file_count: usize,
    pub matches: Vec<AgentCodeSearchMatch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCodeSearchMatch {
    pub path: String,
    pub line: usize,
    pub preview: String,
}

#[derive(Debug)]
pub enum AgentCodeToolError {
    EmptyQuery,
    InvalidPath {
        path: PathBuf,
    },
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Utf8 {
        path: PathBuf,
        source: std::string::FromUtf8Error,
    },
}

pub fn read_project_code_file(
    root: impl AsRef<Path>,
    path: impl AsRef<str>,
    max_bytes: usize,
) -> Result<AgentCodeReadReport, AgentCodeToolError> {
    let root = root.as_ref();
    let relative = path.as_ref();
    let resolved = resolve_project_path(root, relative)?;
    let max_bytes = bounded_read_bytes(max_bytes);
    let (content, bytes_read, truncated) = read_text_prefix(&resolved, max_bytes)?;

    Ok(AgentCodeReadReport {
        path: relative_path(root, &resolved),
        bytes_read,
        truncated,
        content,
    })
}

pub fn search_project_code(
    root: impl AsRef<Path>,
    query: impl AsRef<str>,
    limit: usize,
) -> Result<AgentCodeSearchReport, AgentCodeToolError> {
    let root = root.as_ref();
    let query = query.as_ref().trim().to_string();
    if query.is_empty() {
        return Err(AgentCodeToolError::EmptyQuery);
    }
    let normalized_query = query.to_lowercase();
    let limit = bounded_search_limit(limit);
    let mut matches = Vec::new();
    let mut scanned_file_count = 0;
    let mut matched_files = std::collections::HashSet::new();

    search_directory(
        root,
        root,
        &normalized_query,
        limit,
        &mut scanned_file_count,
        &mut matched_files,
        &mut matches,
    )?;

    Ok(AgentCodeSearchReport {
        query,
        scanned_file_count,
        matched_file_count: matched_files.len(),
        matches,
    })
}

fn search_directory(
    root: &Path,
    directory: &Path,
    query: &str,
    limit: usize,
    scanned_file_count: &mut usize,
    matched_files: &mut std::collections::HashSet<String>,
    matches: &mut Vec<AgentCodeSearchMatch>,
) -> Result<(), AgentCodeToolError> {
    if matches.len() >= limit {
        return Ok(());
    }

    let mut entries = fs::read_dir(directory)
        .map_err(|source| AgentCodeToolError::Io {
            path: directory.to_path_buf(),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| AgentCodeToolError::Io {
            path: directory.to_path_buf(),
            source,
        })?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        if matches.len() >= limit {
            break;
        }
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| AgentCodeToolError::Io {
            path: path.clone(),
            source,
        })?;
        if file_type.is_dir() {
            if should_skip_directory(&path) {
                continue;
            }
            search_directory(
                root,
                &path,
                query,
                limit,
                scanned_file_count,
                matched_files,
                matches,
            )?;
        } else if file_type.is_file() {
            *scanned_file_count += 1;
            search_file(root, &path, query, limit, matched_files, matches);
        }
    }

    Ok(())
}

fn search_file(
    root: &Path,
    path: &Path,
    query: &str,
    limit: usize,
    matched_files: &mut std::collections::HashSet<String>,
    matches: &mut Vec<AgentCodeSearchMatch>,
) {
    let Ok((content, _, _)) = read_text_prefix(path, CODE_SEARCH_FILE_BYTES) else {
        return;
    };
    let relative = relative_path(root, path);
    for (index, line) in content.lines().enumerate() {
        if matches.len() >= limit {
            break;
        }
        if line.to_lowercase().contains(query) {
            matched_files.insert(relative.clone());
            matches.push(AgentCodeSearchMatch {
                path: relative.clone(),
                line: index + 1,
                preview: line.trim().chars().take(180).collect(),
            });
        }
    }
}

fn resolve_project_path(root: &Path, value: &str) -> Result<PathBuf, AgentCodeToolError> {
    let relative = Path::new(value);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(AgentCodeToolError::InvalidPath {
            path: relative.to_path_buf(),
        });
    }
    let resolved = root.join(relative);
    if !resolved.starts_with(root) {
        return Err(AgentCodeToolError::InvalidPath { path: resolved });
    }
    Ok(resolved)
}

fn read_text_prefix(
    path: &Path,
    max_bytes: usize,
) -> Result<(String, usize, bool), AgentCodeToolError> {
    let mut file = fs::File::open(path).map_err(|source| AgentCodeToolError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut buffer = Vec::with_capacity(max_bytes + 4);
    file.by_ref()
        .take((max_bytes + 4) as u64)
        .read_to_end(&mut buffer)
        .map_err(|source| AgentCodeToolError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let truncated = buffer.len() > max_bytes;
    if truncated {
        buffer.truncate(max_bytes);
        trim_utf8_boundary(&mut buffer);
    }
    let bytes_read = buffer.len();
    let content = String::from_utf8(buffer).map_err(|source| AgentCodeToolError::Utf8 {
        path: path.to_path_buf(),
        source,
    })?;
    Ok((content, bytes_read, truncated))
}

fn trim_utf8_boundary(buffer: &mut Vec<u8>) {
    if let Err(error) = std::str::from_utf8(buffer)
        && error.error_len().is_none()
    {
        buffer.truncate(error.valid_up_to());
    }
}

fn should_skip_directory(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, ".git" | "target"))
}

fn bounded_read_bytes(value: usize) -> usize {
    if value == 0 {
        DEFAULT_CODE_READ_BYTES
    } else {
        value.min(MAX_CODE_READ_BYTES)
    }
}

fn bounded_search_limit(value: usize) -> usize {
    if value == 0 {
        DEFAULT_CODE_SEARCH_LIMIT
    } else {
        value.min(MAX_CODE_SEARCH_LIMIT)
    }
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

impl fmt::Display for AgentCodeToolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentCodeToolError::EmptyQuery => write!(formatter, "代码搜索关键词不能为空"),
            AgentCodeToolError::InvalidPath { path } => {
                write!(
                    formatter,
                    "代码文件路径不允许越过项目根目录：{}",
                    path.display()
                )
            }
            AgentCodeToolError::Io { path, source } => {
                write!(formatter, "{}: {}", path.display(), source)
            }
            AgentCodeToolError::Utf8 { path, source } => {
                write!(
                    formatter,
                    "读取代码文件 {} 时发现非法 UTF-8：{}",
                    path.display(),
                    source
                )
            }
        }
    }
}

impl Error for AgentCodeToolError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AgentCodeToolError::Io { source, .. } => Some(source),
            AgentCodeToolError::Utf8 { source, .. } => Some(source),
            AgentCodeToolError::EmptyQuery | AgentCodeToolError::InvalidPath { .. } => None,
        }
    }
}
