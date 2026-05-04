use super::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("selfforge-{name}-{stamp}"))
}

fn cleanup(path: &Path) {
    if path.exists() {
        let _ = fs::remove_dir_all(path);
    }
}

fn env_lookup<'a>(values: &'a HashMap<&str, &str>) -> impl Fn(&str) -> Option<String> + 'a {
    move |key| values.get(key).map(|value| (*value).to_string())
}

fn assert_workspace_structure(root: &Path) {
    let workspace = root.join("workspaces").join("v0");
    assert!(workspace.join("README.md").is_file());
    assert!(workspace.join(".gitignore").is_file());
    for directory in ["source", "tests", "sandbox", "artifacts", "logs"] {
        assert!(workspace.join(directory).is_dir());
        assert!(workspace.join(directory).join("README.md").is_file());
    }
}

fn create_patch_draft_for_audit(
    root: &Path,
    app: &SelfForgeApp,
    scope_markdown: &str,
) -> AiPatchDraftRecord {
    fs::write(
        root.join(".env"),
        "SELFFORGE_AI_PROVIDER=deepseek\nDEEPSEEK_API_KEY=test-patch-audit-key\n",
    )
    .expect("test should write dotenv file");
    fs::write(
            root.join("forge").join("memory").join("v0.md"),
            format!(
                "# v0 记忆记录\n\n## {CURRENT_VERSION}\n\n# 错误总结\n\n本轮没有未解决错误。\n\n# 评估\n\n系统已经具备受控补丁草案能力。\n\n# 优化建议\n\n实现候选补丁差异审计和冲突检查。\n\n# 可复用经验\n\nAI 生成代码前必须先审计写入范围。\n"
            ),
        )
        .expect("test should write memory archive");
    let preview = app
        .ai_patch_draft_preview_with_lookup("生成补丁草案", |_| None)
        .expect("preview should build before audit fixture");
    let request = preview.request.clone();
    let ai = AiExecutionReport {
        request,
        response: AiTextResponse {
            provider_id: "deepseek".to_string(),
            model: "deepseek-v4-flash".to_string(),
            protocol: "openai-chat-completions".to_string(),
            text: format!(
                "# 补丁目标\n生成受控补丁草案。\n\n# 计划\n1. 审计写入范围。\n2. 执行测试。\n\n# 允许写入范围\n{scope_markdown}\n\n# 代码草案\n```rust\nfn example() {{}}\n```\n\n# 测试草案\n```rust\n#[test]\nfn example_test() {{}}\n```\n\n# 验证命令\ncargo test\n\n# 风险与回滚\n失败时保留稳定版本。\n"
            ),
            raw_bytes: 300,
        },
        status_code: 200,
    };

    app.finish_ai_patch_draft(preview, ai)
        .expect("successful patch draft fixture should be written")
        .record
}

mod agent_core;
mod ai_provider;
mod app_loop;
mod layout_version;
mod memory;
mod patch_draft_flow;
mod patch_source_flow;
mod runtime_error;
