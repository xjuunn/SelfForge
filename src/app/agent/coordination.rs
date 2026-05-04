mod model;

pub use self::model::{
    AgentWorkClaimReport, AgentWorkCoordinator, AgentWorkError, AgentWorkEvent, AgentWorkQueue,
    AgentWorkQueueReport, AgentWorkReapReport, AgentWorkTask, AgentWorkTaskStatus,
};

use self::model::DEFAULT_WORK_LEASE_SECONDS;
use crate::version_major_key;
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const COORDINATION_DIRECTORY: &str = "coordination";
const WORK_QUEUE_FILE: &str = "work-queue.json";
const WORK_QUEUE_LOCK: &str = "work-queue.lock";
const LOCK_RETRY_COUNT: usize = 80;
const LOCK_RETRY_DELAY_MS: u64 = 25;

impl AgentWorkCoordinator {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn initialize(
        &self,
        version: &str,
        goal: &str,
        thread_count: usize,
    ) -> Result<AgentWorkQueueReport, AgentWorkError> {
        if thread_count == 0 {
            return Err(AgentWorkError::InvalidThreadCount);
        }
        let version = version.to_string();
        let goal = normalize_goal(goal);
        self.with_lock(&version, |layout| {
            if layout.queue_path.exists() {
                let mut queue = read_queue(&layout.queue_path)?;
                if queue.version != version {
                    queue.version = version.clone();
                    queue.updated_at_unix_seconds = current_unix_seconds();
                    push_event(
                        &mut queue,
                        "retarget",
                        None,
                        None,
                        format!("协作队列已复用到版本 {version}。"),
                    );
                    write_queue(&layout.queue_path, &queue)?;
                }
                return Ok(AgentWorkQueueReport {
                    version: version.clone(),
                    queue_path: layout.queue_path.clone(),
                    created: false,
                    queue,
                });
            }

            let queue = create_queue(&version, &goal, thread_count);
            write_queue(&layout.queue_path, &queue)?;
            Ok(AgentWorkQueueReport {
                version: version.clone(),
                queue_path: layout.queue_path.clone(),
                created: true,
                queue,
            })
        })
    }

    pub fn status(&self, version: &str) -> Result<AgentWorkQueueReport, AgentWorkError> {
        let version = version.to_string();
        let layout = self.layout(&version)?;
        if !layout.queue_path.exists() {
            return Err(AgentWorkError::MissingQueue {
                version,
                path: layout.queue_path,
            });
        }
        let queue = read_queue(&layout.queue_path)?;
        Ok(AgentWorkQueueReport {
            version,
            queue_path: layout.queue_path,
            created: false,
            queue,
        })
    }

    pub fn claim_next(
        &self,
        version: &str,
        worker_id: &str,
        preferred_agent_id: Option<&str>,
    ) -> Result<AgentWorkClaimReport, AgentWorkError> {
        self.claim_next_with_lease(version, worker_id, preferred_agent_id, None)
    }

    pub fn claim_next_with_lease(
        &self,
        version: &str,
        worker_id: &str,
        preferred_agent_id: Option<&str>,
        lease_seconds: Option<u64>,
    ) -> Result<AgentWorkClaimReport, AgentWorkError> {
        validate_worker_id(worker_id)?;
        if lease_seconds == Some(0) {
            return Err(AgentWorkError::InvalidLeaseSeconds);
        }
        let version = version.to_string();
        let worker_id = worker_id.to_string();
        let preferred_agent_id = preferred_agent_id.map(str::to_string);
        self.with_lock(&version, |layout| {
            if !layout.queue_path.exists() {
                return Err(AgentWorkError::MissingQueue {
                    version: version.clone(),
                    path: layout.queue_path.clone(),
                });
            }

            let mut queue = read_queue(&layout.queue_path)?;
            let Some(task_index) = select_claimable_task(&queue, preferred_agent_id.as_deref())
            else {
                return Err(AgentWorkError::NoAvailableTask {
                    version: version.clone(),
                });
            };

            let now = current_unix_seconds();
            let lease_seconds = lease_seconds.unwrap_or(queue.lease_duration_seconds);
            let lease_expires_at = now.saturating_add(lease_seconds);
            queue.updated_at_unix_seconds = now;
            let prompt = build_thread_prompt(
                &queue,
                &queue.tasks[task_index],
                &worker_id,
                lease_expires_at,
            );
            {
                let task = &mut queue.tasks[task_index];
                task.status = AgentWorkTaskStatus::Claimed;
                task.claimed_by = Some(worker_id.clone());
                task.claimed_at_unix_seconds = Some(now);
                task.lease_expires_at_unix_seconds = Some(lease_expires_at);
                task.prompt = prompt.clone();
            }
            let task_id = queue.tasks[task_index].id.clone();
            push_event(
                &mut queue,
                "claim",
                Some(worker_id.clone()),
                Some(task_id),
                "任务已被工作线程领取。",
            );
            write_queue(&layout.queue_path, &queue)?;
            let remaining_available = claimable_task_count(&queue, preferred_agent_id.as_deref());
            Ok(AgentWorkClaimReport {
                version: version.clone(),
                queue_path: layout.queue_path.clone(),
                worker_id,
                task: queue.tasks[task_index].clone(),
                remaining_available,
                prompt,
            })
        })
    }

    pub fn reap_expired(
        &self,
        version: &str,
        reason: &str,
    ) -> Result<AgentWorkReapReport, AgentWorkError> {
        let version = version.to_string();
        let reason = default_summary(reason, "租约过期，任务自动释放。");
        self.with_lock(&version, |layout| {
            if !layout.queue_path.exists() {
                return Err(AgentWorkError::MissingQueue {
                    version: version.clone(),
                    path: layout.queue_path.clone(),
                });
            }

            let mut queue = read_queue(&layout.queue_path)?;
            let now = current_unix_seconds();
            let expired_indexes = queue
                .tasks
                .iter()
                .enumerate()
                .filter(|(_, task)| task.status == AgentWorkTaskStatus::Claimed)
                .filter(|(_, task)| {
                    task.lease_expires_at_unix_seconds
                        .map(|expires_at| expires_at <= now)
                        .unwrap_or(false)
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();

            let mut released_tasks = Vec::new();
            let mut events = Vec::new();
            let goal = queue.goal.clone();
            for index in expired_indexes {
                let task = &mut queue.tasks[index];
                let task_id = task.id.clone();
                let worker_id = task.claimed_by.clone();
                task.status = AgentWorkTaskStatus::Pending;
                task.claimed_by = None;
                task.claimed_at_unix_seconds = None;
                task.lease_expires_at_unix_seconds = None;
                task.result = Some(reason.clone());
                task.prompt = build_base_prompt(&goal, task);
                released_tasks.push(task.clone());
                events.push((worker_id, task_id));
            }

            if !released_tasks.is_empty() {
                queue.updated_at_unix_seconds = now;
                for (worker_id, task_id) in events {
                    push_event(&mut queue, "reap", worker_id, Some(task_id), reason.clone());
                }
                write_queue(&layout.queue_path, &queue)?;
            }

            Ok(AgentWorkReapReport {
                version: version.clone(),
                queue_path: layout.queue_path.clone(),
                released_tasks,
                queue,
            })
        })
    }

    pub fn complete(
        &self,
        version: &str,
        task_id: &str,
        worker_id: &str,
        summary: &str,
    ) -> Result<AgentWorkQueueReport, AgentWorkError> {
        validate_worker_id(worker_id)?;
        validate_task_id(task_id)?;
        self.update_claimed_task(version, task_id, worker_id, |queue, index| {
            let now = current_unix_seconds();
            let message = default_summary(summary, "任务已完成。");
            queue.updated_at_unix_seconds = now;
            {
                let task = &mut queue.tasks[index];
                task.status = AgentWorkTaskStatus::Completed;
                task.completed_at_unix_seconds = Some(now);
                task.lease_expires_at_unix_seconds = None;
                task.result = Some(message.clone());
            }
            push_event(
                queue,
                "complete",
                Some(worker_id.to_string()),
                Some(task_id.to_string()),
                message,
            );
        })
    }

    pub fn release(
        &self,
        version: &str,
        task_id: &str,
        worker_id: &str,
        reason: &str,
    ) -> Result<AgentWorkQueueReport, AgentWorkError> {
        validate_worker_id(worker_id)?;
        validate_task_id(task_id)?;
        self.update_claimed_task(version, task_id, worker_id, |queue, index| {
            let message = default_summary(reason, "任务已释放。");
            let goal = queue.goal.clone();
            queue.updated_at_unix_seconds = current_unix_seconds();
            {
                let task = &mut queue.tasks[index];
                task.status = AgentWorkTaskStatus::Pending;
                task.claimed_by = None;
                task.claimed_at_unix_seconds = None;
                task.lease_expires_at_unix_seconds = None;
                task.completed_at_unix_seconds = None;
                task.result = Some(message.clone());
                task.prompt = build_base_prompt(&goal, task);
            }
            push_event(
                queue,
                "release",
                Some(worker_id.to_string()),
                Some(task_id.to_string()),
                message,
            );
        })
    }

    fn update_claimed_task<F>(
        &self,
        version: &str,
        task_id: &str,
        worker_id: &str,
        update: F,
    ) -> Result<AgentWorkQueueReport, AgentWorkError>
    where
        F: FnOnce(&mut AgentWorkQueue, usize),
    {
        let version = version.to_string();
        let task_id = task_id.to_string();
        let worker_id = worker_id.to_string();
        self.with_lock(&version, |layout| {
            if !layout.queue_path.exists() {
                return Err(AgentWorkError::MissingQueue {
                    version: version.clone(),
                    path: layout.queue_path.clone(),
                });
            }
            let mut queue = read_queue(&layout.queue_path)?;
            let Some(index) = queue.tasks.iter().position(|task| task.id == task_id) else {
                return Err(AgentWorkError::TaskNotFound {
                    task_id: task_id.clone(),
                });
            };
            if queue.tasks[index].claimed_by.as_deref() != Some(worker_id.as_str()) {
                return Err(AgentWorkError::TaskNotClaimedByWorker {
                    task_id: task_id.clone(),
                    worker_id: worker_id.clone(),
                });
            }
            update(&mut queue, index);
            write_queue(&layout.queue_path, &queue)?;
            Ok(AgentWorkQueueReport {
                version: version.clone(),
                queue_path: layout.queue_path.clone(),
                created: false,
                queue,
            })
        })
    }

    fn with_lock<T, F>(&self, version: &str, action: F) -> Result<T, AgentWorkError>
    where
        F: FnOnce(&AgentWorkLayout) -> Result<T, AgentWorkError>,
    {
        let layout = self.layout(version)?;
        fs::create_dir_all(&layout.coordination_dir).map_err(|source| AgentWorkError::Io {
            path: layout.coordination_dir.clone(),
            source,
        })?;
        let _guard = AgentWorkLock::acquire(&layout.lock_path)?;
        action(&layout)
    }

    fn layout(&self, version: &str) -> Result<AgentWorkLayout, AgentWorkError> {
        let major = version_major_key(version)?;
        let workspace = self.root.join("workspaces").join(&major);
        if !workspace.is_dir() {
            return Err(AgentWorkError::WorkspaceMissing {
                version: version.to_string(),
                path: workspace,
            });
        }

        let coordination_dir = workspace
            .join("artifacts")
            .join("agents")
            .join(COORDINATION_DIRECTORY);
        Ok(AgentWorkLayout {
            queue_path: coordination_dir.join(WORK_QUEUE_FILE),
            lock_path: coordination_dir.join(WORK_QUEUE_LOCK),
            coordination_dir,
        })
    }
}

struct AgentWorkLayout {
    coordination_dir: PathBuf,
    queue_path: PathBuf,
    lock_path: PathBuf,
}

struct AgentWorkLock {
    path: PathBuf,
}

impl AgentWorkLock {
    fn acquire(path: &Path) -> Result<Self, AgentWorkError> {
        for _ in 0..LOCK_RETRY_COUNT {
            match OpenOptions::new().write(true).create_new(true).open(path) {
                Ok(mut file) => {
                    let _ = writeln!(file, "pid={}", std::process::id());
                    let _ = writeln!(file, "time={}", current_unix_seconds());
                    return Ok(Self {
                        path: path.to_path_buf(),
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    thread::sleep(Duration::from_millis(LOCK_RETRY_DELAY_MS));
                }
                Err(source) => {
                    return Err(AgentWorkError::Io {
                        path: path.to_path_buf(),
                        source,
                    });
                }
            }
        }

        Err(AgentWorkError::LockBusy {
            path: path.to_path_buf(),
        })
    }
}

impl Drop for AgentWorkLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn create_queue(version: &str, goal: &str, thread_count: usize) -> AgentWorkQueue {
    let now = current_unix_seconds();
    let major = version_major_key(version).unwrap_or_else(|_| "v0".to_string());
    let mut queue = AgentWorkQueue {
        version: version.to_string(),
        goal: goal.to_string(),
        thread_count,
        lease_duration_seconds: DEFAULT_WORK_LEASE_SECONDS,
        created_at_unix_seconds: now,
        updated_at_unix_seconds: now,
        conflict_policy:
            "同一时间禁止两个线程领取写入范围重叠的任务；冲突时释放任务并等待人工或调度层重新分配。"
                .to_string(),
        prompt_policy:
            "每个线程只执行领取到的任务，必须遵守写入范围、依赖关系、验收标准和归档规则。"
                .to_string(),
        tasks: default_tasks(goal, &major),
        events: Vec::new(),
    };
    push_event(&mut queue, "init", None, None, "多 AI 协作队列已初始化。");
    queue
}

fn default_tasks(goal: &str, major: &str) -> Vec<AgentWorkTask> {
    vec![
        work_task(
            "coord-001-architecture",
            "拆解协作架构和提示词边界",
            "梳理目标、协作规则、提示词约束、冲突策略和后续扩展点。",
            "architect",
            10,
            &[],
            vec![
                "Agents.md".to_string(),
                format!("workspaces/{major}/artifacts/agents/coordination/"),
            ],
            &[
                "提示词必须说明只处理已领取任务。",
                "冲突处理必须明确释放或阻断流程。",
            ],
            goal,
        ),
        work_task(
            "coord-002-application",
            "实现应用层协作队列",
            "实现任务板持久化、领取、完成、释放和冲突检测。",
            "builder",
            20,
            &[],
            vec![
                "src/app/agent/".to_string(),
                "src/app/minimal_loop.rs".to_string(),
                "src/app/mod.rs".to_string(),
            ],
            &[
                "队列必须写入 major 工作区 artifacts/agents/coordination。",
                "领取必须防止重复任务和写入范围冲突。",
            ],
            goal,
        ),
        work_task(
            "coord-003-cli",
            "实现协作命令入口",
            "提供初始化、状态查询、领取、完成和释放命令。",
            "builder",
            30,
            &[],
            vec!["src/main.rs".to_string(), "README.md".to_string()],
            &[
                "CLI 只能解析参数和展示应用层结果。",
                "领取命令必须输出当前任务提示词。",
            ],
            goal,
        ),
        work_task(
            "coord-004-tests",
            "补充协作队列测试",
            "覆盖单线程领取、多线程不重复领取、依赖阻断、冲突范围和错误完成。",
            "verifier",
            40,
            &["coord-002-application", "coord-003-cli"],
            vec!["src/lib.rs".to_string()],
            &[
                "必须包含单元、边界和错误测试。",
                "测试必须验证重复领取被阻止。",
            ],
            goal,
        ),
        work_task(
            "coord-005-review-archive",
            "审查并写入中文归档",
            "审查协作队列规则，补充任务、记忆、版本和错误归档。",
            "archivist",
            50,
            &[
                "coord-001-architecture",
                "coord-002-application",
                "coord-003-cli",
                "coord-004-tests",
            ],
            vec![
                format!("forge/tasks/{major}.md"),
                format!("forge/memory/{major}.md"),
                format!("forge/errors/{major}.md"),
                format!("forge/versions/{major}.md"),
            ],
            &["归档必须为中文。", "不得创建小版本独立归档文件。"],
            goal,
        ),
    ]
}

fn work_task(
    id: &str,
    title: &str,
    description: &str,
    preferred_agent_id: &str,
    priority: usize,
    depends_on: &[&str],
    write_scope: Vec<String>,
    acceptance: &[&str],
    goal: &str,
) -> AgentWorkTask {
    let mut task = AgentWorkTask {
        id: id.to_string(),
        title: title.to_string(),
        description: description.to_string(),
        preferred_agent_id: preferred_agent_id.to_string(),
        priority,
        depends_on: depends_on
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        write_scope,
        acceptance: acceptance
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        status: AgentWorkTaskStatus::Pending,
        claimed_by: None,
        claimed_at_unix_seconds: None,
        lease_expires_at_unix_seconds: None,
        completed_at_unix_seconds: None,
        result: None,
        prompt: String::new(),
    };
    task.prompt = build_base_prompt(goal, &task);
    task
}

fn read_queue(path: &Path) -> Result<AgentWorkQueue, AgentWorkError> {
    let contents = fs::read_to_string(path).map_err(|source| AgentWorkError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str::<AgentWorkQueue>(&contents).map_err(|source| AgentWorkError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

fn write_queue(path: &Path, queue: &AgentWorkQueue) -> Result<(), AgentWorkError> {
    let contents =
        serde_json::to_string_pretty(queue).map_err(|source| AgentWorkError::Serialize {
            path: path.to_path_buf(),
            source,
        })? + "\n";
    fs::write(path, contents).map_err(|source| AgentWorkError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn select_claimable_task(
    queue: &AgentWorkQueue,
    preferred_agent_id: Option<&str>,
) -> Option<usize> {
    let available = claimable_task_indexes(queue);
    if let Some(agent_id) = preferred_agent_id {
        if let Some(index) = available
            .iter()
            .copied()
            .find(|index| queue.tasks[*index].preferred_agent_id == agent_id)
        {
            return Some(index);
        }
    }
    available.into_iter().next()
}

fn claimable_task_count(queue: &AgentWorkQueue, preferred_agent_id: Option<&str>) -> usize {
    claimable_task_indexes(queue)
        .into_iter()
        .filter(|index| {
            preferred_agent_id
                .map(|agent_id| queue.tasks[*index].preferred_agent_id == agent_id)
                .unwrap_or(true)
        })
        .count()
}

fn claimable_task_indexes(queue: &AgentWorkQueue) -> Vec<usize> {
    let completed = queue
        .tasks
        .iter()
        .filter(|task| task.status == AgentWorkTaskStatus::Completed)
        .map(|task| task.id.as_str())
        .collect::<HashSet<_>>();
    let mut indexes = queue
        .tasks
        .iter()
        .enumerate()
        .filter(|(_, task)| task.status == AgentWorkTaskStatus::Pending)
        .filter(|(_, task)| {
            task.depends_on
                .iter()
                .all(|dependency| completed.contains(dependency.as_str()))
        })
        .filter(|(_, task)| !has_active_scope_conflict(queue, task))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    indexes.sort_by_key(|index| queue.tasks[*index].priority);
    indexes
}

fn has_active_scope_conflict(queue: &AgentWorkQueue, task: &AgentWorkTask) -> bool {
    queue
        .tasks
        .iter()
        .filter(|other| other.status == AgentWorkTaskStatus::Claimed)
        .any(|other| scopes_overlap(&other.write_scope, &task.write_scope))
}

fn scopes_overlap(left: &[String], right: &[String]) -> bool {
    left.iter().any(|left_scope| {
        let left = normalize_scope(left_scope);
        right.iter().any(|right_scope| {
            let right = normalize_scope(right_scope);
            left == right
                || left.starts_with(&(right.clone() + "/"))
                || right.starts_with(&(left.clone() + "/"))
        })
    })
}

fn normalize_scope(scope: &str) -> String {
    scope
        .trim()
        .trim_end_matches(['/', '\\'])
        .replace('\\', "/")
}

fn build_base_prompt(goal: &str, task: &AgentWorkTask) -> String {
    format!(
        "你是 SelfForge 多 AI 协作任务的候选执行者。\n\n全局目标：{goal}\n任务编号：{}\n任务标题：{}\n职责 Agent：{}\n\n任务描述：{}\n\n写入范围：{}\n\n验收标准：{}\n\n协作规则：先领取任务，再修改代码；只处理领取到的任务；不得修改其他任务的写入范围；发现冲突时释放任务并记录原因。",
        task.id,
        task.title,
        task.preferred_agent_id,
        task.description,
        task.write_scope.join("，"),
        join_chinese_sentences(&task.acceptance)
    )
}

fn build_thread_prompt(
    queue: &AgentWorkQueue,
    task: &AgentWorkTask,
    worker_id: &str,
    lease_expires_at: u64,
) -> String {
    let dependency_text = if task.depends_on.is_empty() {
        "无".to_string()
    } else {
        task.depends_on.join("，")
    };
    let available = claimable_task_indexes(queue)
        .into_iter()
        .filter(|index| queue.tasks[*index].id != task.id)
        .map(|index| queue.tasks[index].id.clone())
        .collect::<Vec<_>>()
        .join("，");
    format!(
        "你是 SelfForge 多 AI 协作线程 `{worker_id}`。\n\n全局目标：{}\n当前任务：{} - {}\n职责 Agent：{}\n任务说明：{}\n\n必须遵守：\n1. 只完成当前已领取任务，禁止重复实现其他任务。\n2. 只修改写入范围：{}。\n3. 依赖任务：{}。\n4. 验收标准：{}。\n5. 租约到期时间 unix:{}，无法继续时必须主动释放任务。\n6. 如发现写入范围冲突、依赖缺失或无法完成，执行 `agent-work-release {} --worker {worker_id} --reason 原因`。\n7. 完成后执行必要测试，并执行 `agent-work-complete {} --worker {worker_id} --summary 摘要`。\n\n当前仍可领取任务：{}。\n冲突策略：{}",
        queue.goal,
        task.id,
        task.title,
        task.preferred_agent_id,
        task.description,
        task.write_scope.join("，"),
        dependency_text,
        join_chinese_sentences(&task.acceptance),
        lease_expires_at,
        task.id,
        task.id,
        if available.is_empty() {
            "无".to_string()
        } else {
            available
        },
        queue.conflict_policy
    )
}

fn join_chinese_sentences(values: &[String]) -> String {
    values
        .iter()
        .map(|value| value.trim().trim_end_matches(['。', '；', ';']).to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("；")
}

fn push_event(
    queue: &mut AgentWorkQueue,
    action: &str,
    worker_id: Option<String>,
    task_id: Option<String>,
    message: impl Into<String>,
) {
    queue.events.push(AgentWorkEvent {
        order: queue.events.len() + 1,
        timestamp_unix_seconds: current_unix_seconds(),
        action: action.to_string(),
        worker_id,
        task_id,
        message: message.into(),
    });
}

fn validate_worker_id(worker_id: &str) -> Result<(), AgentWorkError> {
    if valid_identifier(worker_id) {
        Ok(())
    } else {
        Err(AgentWorkError::InvalidWorkerId {
            worker_id: worker_id.to_string(),
        })
    }
}

fn validate_task_id(task_id: &str) -> Result<(), AgentWorkError> {
    if valid_identifier(task_id) {
        Ok(())
    } else {
        Err(AgentWorkError::InvalidTaskId {
            task_id: task_id.to_string(),
        })
    }
}

fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

fn normalize_goal(goal: &str) -> String {
    if goal.trim().is_empty() {
        "协调多个 AI 线程完成受控代码修改".to_string()
    } else {
        goal.trim().to_string()
    }
}

fn default_summary(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value.trim().to_string()
    }
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
