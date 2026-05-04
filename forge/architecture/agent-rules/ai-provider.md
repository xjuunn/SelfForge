# AI Provider 规则

# 配置来源

AI 配置只允许来自真实进程环境变量或项目根目录 `.env`。真实环境变量优先级高于 `.env`。

支持变量：

```txt
SELFFORGE_AI_PROVIDER
OPENAI_API_KEY
DEEPSEEK_API_KEY
GEMINI_API_KEY
GOOGLE_API_KEY
OPENAI_MODEL
DEEPSEEK_MODEL
GEMINI_MODEL
OPENAI_BASE_URL
DEEPSEEK_BASE_URL
GEMINI_BASE_URL
```

禁止把 API Key 写入源码、Markdown、日志、状态文件或运行记录。

# 检查配置

使用：

```txt
cargo run -- ai-config
```

输出只能显示密钥是否存在和来源变量名，禁止输出密钥值。

# PowerShell 设置方式

Windows PowerShell 必须使用：

```powershell
$env:SELFFORGE_AI_PROVIDER="deepseek"
$env:DEEPSEEK_API_KEY="密钥"
```

只写 `SELFFORGE_AI_PROVIDER=deepseek` 不会传递给 `cargo run` 子进程。

# 请求规则

1. AI 请求优先使用 `ai-request [prompt]` 或应用层统一请求规格。
2. 审计请求结构使用 `ai-request --dry-run [prompt]`。
3. 真实请求必须设置超时。
4. 默认不打印完整请求体，避免泄露敏感提示词。
5. Provider 端点、认证头、请求体、HTTP 执行和响应解析必须集中在 AI Provider 模块。
6. 业务流程只能消费归一化文本结果。
7. 解析失败必须返回明确错误，禁止静默用空文本继续推进。
