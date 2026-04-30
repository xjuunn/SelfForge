use std::error::Error;
use std::fmt;

const PROVIDER_ENV: &str = "SELFFORGE_AI_PROVIDER";
const HTTP_METHOD_POST: &str = "POST";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiConfigReport {
    pub providers: Vec<AiProviderStatus>,
    pub selected_provider: Option<String>,
    pub ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProviderStatus {
    pub id: String,
    pub display_name: String,
    pub selected: bool,
    pub configured: bool,
    pub api_key_env_var: Option<String>,
    pub accepted_api_key_env_vars: Vec<String>,
    pub model: String,
    pub model_source: String,
    pub base_url: String,
    pub base_url_source: String,
    pub protocol: String,
    pub request_path: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiRequestSpec {
    pub provider_id: String,
    pub model: String,
    pub protocol: String,
    pub method: String,
    pub url: String,
    pub auth_header_name: String,
    pub api_key_env_var: String,
    pub content_type: String,
    pub body: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiTextResponse {
    pub provider_id: String,
    pub model: String,
    pub protocol: String,
    pub text: String,
    pub raw_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiConfigError {
    UnknownProvider {
        requested: String,
        supported: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiRequestError {
    Config(AiConfigError),
    MissingProvider,
    MissingApiKey { provider: String },
    EmptyPrompt,
}

#[derive(Debug)]
pub enum AiResponseError {
    InvalidJson { source: serde_json::Error },
    MissingText { protocol: String },
    EmptyText { protocol: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiProviderProtocol {
    OpenAiResponses,
    OpenAiChatCompletions,
    GeminiGenerateContent,
}

#[derive(Debug, Clone, Copy)]
struct AiProviderDefinition {
    id: &'static str,
    display_name: &'static str,
    api_key_env_vars: &'static [&'static str],
    model_env_var: &'static str,
    base_url_env_var: &'static str,
    default_model: &'static str,
    default_base_url: &'static str,
    protocol: AiProviderProtocol,
}

pub struct AiProviderRegistry;

impl AiProviderRegistry {
    pub fn inspect_env() -> Result<AiConfigReport, AiConfigError> {
        inspect_with(|key| std::env::var(key).ok())
    }

    pub fn inspect_with<F>(lookup: F) -> Result<AiConfigReport, AiConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        inspect_with(lookup)
    }

    pub fn build_text_request_env(prompt: &str) -> Result<AiRequestSpec, AiRequestError> {
        build_text_request_with(prompt, |key| std::env::var(key).ok())
    }

    pub fn build_text_request_with<F>(
        prompt: &str,
        lookup: F,
    ) -> Result<AiRequestSpec, AiRequestError>
    where
        F: Fn(&str) -> Option<String>,
    {
        build_text_request_with(prompt, lookup)
    }

    pub fn parse_text_response(
        request: &AiRequestSpec,
        response_body: &str,
    ) -> Result<AiTextResponse, AiResponseError> {
        parse_text_response(request, response_body)
    }
}

impl AiConfigReport {
    pub fn selected(&self) -> Option<&AiProviderStatus> {
        let selected_provider = self.selected_provider.as_deref()?;
        self.providers
            .iter()
            .find(|provider| provider.id == selected_provider)
    }
}

impl fmt::Display for AiConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiConfigError::UnknownProvider {
                requested,
                supported,
            } => write!(
                formatter,
                "未知 AI 提供商 {requested}，支持的提供商：{}",
                supported.join(", ")
            ),
        }
    }
}

impl Error for AiConfigError {}

impl fmt::Display for AiRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiRequestError::Config(error) => write!(formatter, "{error}"),
            AiRequestError::MissingProvider => {
                write!(formatter, "没有可用 AI 提供商，请先配置 API Key 环境变量")
            }
            AiRequestError::MissingApiKey { provider } => {
                write!(formatter, "AI 提供商 {provider} 未配置 API Key 环境变量")
            }
            AiRequestError::EmptyPrompt => write!(formatter, "AI 请求提示词不能为空"),
        }
    }
}

impl Error for AiRequestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiRequestError::Config(error) => Some(error),
            AiRequestError::MissingProvider
            | AiRequestError::MissingApiKey { .. }
            | AiRequestError::EmptyPrompt => None,
        }
    }
}

impl fmt::Display for AiResponseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiResponseError::InvalidJson { source } => {
                write!(formatter, "AI 响应不是合法 JSON：{source}")
            }
            AiResponseError::MissingText { protocol } => {
                write!(formatter, "AI 响应缺少可解析文本，协议：{protocol}")
            }
            AiResponseError::EmptyText { protocol } => {
                write!(formatter, "AI 响应文本为空，协议：{protocol}")
            }
        }
    }
}

impl Error for AiResponseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiResponseError::InvalidJson { source } => Some(source),
            AiResponseError::MissingText { .. } | AiResponseError::EmptyText { .. } => None,
        }
    }
}

impl From<AiConfigError> for AiRequestError {
    fn from(error: AiConfigError) -> Self {
        AiRequestError::Config(error)
    }
}

fn parse_text_response(
    request: &AiRequestSpec,
    response_body: &str,
) -> Result<AiTextResponse, AiResponseError> {
    let value: serde_json::Value = serde_json::from_str(response_body)
        .map_err(|source| AiResponseError::InvalidJson { source })?;
    let Some(text) = response_text(&request.protocol, &value) else {
        return Err(AiResponseError::MissingText {
            protocol: request.protocol.clone(),
        });
    };
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err(AiResponseError::EmptyText {
            protocol: request.protocol.clone(),
        });
    }

    Ok(AiTextResponse {
        provider_id: request.provider_id.clone(),
        model: request.model.clone(),
        protocol: request.protocol.clone(),
        text,
        raw_bytes: response_body.len(),
    })
}

fn response_text(protocol: &str, value: &serde_json::Value) -> Option<String> {
    match protocol {
        "openai-responses" => openai_response_text(value),
        "openai-chat-completions" => chat_completion_text(value),
        "gemini-generate-content" => gemini_response_text(value),
        _ => None,
    }
}

fn openai_response_text(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value.get("output_text").and_then(serde_json::Value::as_str) {
        return Some(text.to_string());
    }

    let output = value.get("output")?.as_array()?;
    let mut parts = Vec::new();
    for item in output {
        if let Some(content) = item.get("content").and_then(serde_json::Value::as_array) {
            collect_text_parts(content, &mut parts);
        }
    }
    non_empty_join(parts)
}

fn chat_completion_text(value: &serde_json::Value) -> Option<String> {
    value
        .get("choices")?
        .as_array()?
        .first()?
        .get("message")?
        .get("content")?
        .as_str()
        .map(str::to_string)
}

fn gemini_response_text(value: &serde_json::Value) -> Option<String> {
    let candidates = value.get("candidates")?.as_array()?;
    let mut parts = Vec::new();
    for candidate in candidates {
        if let Some(content_parts) = candidate
            .get("content")
            .and_then(|content| content.get("parts"))
            .and_then(serde_json::Value::as_array)
        {
            collect_text_parts(content_parts, &mut parts);
        }
    }
    non_empty_join(parts)
}

fn collect_text_parts(values: &[serde_json::Value], parts: &mut Vec<String>) {
    for value in values {
        if let Some(text) = value.get("text").and_then(serde_json::Value::as_str) {
            parts.push(text.to_string());
        }
    }
}

fn non_empty_join(parts: Vec<String>) -> Option<String> {
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

impl AiProviderProtocol {
    fn as_str(self) -> &'static str {
        match self {
            AiProviderProtocol::OpenAiResponses => "openai-responses",
            AiProviderProtocol::OpenAiChatCompletions => "openai-chat-completions",
            AiProviderProtocol::GeminiGenerateContent => "gemini-generate-content",
        }
    }

    fn request_path(self, model: &str) -> String {
        match self {
            AiProviderProtocol::OpenAiResponses => "/responses".to_string(),
            AiProviderProtocol::OpenAiChatCompletions => "/chat/completions".to_string(),
            AiProviderProtocol::GeminiGenerateContent => {
                format!("/models/{model}:generateContent")
            }
        }
    }
}

fn inspect_with<F>(lookup: F) -> Result<AiConfigReport, AiConfigError>
where
    F: Fn(&str) -> Option<String>,
{
    let requested_provider = clean_env_value(lookup(PROVIDER_ENV))
        .map(|provider| provider.trim().to_ascii_lowercase().replace('_', "-"));
    if let Some(requested) = &requested_provider {
        if !provider_definitions()
            .iter()
            .any(|definition| definition.id == requested)
        {
            return Err(AiConfigError::UnknownProvider {
                requested: requested.clone(),
                supported: provider_definitions()
                    .iter()
                    .map(|definition| definition.id.to_string())
                    .collect(),
            });
        }
    }

    let mut providers = provider_definitions()
        .iter()
        .map(|definition| inspect_provider(definition, &lookup))
        .collect::<Vec<_>>();

    let selected_provider = requested_provider.or_else(|| {
        providers
            .iter()
            .find(|provider| provider.configured)
            .map(|provider| provider.id.clone())
    });
    for provider in &mut providers {
        provider.selected = selected_provider
            .as_deref()
            .is_some_and(|selected| selected == provider.id);
    }
    let ready = providers
        .iter()
        .any(|provider| provider.selected && provider.configured);

    Ok(AiConfigReport {
        providers,
        selected_provider,
        ready,
    })
}

fn build_text_request_with<F>(prompt: &str, lookup: F) -> Result<AiRequestSpec, AiRequestError>
where
    F: Fn(&str) -> Option<String>,
{
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err(AiRequestError::EmptyPrompt);
    }

    let report = inspect_with(lookup)?;
    let provider = report.selected().ok_or(AiRequestError::MissingProvider)?;
    let api_key_env_var =
        provider
            .api_key_env_var
            .clone()
            .ok_or_else(|| AiRequestError::MissingApiKey {
                provider: provider.id.clone(),
            })?;
    let body = request_body(provider, prompt);

    Ok(AiRequestSpec {
        provider_id: provider.id.clone(),
        model: provider.model.clone(),
        protocol: provider.protocol.clone(),
        method: HTTP_METHOD_POST.to_string(),
        url: join_url(&provider.base_url, &provider.request_path),
        auth_header_name: auth_header_name(provider),
        api_key_env_var,
        content_type: "application/json".to_string(),
        body,
    })
}

fn request_body(provider: &AiProviderStatus, prompt: &str) -> serde_json::Value {
    match provider.protocol.as_str() {
        "openai-responses" => serde_json::json!({
            "model": provider.model,
            "input": prompt
        }),
        "openai-chat-completions" => serde_json::json!({
            "model": provider.model,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "stream": false
        }),
        "gemini-generate-content" => serde_json::json!({
            "contents": [
                {
                    "parts": [
                        {
                            "text": prompt
                        }
                    ]
                }
            ]
        }),
        _ => serde_json::json!({}),
    }
}

fn auth_header_name(provider: &AiProviderStatus) -> String {
    match provider.protocol.as_str() {
        "gemini-generate-content" => "x-goog-api-key".to_string(),
        _ => "Authorization".to_string(),
    }
}

fn join_url(base_url: &str, request_path: &str) -> String {
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        request_path.trim_start_matches('/')
    )
}

fn inspect_provider<F>(definition: &AiProviderDefinition, lookup: &F) -> AiProviderStatus
where
    F: Fn(&str) -> Option<String>,
{
    let api_key_env_var = definition
        .api_key_env_vars
        .iter()
        .find(|env_var| clean_env_value(lookup(env_var)).is_some())
        .map(|env_var| (*env_var).to_string());
    let (model, model_source) = value_with_source(
        lookup(definition.model_env_var),
        definition.model_env_var,
        definition.default_model,
    );
    let (base_url, base_url_source) = value_with_source(
        lookup(definition.base_url_env_var),
        definition.base_url_env_var,
        definition.default_base_url,
    );
    let request_path = definition.protocol.request_path(&model);

    AiProviderStatus {
        id: definition.id.to_string(),
        display_name: definition.display_name.to_string(),
        selected: false,
        configured: api_key_env_var.is_some(),
        api_key_env_var,
        accepted_api_key_env_vars: definition
            .api_key_env_vars
            .iter()
            .map(|env_var| (*env_var).to_string())
            .collect(),
        model,
        model_source,
        base_url,
        base_url_source,
        protocol: definition.protocol.as_str().to_string(),
        request_path,
    }
}

fn value_with_source(
    value: Option<String>,
    env_var: &str,
    default_value: &str,
) -> (String, String) {
    match clean_env_value(value) {
        Some(value) => (value, env_var.to_string()),
        None => (default_value.to_string(), "默认值".to_string()),
    }
}

fn clean_env_value(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn provider_definitions() -> &'static [AiProviderDefinition] {
    &[
        AiProviderDefinition {
            id: "openai",
            display_name: "OpenAI",
            api_key_env_vars: &["OPENAI_API_KEY"],
            model_env_var: "OPENAI_MODEL",
            base_url_env_var: "OPENAI_BASE_URL",
            default_model: "gpt-5.2",
            default_base_url: "https://api.openai.com/v1",
            protocol: AiProviderProtocol::OpenAiResponses,
        },
        AiProviderDefinition {
            id: "deepseek",
            display_name: "DeepSeek",
            api_key_env_vars: &["DEEPSEEK_API_KEY"],
            model_env_var: "DEEPSEEK_MODEL",
            base_url_env_var: "DEEPSEEK_BASE_URL",
            default_model: "deepseek-v4-flash",
            default_base_url: "https://api.deepseek.com",
            protocol: AiProviderProtocol::OpenAiChatCompletions,
        },
        AiProviderDefinition {
            id: "gemini",
            display_name: "Gemini",
            api_key_env_vars: &["GOOGLE_API_KEY", "GEMINI_API_KEY"],
            model_env_var: "GEMINI_MODEL",
            base_url_env_var: "GEMINI_BASE_URL",
            default_model: "gemini-2.5-flash",
            default_base_url: "https://generativelanguage.googleapis.com/v1beta",
            protocol: AiProviderProtocol::GeminiGenerateContent,
        },
    ]
}
