use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const DEFAULT_CODE_READ_BYTES: usize = 16 * 1024;
const MAX_CODE_READ_BYTES: usize = 64 * 1024;
const DEFAULT_CODE_SEARCH_LIMIT: usize = 20;
const MAX_CODE_SEARCH_LIMIT: usize = 100;
const CODE_SEARCH_FILE_BYTES: usize = 64 * 1024;
const DEFAULT_CODE_DIFF_BYTES: usize = 16 * 1024;
const MAX_CODE_DIFF_BYTES: usize = 64 * 1024;
const CODE_OUTLINE_FILE_BYTES: usize = 64 * 1024;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCodeListReport {
    pub path: String,
    pub file_count: usize,
    pub truncated: bool,
    pub files: Vec<AgentCodeListEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCodeListEntry {
    pub path: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCodeDiffReport {
    pub path: String,
    pub status_entries: Vec<String>,
    pub diff_bytes: usize,
    pub truncated: bool,
    pub diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCodeOutlineReport {
    pub path: String,
    pub symbol_count: usize,
    pub truncated: bool,
    pub items: Vec<AgentCodeOutlineItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCodeOutlineItem {
    pub line: usize,
    pub kind: String,
    pub name: String,
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
    Git {
        args: Vec<String>,
        code: Option<i32>,
        stderr: String,
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
    if is_sensitive_local_env_file(&resolved) {
        return Err(AgentCodeToolError::InvalidPath { path: resolved });
    }
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

pub fn list_project_code_files(
    root: impl AsRef<Path>,
    path: impl AsRef<str>,
    limit: usize,
) -> Result<AgentCodeListReport, AgentCodeToolError> {
    let root = root.as_ref();
    let requested = path.as_ref();
    let resolved = resolve_project_path(root, requested)?;
    let limit = bounded_search_limit(limit);
    let mut files = Vec::new();
    let mut total_file_count = 0;
    let mut truncated = false;

    if resolved.is_file() {
        if is_sensitive_local_env_file(&resolved) {
            return Err(AgentCodeToolError::InvalidPath { path: resolved });
        }
        let metadata = fs::metadata(&resolved).map_err(|source| AgentCodeToolError::Io {
            path: resolved.clone(),
            source,
        })?;
        total_file_count = 1;
        files.push(AgentCodeListEntry {
            path: relative_path(root, &resolved),
            bytes: metadata.len(),
        });
    } else {
        collect_files(root, &resolved, &mut |path, metadata| {
            total_file_count += 1;
            if files.len() < limit {
                files.push(AgentCodeListEntry {
                    path: relative_path(root, path),
                    bytes: metadata.len(),
                });
            } else {
                truncated = true;
            }
            Ok(())
        })?;
    }

    Ok(AgentCodeListReport {
        path: relative_path(root, &resolved),
        file_count: total_file_count,
        truncated,
        files,
    })
}

pub fn inspect_project_code_diff(
    root: impl AsRef<Path>,
    path: impl AsRef<str>,
    max_bytes: usize,
) -> Result<AgentCodeDiffReport, AgentCodeToolError> {
    let root = root.as_ref();
    let requested = path.as_ref();
    let resolved = resolve_project_path(root, requested)?;
    if resolved.is_file() && is_sensitive_local_env_file(&resolved) {
        return Err(AgentCodeToolError::InvalidPath { path: resolved });
    }
    let pathspec = git_pathspec(root, &resolved);
    let status_output = run_git(
        root,
        &[
            "status",
            "--short",
            "--untracked-files=all",
            "--",
            &pathspec,
        ],
    )?;
    let status_entries = status_output
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .filter(|line| !status_line_mentions_sensitive_file(line))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let diff_output = run_git(root, &["diff", "--no-ext-diff", "--", &pathspec])?;
    let filtered_diff = filter_sensitive_diff_sections(&diff_output);
    let max_bytes = bounded_diff_bytes(max_bytes);
    let (diff, diff_bytes, truncated) = truncate_text_to_bytes(filtered_diff, max_bytes)?;

    Ok(AgentCodeDiffReport {
        path: relative_path(root, &resolved),
        status_entries,
        diff_bytes,
        truncated,
        diff,
    })
}

pub fn outline_project_code(
    root: impl AsRef<Path>,
    path: impl AsRef<str>,
    limit: usize,
) -> Result<AgentCodeOutlineReport, AgentCodeToolError> {
    let root = root.as_ref();
    let requested = path.as_ref();
    let resolved = resolve_project_path(root, requested)?;
    if !resolved.is_file() || is_sensitive_local_env_file(&resolved) {
        return Err(AgentCodeToolError::InvalidPath { path: resolved });
    }
    let limit = bounded_search_limit(limit);
    let (content, _, _) = read_text_prefix(&resolved, CODE_OUTLINE_FILE_BYTES)?;
    let mut items = Vec::new();
    let mut symbol_count = 0;
    let mut truncated = false;
    for (index, line) in content.lines().enumerate() {
        let Some((kind, name)) = outline_symbol(line) else {
            continue;
        };
        symbol_count += 1;
        if items.len() < limit {
            items.push(AgentCodeOutlineItem {
                line: index + 1,
                kind,
                name,
                preview: line.trim().chars().take(180).collect(),
            });
        } else {
            truncated = true;
        }
    }

    Ok(AgentCodeOutlineReport {
        path: relative_path(root, &resolved),
        symbol_count,
        truncated,
        items,
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

    collect_files(root, directory, &mut |path, _metadata| {
        if matches.len() < limit {
            *scanned_file_count += 1;
            search_file(root, &path, query, limit, matched_files, matches);
        }
        Ok(())
    })?;

    Ok(())
}

fn collect_files(
    root: &Path,
    directory: &Path,
    visitor: &mut dyn FnMut(&Path, &fs::Metadata) -> Result<(), AgentCodeToolError>,
) -> Result<(), AgentCodeToolError> {
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
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| AgentCodeToolError::Io {
            path: path.clone(),
            source,
        })?;
        if file_type.is_dir() {
            if should_skip_directory(&path) {
                continue;
            }
            collect_files(root, &path, visitor)?;
        } else if file_type.is_file() {
            let metadata = entry.metadata().map_err(|source| AgentCodeToolError::Io {
                path: path.clone(),
                source,
            })?;
            if path.starts_with(root) {
                if should_skip_file(&path) {
                    continue;
                }
                visitor(&path, &metadata)?;
            }
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

fn outline_symbol(line: &str) -> Option<(String, String)> {
    let text = line.trim();
    if text.is_empty()
        || text.starts_with("//")
        || text.starts_with('#')
        || text.starts_with('*')
        || text.starts_with("use ")
        || text.starts_with("import ")
    {
        return None;
    }
    let text = strip_declaration_prefixes(text);
    for (keyword, kind) in [
        ("fn", "函数"),
        ("struct", "结构体"),
        ("enum", "枚举"),
        ("trait", "特征"),
        ("impl", "实现"),
        ("mod", "模块"),
        ("type", "类型"),
        ("const", "常量"),
        ("static", "静态"),
        ("def", "函数"),
        ("class", "类"),
        ("function", "函数"),
        ("interface", "接口"),
    ] {
        if let Some(rest) = keyword_rest(text, keyword)
            && let Some(name) = outline_name(rest, keyword == "impl")
        {
            return Some((kind.to_string(), name));
        }
    }
    if let Some(name) = arrow_function_name(text) {
        return Some(("函数".to_string(), name));
    }
    None
}

fn strip_declaration_prefixes(mut text: &str) -> &str {
    loop {
        let trimmed = text.trim_start();
        let Some((first, rest)) = trimmed.split_once(char::is_whitespace) else {
            return trimmed;
        };
        if matches!(
            first,
            "pub" | "async" | "unsafe" | "export" | "default" | "open" | "private" | "protected"
        ) || first.starts_with("pub(")
        {
            text = rest;
            continue;
        }
        return trimmed;
    }
}

fn keyword_rest<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = text.strip_prefix(keyword)?;
    if rest.is_empty() || rest.starts_with(char::is_whitespace) || rest.starts_with('<') {
        Some(rest.trim_start())
    } else {
        None
    }
}

fn outline_name(rest: &str, allow_for: bool) -> Option<String> {
    let mut name = String::new();
    for ch in rest.chars() {
        if ch.is_alphanumeric() || matches!(ch, '_' | ':' | '<' | '>' | '\'' | '&') {
            name.push(ch);
        } else {
            break;
        }
    }
    let name = name.trim_matches('&').trim().to_string();
    if name.is_empty() {
        return None;
    }
    if allow_for && name == "for" {
        return None;
    }
    Some(name)
}

fn arrow_function_name(text: &str) -> Option<String> {
    if !text.contains("=>") {
        return None;
    }
    let text = strip_declaration_prefixes(text);
    let rest = text
        .strip_prefix("const ")
        .or_else(|| text.strip_prefix("let "))
        .or_else(|| text.strip_prefix("var "))?;
    let name = rest
        .split(|ch: char| ch.is_whitespace() || matches!(ch, '=' | ':' | '('))
        .next()
        .unwrap_or_default()
        .trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
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

fn should_skip_file(path: &Path) -> bool {
    is_sensitive_local_env_file(path)
}

fn is_sensitive_local_env_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == ".env" || (name.starts_with(".env.") && name != ".env.example"))
}

fn status_line_mentions_sensitive_file(line: &str) -> bool {
    let payload = line.get(3..).unwrap_or(line);
    payload
        .split(" -> ")
        .any(|path| is_sensitive_relative_path(path.trim_matches('"')))
}

fn filter_sensitive_diff_sections(diff: &str) -> String {
    let mut output = String::new();
    let mut keep_section = true;
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            keep_section = !diff_header_mentions_sensitive_file(rest);
        }
        if keep_section {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

fn diff_header_mentions_sensitive_file(rest: &str) -> bool {
    rest.split_whitespace().any(|part| {
        part.strip_prefix("a/")
            .or_else(|| part.strip_prefix("b/"))
            .is_some_and(is_sensitive_relative_path)
    })
}

fn is_sensitive_relative_path(value: &str) -> bool {
    is_sensitive_local_env_file(Path::new(value))
}

fn git_pathspec(root: &Path, path: &Path) -> String {
    let relative = relative_path(root, path);
    if relative.is_empty() {
        ".".to_string()
    } else {
        relative
    }
}

fn run_git(root: &Path, args: &[&str]) -> Result<String, AgentCodeToolError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|source| AgentCodeToolError::Io {
            path: root.to_path_buf(),
            source,
        })?;
    if !output.status.success() {
        return Err(AgentCodeToolError::Git {
            args: args.iter().map(|arg| (*arg).to_string()).collect(),
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    String::from_utf8(output.stdout).map_err(|source| AgentCodeToolError::Utf8 {
        path: root.to_path_buf(),
        source,
    })
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

fn bounded_diff_bytes(value: usize) -> usize {
    if value == 0 {
        DEFAULT_CODE_DIFF_BYTES
    } else {
        value.min(MAX_CODE_DIFF_BYTES)
    }
}

fn truncate_text_to_bytes(
    content: String,
    max_bytes: usize,
) -> Result<(String, usize, bool), AgentCodeToolError> {
    let mut buffer = content.into_bytes();
    let truncated = buffer.len() > max_bytes;
    if truncated {
        buffer.truncate(max_bytes);
        trim_utf8_boundary(&mut buffer);
    }
    let bytes = buffer.len();
    let text = String::from_utf8(buffer).map_err(|source| AgentCodeToolError::Utf8 {
        path: PathBuf::new(),
        source,
    })?;
    Ok((text, bytes, truncated))
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
            AgentCodeToolError::Git { args, code, stderr } => {
                write!(
                    formatter,
                    "Git 只读命令失败 {:?} 退出 {:?}: {}",
                    args, code, stderr
                )
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
            AgentCodeToolError::EmptyQuery
            | AgentCodeToolError::InvalidPath { .. }
            | AgentCodeToolError::Git { .. } => None,
        }
    }
}
