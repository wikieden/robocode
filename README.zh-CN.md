# RoboCode

RoboCode 是一个本地优先的编程 Agent CLI，提供 cockpit 风格 TUI、带权限控制的工具执行，以及多模型 provider 支持。

英文版： [README.md](README.md)

![RoboCode TUI 系统截图](docs/previews/robocode-tui-system-screenshot.svg)

## 产品特色

- Cockpit TUI：在一个终端屏幕里同时查看对话、工具调用、审批、诊断、workspace 上下文、任务状态和 provider 健康度。
- 本地优先：transcript、任务状态和项目记忆默认保存在本机。
- 权限感知编辑：文件、shell、Git 和 workflow 变更都会先经过权限模式和审批，再影响工作区。
- 多 provider 支持：DeepSeek、OpenAI、Anthropic、OpenAI-compatible gateways、Ollama，以及离线 fallback provider。
- 面向开发工作流的内置工具：文件读写编辑、搜索、shell、Web、Git、会话恢复、任务、记忆和 LSP 诊断。
- 多平台二进制发布：支持 macOS、Linux 和 Windows。

## 安装

从 [RoboCode v0.1.3](https://github.com/wikieden/robocode/releases/tag/v0.1.3)
下载 release 压缩包。

当前 release targets：

- `aarch64-apple-darwin`：Apple Silicon macOS
- `x86_64-apple-darwin`：Intel macOS
- `x86_64-unknown-linux-gnu`：Linux x64
- `x86_64-pc-windows-msvc`：Windows x64

macOS 或 Linux 安装：

```bash
VERSION=0.1.3
TARGET=aarch64-apple-darwin
curl -L -O "https://github.com/wikieden/robocode/releases/download/v${VERSION}/robocode-v${VERSION}-${TARGET}.tar.gz"
tar -xzf "robocode-v${VERSION}-${TARGET}.tar.gz"
sudo install -m 755 "robocode-v${VERSION}-${TARGET}/robocode-cli" /usr/local/bin/robocode-cli
robocode-cli --help
```

Windows PowerShell 安装：

```powershell
$Version = "0.1.3"
$Target = "x86_64-pc-windows-msvc"
Invoke-WebRequest "https://github.com/wikieden/robocode/releases/download/v$Version/robocode-v$Version-$Target.tar.gz" -OutFile "robocode-v$Version-$Target.tar.gz"
tar -xzf "robocode-v$Version-$Target.tar.gz"
New-Item -ItemType Directory -Force "$env:USERPROFILE\bin"
Copy-Item "robocode-v$Version-$Target\robocode-cli.exe" "$env:USERPROFILE\bin\robocode-cli.exe"
$env:PATH += ";$env:USERPROFILE\bin"
robocode-cli.exe --help
```

## 使用

运行离线 smoke test：

```bash
robocode-cli --provider fallback --model test-local
```

使用 fallback provider 启动 TUI：

```bash
robocode-cli --tui --provider fallback --model test-local
```

使用 DeepSeek V4 Flash 启动 TUI：

```bash
export DEEPSEEK_API_KEY="sk-..."
robocode-cli --tui --provider deepseek --model deepseek-v4-flash
```

开发时从源码启动：

```bash
cargo run -p robocode-cli -- --tui --provider fallback --model test-local
```

使用显式配置文件：

```bash
robocode-cli --config .robocode/config.toml
```

provider 配置示例：

```toml
provider = "deepseek"
model = "deepseek-v4-flash"
permission_mode = "acceptEdits"
request_timeout_secs = 120
max_retries = 2

[providers.deepseek]
api_base = "https://api.deepseek.com"
api_key_env = "DEEPSEEK_API_KEY"
```

## TUI 操作

- `Enter` 提交输入区；`Ctrl-J` 是显式发送动作。
- `Ctrl-K` 清空输入区，`Ctrl-R` 重新生成，`Ctrl-N` 开启新任务。
- `?` 打开 TUI 内帮助。
- `Esc` 或 `Ctrl-C` 退出；`/quit` 和 `/exit` 也可以关闭 TUI。
- 审批弹窗默认停在 `Approve`；按 `y` 通过，`n` 拒绝，`d` 聚焦 diff，也可以用 `Tab` / 方向键在动作间移动。
- Slash commands 以 `/` 开头；常用入口包括 `/help`、`/provider`、`/status`、`/config`、`/permissions`、`/sessions`、`/resume latest`、`/task`、`/memory`。

## 问题反馈

请通过 [GitHub Issues](https://github.com/wikieden/robocode/issues)
反馈 bug 和功能建议。

提交 issue 时建议包含：

- RoboCode 版本号或 release asset 名称。
- 操作系统和终端应用。
- provider 和 model，例如 `deepseek / deepseek-v4-flash`。
- 执行的命令和最小复现步骤。
- 相关日志或截图，注意先移除 API key 和私有路径。

## 文档

README 只保留产品介绍和使用入口。架构与实现细节放在文档目录：

- [架构](docs/architecture.zh-CN.md)
- [产品需求](docs/product-requirements.zh-CN.md)
- [阶段路线图](docs/staged-roadmap.zh-CN.md)
- [参考工程分析](docs/reference-analysis.zh-CN.md)
- [Provider 真实调用矩阵](docs/provider-live-matrix.zh-CN.md)
- [TUI Cockpit 设计](docs/tui-cockpit-design.zh-CN.md)

维护者可以用下面命令在本地构建 release 压缩包：

```bash
scripts/package-release.sh
```

运行 workspace 测试：

```bash
cargo test --workspace
```

## 授权

RoboCode 使用 MIT License 发布。详见 [LICENSE](LICENSE)。
