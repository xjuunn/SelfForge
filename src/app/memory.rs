use crate::version::{ForgeVersion, VersionError, version_major_file_name};
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

#[derive(Debug)]
pub enum MemoryContextError {
    Version(VersionError),
    MissingArchive { version: String, path: PathBuf },
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

fn should_replace_section(existing: &MemoryContextEntry, candidate: &MemoryContextEntry) -> bool {
    let existing_final = existing.title.contains("最终");
    let candidate_final = candidate.title.contains("最终");
    candidate_final || !existing_final
}

fn version_key(version: &str) -> (u64, u64, u64) {
    let parsed = ForgeVersion::from_str(version).expect("记忆小节版本号应已通过解析");
    (parsed.major(), parsed.minor(), parsed.patch())
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
