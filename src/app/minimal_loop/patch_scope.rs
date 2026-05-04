use super::super::agent::{
    AgentWorkQueueReport, AgentWorkTaskStatus, AiPatchAuditFinding, AiPatchAuditFindingKind,
    AiPatchAuditSeverity,
};
use super::AiPatchAuditError;
use crate::version_major_key;
use std::collections::HashSet;

#[derive(Debug)]
pub(super) struct PatchWriteScopeAudit {
    pub(super) normalized_write_scope: Vec<String>,
    pub(super) findings: Vec<AiPatchAuditFinding>,
}

pub(super) fn extract_patch_audit_write_scope(markdown: &str) -> Vec<String> {
    let mut in_scope_section = false;
    let mut scopes = Vec::new();
    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(title) = markdown_heading_title(trimmed) {
            if in_scope_section && !title.contains("允许写入范围") && !title.contains("写入范围")
            {
                break;
            }
            in_scope_section = title.contains("允许写入范围") || title == "写入范围";
            continue;
        }
        if !in_scope_section || trimmed.is_empty() || trimmed.starts_with("```") {
            continue;
        }
        for scope in split_patch_scope_line(trimmed) {
            if !scope.is_empty() {
                scopes.push(scope);
            }
        }
    }

    scopes
}

pub(super) fn audit_patch_write_scope(
    requested_write_scope: &[String],
    version: &str,
) -> Result<PatchWriteScopeAudit, AiPatchAuditError> {
    let mut findings = Vec::new();
    let mut normalized_write_scope = Vec::new();
    let mut seen = HashSet::new();
    if requested_write_scope.is_empty() {
        findings.push(AiPatchAuditFinding {
            severity: AiPatchAuditSeverity::Error,
            kind: AiPatchAuditFindingKind::MissingWriteScope,
            message: "补丁草案缺少可审计的允许写入范围。".to_string(),
            path: None,
            task_id: None,
            task_title: None,
            worker_id: None,
        });
    }

    let protected_roots = patch_audit_protected_roots(version)?;
    for raw_scope in requested_write_scope {
        match normalize_patch_scope_path(raw_scope) {
            Ok(scope) => {
                if patch_scope_is_protected(&scope, &protected_roots) {
                    findings.push(AiPatchAuditFinding {
                        severity: AiPatchAuditSeverity::Error,
                        kind: AiPatchAuditFindingKind::ProtectedPath,
                        message: "补丁草案请求修改受保护路径。".to_string(),
                        path: Some(scope.clone()),
                        task_id: None,
                        task_title: None,
                        worker_id: None,
                    });
                }
                if seen.insert(scope.clone()) {
                    normalized_write_scope.push(scope);
                }
            }
            Err(reason) => findings.push(AiPatchAuditFinding {
                severity: AiPatchAuditSeverity::Error,
                kind: AiPatchAuditFindingKind::InvalidPath,
                message: reason,
                path: Some(raw_scope.clone()),
                task_id: None,
                task_title: None,
                worker_id: None,
            }),
        }
    }

    Ok(PatchWriteScopeAudit {
        normalized_write_scope,
        findings,
    })
}

pub(super) fn audit_patch_scope_conflicts(
    normalized_write_scope: &[String],
    queue_report: &AgentWorkQueueReport,
) -> Vec<AiPatchAuditFinding> {
    let mut findings = Vec::new();
    let mut seen = HashSet::new();
    for task in queue_report
        .queue
        .tasks
        .iter()
        .filter(|task| task.status == AgentWorkTaskStatus::Claimed)
    {
        for requested_scope in normalized_write_scope {
            if scopes_overlap_one_to_many(requested_scope, &task.write_scope) {
                let key = format!("{}:{requested_scope}", task.id);
                if seen.insert(key) {
                    findings.push(AiPatchAuditFinding {
                        severity: AiPatchAuditSeverity::Error,
                        kind: AiPatchAuditFindingKind::ActiveConflict,
                        message: "补丁草案写入范围与已领取协作任务重叠。".to_string(),
                        path: Some(requested_scope.clone()),
                        task_id: Some(task.id.clone()),
                        task_title: Some(task.title.clone()),
                        worker_id: task.claimed_by.clone(),
                    });
                }
            }
        }
    }

    findings
}

pub(super) fn patch_audit_protected_roots(version: &str) -> Result<Vec<String>, AiPatchAuditError> {
    let major = version_major_key(version).map_err(AiPatchAuditError::Version)?;
    Ok(vec![
        ".git/".to_string(),
        ".env".to_string(),
        "runtime/".to_string(),
        "supervisor/".to_string(),
        "state/".to_string(),
        "target/".to_string(),
        format!("workspaces/{major}/sandbox/"),
        format!("workspaces/{major}/logs/"),
    ])
}

pub(super) fn markdown_heading_title(line: &str) -> Option<String> {
    if !line.starts_with('#') {
        return None;
    }
    let title = line.trim_start_matches('#').trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

fn split_patch_scope_line(line: &str) -> Vec<String> {
    let mut value = line
        .trim()
        .trim_start_matches(|character| matches!(character, '-' | '*' | '+'))
        .trim()
        .to_string();
    while value
        .chars()
        .next()
        .map(|character| character.is_ascii_digit() || matches!(character, '.' | ')' | '、'))
        .unwrap_or(false)
    {
        value = value
            .chars()
            .skip(1)
            .collect::<String>()
            .trim_start()
            .to_string();
    }
    value = value
        .replace('`', "")
        .replace('"', "")
        .replace('\'', "")
        .replace('“', "")
        .replace('”', "")
        .trim()
        .to_string();

    value
        .split(|character| matches!(character, '，' | ',' | '；' | ';'))
        .filter_map(scope_candidate_from_segment)
        .collect()
}

fn scope_candidate_from_segment(segment: &str) -> Option<String> {
    let mut value = segment.trim();
    if value.is_empty() || matches!(value, "无" | "暂无") {
        return None;
    }
    if let Some((left, right)) = value.split_once('：') {
        if !looks_like_path(left) || left.contains("路径") || left.contains("文件") {
            value = right.trim();
        }
    } else if let Some((left, right)) = value.split_once(':') {
        if !looks_like_path(left) || left.contains("path") || left.contains("file") {
            value = right.trim();
        }
    }
    let first = value.split_whitespace().next().unwrap_or("").trim();
    let first = first
        .trim_matches(|character| matches!(character, '。' | '，' | ',' | '；' | ';' | '：' | ':'));
    if first.is_empty() || matches!(first, "无" | "暂无") {
        None
    } else {
        Some(first.to_string())
    }
}

fn looks_like_path(value: &str) -> bool {
    let value = value.trim();
    value.contains('/')
        || value.contains('\\')
        || value.starts_with('.')
        || value.ends_with(".rs")
        || value.ends_with(".md")
        || matches!(
            value,
            "Cargo.toml" | "Cargo.lock" | "README.md" | "Agents.md"
        )
}

pub(super) fn normalize_patch_scope_path(value: &str) -> Result<String, String> {
    let mut scope = value
        .trim()
        .replace('\\', "/")
        .trim_matches(|character| matches!(character, '`' | '"' | '\'' | '“' | '”'))
        .trim()
        .to_string();
    while scope.starts_with("./") {
        scope = scope.trim_start_matches("./").to_string();
    }
    scope = scope.trim_end_matches('/').to_string();
    if scope.is_empty() {
        return Err("写入范围为空。".to_string());
    }
    if scope.starts_with('/') || scope.starts_with('~') || scope.chars().nth(1) == Some(':') {
        return Err("写入范围必须是仓库相对路径，禁止使用绝对路径。".to_string());
    }
    if scope
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("写入范围包含非法路径片段。".to_string());
    }
    if scope
        .chars()
        .any(|character| matches!(character, '<' | '>' | '|' | '?' | '*'))
    {
        return Err("写入范围包含非法文件名字符。".to_string());
    }

    Ok(scope)
}

fn patch_scope_is_protected(scope: &str, protected_roots: &[String]) -> bool {
    protected_roots.iter().any(|root| {
        let root = normalize_scope_for_compare(root);
        let scope = normalize_scope_for_compare(scope);
        scope == root || scope.starts_with(&(root + "/"))
    })
}

fn scopes_overlap_one_to_many(left: &str, right: &[String]) -> bool {
    right
        .iter()
        .any(|right_scope| scopes_overlap_pair(left, right_scope))
}

fn scopes_overlap_pair(left: &str, right: &str) -> bool {
    let left = normalize_scope_for_compare(left);
    let right = normalize_scope_for_compare(right);
    left == right || left.starts_with(&(right.clone() + "/")) || right.starts_with(&(left + "/"))
}

fn normalize_scope_for_compare(scope: &str) -> String {
    scope
        .trim()
        .trim_end_matches(|character| matches!(character, '/' | '\\'))
        .replace('\\', "/")
}
