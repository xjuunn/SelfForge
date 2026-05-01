use crate::version::{ForgeVersion, VersionError, version_major_file_name};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryContextReport {
    pub version: String,
    pub archive_path: PathBuf,
    pub entries: Vec<MemoryContextEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryContextEntry {
    pub version: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryInsightReport {
    pub version: String,
    pub archive_path: PathBuf,
    pub source_versions: Vec<String>,
    pub success_experiences: Vec<MemoryInsight>,
    pub failure_experiences: Vec<MemoryInsight>,
    pub optimization_suggestions: Vec<MemoryInsight>,
    pub reusable_experiences: Vec<MemoryInsight>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryInsight {
    pub version: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryCompactionReport {
    pub version: String,
    pub memory_path: PathBuf,
    pub archive_path: PathBuf,
    pub original_sections: usize,
    pub kept_sections: usize,
    pub archived_sections: usize,
    pub total_archive_sections: usize,
}

#[derive(Debug)]
pub enum MemoryContextError {
    Version(VersionError),
    MissingArchive { version: String, path: PathBuf },
    Io { path: PathBuf, source: io::Error },
}

#[derive(Debug)]
pub enum MemoryCompactionError {
    Version(VersionError),
    MissingMemory { version: String, path: PathBuf },
    InvalidKeepCount,
    Io { path: PathBuf, source: io::Error },
}

pub fn read_recent_memory_context(
    root: impl AsRef<Path>,
    version: impl AsRef<str>,
    limit: usize,
) -> Result<MemoryContextReport, MemoryContextError> {
    let root = root.as_ref();
    let version = version.as_ref().to_string();
    let archive_file = version_major_file_name(&version)?;
    let archive_path = root.join("forge").join("memory").join(archive_file);

    if limit == 0 {
        return Ok(MemoryContextReport {
            version,
            archive_path,
            entries: Vec::new(),
        });
    }

    if !archive_path.exists() {
        return Err(MemoryContextError::MissingArchive {
            version,
            path: archive_path,
        });
    }

    let contents = fs::read_to_string(&archive_path).map_err(|source| MemoryContextError::Io {
        path: archive_path.clone(),
        source,
    })?;
    let mut entries = unique_latest_sections(parse_memory_sections(&contents));
    entries.sort_by(|left, right| version_key(&right.version).cmp(&version_key(&left.version)));
    entries.truncate(limit);

    Ok(MemoryContextReport {
        version,
        archive_path,
        entries,
    })
}

pub fn compact_memory_archive(
    root: impl AsRef<Path>,
    version: impl AsRef<str>,
    keep_recent: usize,
) -> Result<MemoryCompactionReport, MemoryCompactionError> {
    if keep_recent == 0 {
        return Err(MemoryCompactionError::InvalidKeepCount);
    }

    let root = root.as_ref();
    let version = version.as_ref().to_string();
    let archive_file = version_major_file_name(&version)?;
    let major = archive_file.trim_end_matches(".md");
    let memory_path = root.join("forge").join("memory").join(&archive_file);
    let archive_path = root
        .join("forge")
        .join("memory")
        .join("archive")
        .join(&archive_file);

    if !memory_path.exists() {
        return Err(MemoryCompactionError::MissingMemory {
            version,
            path: memory_path,
        });
    }

    let memory_contents =
        fs::read_to_string(&memory_path).map_err(|source| MemoryCompactionError::Io {
            path: memory_path.clone(),
            source,
        })?;
    let mut memory_sections = unique_latest_sections(parse_memory_sections(&memory_contents));
    sort_sections_desc(&mut memory_sections);

    let original_sections = memory_sections.len();
    let kept_sections: Vec<MemoryContextEntry> =
        memory_sections.iter().take(keep_recent).cloned().collect();
    let moved_sections: Vec<MemoryContextEntry> =
        memory_sections.iter().skip(keep_recent).cloned().collect();

    let existing_archive_sections = if archive_path.exists() {
        let archive_contents =
            fs::read_to_string(&archive_path).map_err(|source| MemoryCompactionError::Io {
                path: archive_path.clone(),
                source,
            })?;
        parse_memory_sections(&archive_contents)
    } else {
        Vec::new()
    };
    let moved_versions: BTreeSet<String> = moved_sections
        .iter()
        .map(|section| section.version.clone())
        .collect();

    let mut archive_sections = unique_latest_sections(
        existing_archive_sections
            .into_iter()
            .chain(moved_sections)
            .collect(),
    );
    sort_sections_desc(&mut archive_sections);

    if let Some(parent) = archive_path.parent() {
        fs::create_dir_all(parent).map_err(|source| MemoryCompactionError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    fs::write(
        &archive_path,
        render_cold_memory_archive(major, &archive_sections),
    )
    .map_err(|source| MemoryCompactionError::Io {
        path: archive_path.clone(),
        source,
    })?;
    fs::write(
        &memory_path,
        render_hot_memory_archive(
            major,
            &format!("forge/memory/archive/{archive_file}"),
            &kept_sections,
            archive_sections.len(),
        ),
    )
    .map_err(|source| MemoryCompactionError::Io {
        path: memory_path.clone(),
        source,
    })?;

    Ok(MemoryCompactionReport {
        version,
        memory_path,
        archive_path,
        original_sections,
        kept_sections: kept_sections.len(),
        archived_sections: moved_versions.len(),
        total_archive_sections: archive_sections.len(),
    })
}

pub fn extract_memory_insights(
    root: impl AsRef<Path>,
    version: impl AsRef<str>,
    limit: usize,
) -> Result<MemoryInsightReport, MemoryContextError> {
    let context = read_recent_memory_context(root, version, limit)?;
    let mut success_experiences = Vec::new();
    let mut failure_experiences = Vec::new();
    let mut optimization_suggestions = Vec::new();
    let mut reusable_experiences = Vec::new();

    for entry in &context.entries {
        success_experiences.extend(extract_heading_items(entry, "评估"));
        failure_experiences.extend(extract_failure_items(entry));
        optimization_suggestions.extend(extract_heading_items(entry, "优化建议"));
        reusable_experiences.extend(extract_heading_items(entry, "可复用经验"));
    }

    Ok(MemoryInsightReport {
        version: context.version,
        archive_path: context.archive_path,
        source_versions: context
            .entries
            .iter()
            .map(|entry| entry.version.clone())
            .collect(),
        success_experiences,
        failure_experiences,
        optimization_suggestions,
        reusable_experiences,
    })
}

fn parse_memory_sections(contents: &str) -> Vec<MemoryContextEntry> {
    let mut sections = Vec::new();
    let mut current = None;

    for line in contents.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            if let Some(version) = section_version(title) {
                if let Some(entry) = current.take() {
                    sections.push(entry);
                }
                let mut body = String::new();
                body.push_str(line);
                body.push('\n');
                current = Some(MemoryContextEntry {
                    version,
                    title: title.to_string(),
                    body,
                });
                continue;
            }
        }

        if let Some(entry) = current.as_mut() {
            entry.body.push_str(line);
            entry.body.push('\n');
        }
    }

    if let Some(entry) = current {
        sections.push(entry);
    }

    sections
}

fn section_version(title: &str) -> Option<String> {
    let version = title.split_whitespace().next()?;
    ForgeVersion::from_str(version).ok()?;
    Some(version.to_string())
}

fn unique_latest_sections(sections: Vec<MemoryContextEntry>) -> Vec<MemoryContextEntry> {
    let mut unique = Vec::<MemoryContextEntry>::new();

    for section in sections {
        if let Some(existing) = unique
            .iter_mut()
            .find(|entry| entry.version == section.version)
        {
            if should_replace_section(existing, &section) {
                *existing = section;
            }
        } else {
            unique.push(section);
        }
    }

    unique
}

fn sort_sections_desc(sections: &mut [MemoryContextEntry]) {
    sections.sort_by(|left, right| version_key(&right.version).cmp(&version_key(&left.version)));
}

fn should_replace_section(existing: &MemoryContextEntry, candidate: &MemoryContextEntry) -> bool {
    let existing_final = existing.title.contains("最终");
    let candidate_final = candidate.title.contains("最终");
    candidate_final || !existing_final
}

fn version_key(version: &str) -> (u64, u64, u64) {
    let parsed = ForgeVersion::from_str(version).expect("记忆小节版本号应已通过解析");
    (parsed.major(), parsed.minor(), parsed.patch())
}

fn render_hot_memory_archive(
    major: &str,
    archive_relative_path: &str,
    kept_sections: &[MemoryContextEntry],
    total_archive_sections: usize,
) -> String {
    let newest = kept_sections
        .first()
        .map(|section| section.version.as_str())
        .unwrap_or("无");
    let oldest = kept_sections
        .last()
        .map(|section| section.version.as_str())
        .unwrap_or("无");
    let mut output = format!(
        "# {major} 记忆记录\n\n# 记录规则\n\n- 本文件是热记忆文件，只保留近期完整记忆，供 `memory-context`、`memory-insights` 和 Agent 默认读取。\n- 久远完整记忆迁移到 `{archive_relative_path}`，只在审计、追溯或人工复盘时读取。\n- 同一 major 只维护一个热记忆文件和一个冷归档文件，禁止为小版本创建独立记忆文件。\n\n## 压缩记忆索引\n\n- 热记忆保留：{} 条。\n- 冷归档完整记忆：{} 条。\n- 最新热记忆：{}。\n- 最早热记忆：{}。\n- 归档文件：`{}`。\n\n",
        kept_sections.len(),
        total_archive_sections,
        newest,
        oldest,
        archive_relative_path
    );

    for section in kept_sections {
        output.push_str(section.body.trim_end());
        output.push_str("\n\n");
    }

    output
}

fn render_cold_memory_archive(major: &str, archive_sections: &[MemoryContextEntry]) -> String {
    let newest = archive_sections
        .first()
        .map(|section| section.version.as_str())
        .unwrap_or("无");
    let oldest = archive_sections
        .last()
        .map(|section| section.version.as_str())
        .unwrap_or("无");
    let mut output = format!(
        "# {major} 历史记忆冷归档\n\n# 记录规则\n\n- 本文件保存从热记忆文件迁出的完整历史记忆，按 major 聚合。\n- 默认计划、Agent 和 `memory-context` 不读取本文件，避免无关历史占用上下文。\n- 只有审计、追溯、问题复盘或人工指定时才读取本文件。\n\n## 冷归档索引\n\n- 完整归档记忆：{} 条。\n- 最新归档记忆：{}。\n- 最早归档记忆：{}。\n\n",
        archive_sections.len(),
        newest,
        oldest
    );

    for section in archive_sections {
        output.push_str(section.body.trim_end());
        output.push_str("\n\n");
    }

    output
}

fn extract_heading_items(entry: &MemoryContextEntry, heading: &str) -> Vec<MemoryInsight> {
    heading_section(&entry.body, heading)
        .lines()
        .filter_map(normalize_memory_line)
        .map(|text| MemoryInsight {
            version: entry.version.clone(),
            text,
        })
        .collect()
}

fn extract_failure_items(entry: &MemoryContextEntry) -> Vec<MemoryInsight> {
    extract_heading_items(entry, "错误总结")
        .into_iter()
        .filter(|insight| !is_no_failure_summary(&insight.text))
        .collect()
}

fn heading_section(body: &str, heading: &str) -> String {
    let marker = format!("# {heading}");
    let mut collecting = false;
    let mut section = Vec::new();

    for line in body.lines() {
        if line.trim() == marker {
            collecting = true;
            continue;
        }
        if collecting && line.starts_with("# ") {
            break;
        }
        if collecting {
            section.push(line);
        }
    }

    section.join("\n")
}

fn normalize_memory_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let without_marker = trimmed.trim_start_matches(['-', '*', ' ']);
    let normalized = strip_number_prefix(without_marker).trim();
    if normalized.is_empty() || is_placeholder(normalized) {
        return None;
    }

    Some(normalized.to_string())
}

fn strip_number_prefix(value: &str) -> &str {
    let Some((prefix, rest)) = value.split_once('.') else {
        return value;
    };
    if prefix.chars().all(|character| character.is_ascii_digit()) {
        rest
    } else {
        value
    }
}

fn is_placeholder(value: &str) -> bool {
    matches!(
        value,
        "待最终验证后补充。" | "暂无。" | "无。" | "无未解决错误。"
    )
}

fn is_no_failure_summary(value: &str) -> bool {
    value.contains("没有新增未解决错误")
        || value.contains("未发现功能错误")
        || value.contains("未发现未解决错误")
        || value.contains("无未解决错误")
        || value == "本轮最终验证未发现未解决错误。"
}

impl fmt::Display for MemoryContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryContextError::Version(error) => write!(formatter, "{error}"),
            MemoryContextError::MissingArchive { version, path } => write!(
                formatter,
                "版本 {version} 的记忆归档不存在：{}",
                path.display()
            ),
            MemoryContextError::Io { path, source } => {
                write!(formatter, "{}: {}", path.display(), source)
            }
        }
    }
}

impl Error for MemoryContextError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            MemoryContextError::Version(error) => Some(error),
            MemoryContextError::MissingArchive { .. } => None,
            MemoryContextError::Io { source, .. } => Some(source),
        }
    }
}

impl From<VersionError> for MemoryContextError {
    fn from(error: VersionError) -> Self {
        MemoryContextError::Version(error)
    }
}

impl fmt::Display for MemoryCompactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryCompactionError::Version(error) => write!(formatter, "{error}"),
            MemoryCompactionError::MissingMemory { version, path } => write!(
                formatter,
                "版本 {version} 的热记忆文件不存在：{}",
                path.display()
            ),
            MemoryCompactionError::InvalidKeepCount => {
                write!(formatter, "memory-compact 的 --keep 必须大于 0")
            }
            MemoryCompactionError::Io { path, source } => {
                write!(formatter, "{}: {}", path.display(), source)
            }
        }
    }
}

impl Error for MemoryCompactionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            MemoryCompactionError::Version(error) => Some(error),
            MemoryCompactionError::MissingMemory { .. } => None,
            MemoryCompactionError::InvalidKeepCount => None,
            MemoryCompactionError::Io { source, .. } => Some(source),
        }
    }
}

impl From<VersionError> for MemoryCompactionError {
    fn from(error: VersionError) -> Self {
        MemoryCompactionError::Version(error)
    }
}
