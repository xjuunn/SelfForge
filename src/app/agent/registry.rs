use super::types::{AgentCapability, AgentDefinition, AgentError, AgentPlan, AgentPlanStep};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRegistry {
    agents: Vec<AgentDefinition>,
}

impl AgentRegistry {
    pub fn new(agents: Vec<AgentDefinition>) -> Result<Self, AgentError> {
        validate_agents(&agents)?;
        Ok(Self { agents })
    }

    pub fn standard() -> Self {
        Self::new(vec![
            AgentDefinition::new(
                "architect",
                "架构 Agent",
                "负责边界设计、模块拆分、约束解释和演进路线",
                vec![AgentCapability::Architecture, AgentCapability::Planning],
                vec!["目标", "历史记忆", "系统约束"],
                vec!["架构决策", "可验证计划"],
            ),
            AgentDefinition::new(
                "builder",
                "实现 Agent",
                "负责在既定边界内完成最小必要代码修改",
                vec![AgentCapability::Implementation, AgentCapability::Runtime],
                vec!["计划", "代码上下文", "接口约束"],
                vec!["代码变更", "运行记录"],
            ),
            AgentDefinition::new(
                "verifier",
                "验证 Agent",
                "负责测试设计、执行验证、边界用例和失败复现",
                vec![AgentCapability::Testing, AgentCapability::Runtime],
                vec!["代码变更", "验收标准"],
                vec!["测试结果", "验证报告"],
            ),
            AgentDefinition::new(
                "reviewer",
                "审查 Agent",
                "负责风险审查、架构一致性检查和回归风险识别",
                vec![AgentCapability::Review, AgentCapability::Architecture],
                vec!["代码变更", "测试结果", "系统约束"],
                vec!["审查结论", "风险清单"],
            ),
            AgentDefinition::new(
                "archivist",
                "归档 Agent",
                "负责记忆、任务、错误和版本记录的中文归档",
                vec![AgentCapability::Documentation, AgentCapability::Memory],
                vec!["计划", "执行结果", "测试结果", "错误信息"],
                vec!["记忆记录", "任务记录", "版本记录"],
            ),
        ])
        .expect("标准 Agent 注册表必须保持有效")
    }

    pub fn agents(&self) -> &[AgentDefinition] {
        &self.agents
    }

    pub fn find(&self, id: &str) -> Option<&AgentDefinition> {
        self.agents.iter().find(|agent| agent.id == id)
    }

    pub fn agent_for(&self, capability: AgentCapability) -> Result<&AgentDefinition, AgentError> {
        self.agents
            .iter()
            .find(|agent| agent.has_capability(capability))
            .ok_or(AgentError::MissingCapability { capability })
    }

    pub fn plan_for_goal(&self, goal: &str) -> Result<AgentPlan, AgentError> {
        let goal = goal.trim();
        if goal.is_empty() {
            return Err(AgentError::EmptyGoal);
        }

        let step_specs = [
            StepSpec {
                capability: AgentCapability::Planning,
                title: "读取记忆并拆解目标",
                inputs: &["目标", "最近记忆", "系统约束"],
                outputs: &["任务边界", "执行顺序"],
                verification: "计划步骤必须有明确顺序和验收条件",
            },
            StepSpec {
                capability: AgentCapability::Architecture,
                title: "确认模块边界和扩展点",
                inputs: &["任务边界", "现有代码结构"],
                outputs: &["模块边界", "接口契约"],
                verification: "新增能力必须进入合适应用层模块",
            },
            StepSpec {
                capability: AgentCapability::Implementation,
                title: "完成最小必要实现",
                inputs: &["接口契约", "代码上下文"],
                outputs: &["代码变更", "局部运行结果"],
                verification: "实现必须保持小步可回滚",
            },
            StepSpec {
                capability: AgentCapability::Testing,
                title: "执行单元、边界和错误测试",
                inputs: &["代码变更", "验收条件"],
                outputs: &["测试结果", "失败记录"],
                verification: "测试失败必须停止提升版本",
            },
            StepSpec {
                capability: AgentCapability::Review,
                title: "审查风险和架构一致性",
                inputs: &["代码变更", "测试结果"],
                outputs: &["审查结论", "风险处理建议"],
                verification: "不得引入越界修改和未解释风险",
            },
            StepSpec {
                capability: AgentCapability::Documentation,
                title: "写入记忆、任务、错误和版本归档",
                inputs: &["执行结果", "测试结果", "审查结论"],
                outputs: &["中文归档", "版本记录"],
                verification: "归档必须写入当前 major 聚合文件",
            },
        ];

        let mut selected_agent_ids = HashSet::new();
        let mut selected_agents = Vec::new();
        let mut steps = Vec::new();
        for (index, spec) in step_specs.iter().enumerate() {
            let agent = self.agent_for(spec.capability)?;
            if selected_agent_ids.insert(agent.id.clone()) {
                selected_agents.push(agent.clone());
            }
            steps.push(AgentPlanStep {
                order: index + 1,
                agent_id: agent.id.clone(),
                title: spec.title.to_string(),
                capability: spec.capability,
                inputs: spec
                    .inputs
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect(),
                outputs: spec
                    .outputs
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect(),
                verification: spec.verification.to_string(),
            });
        }

        Ok(AgentPlan {
            goal: goal.to_string(),
            agents: selected_agents,
            steps,
        })
    }
}

struct StepSpec {
    capability: AgentCapability,
    title: &'static str,
    inputs: &'static [&'static str],
    outputs: &'static [&'static str],
    verification: &'static str,
}

fn validate_agents(agents: &[AgentDefinition]) -> Result<(), AgentError> {
    let mut ids = HashSet::new();
    for agent in agents {
        let id = agent.id.trim();
        if id.is_empty() {
            return Err(AgentError::EmptyAgentId);
        }
        if !ids.insert(id.to_string()) {
            return Err(AgentError::DuplicateAgent { id: id.to_string() });
        }
    }

    Ok(())
}
