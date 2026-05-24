# RoboCode

RoboCode is a local-first coding agent CLI with a cockpit-style terminal UI,
permission-aware tool execution, and multi-provider model support.

Chinese version: [README.zh-CN.md](README.zh-CN.md)

![RoboCode TUI system screenshot](docs/previews/robocode-tui-system-screenshot.svg)

## Highlights

- Cockpit TUI for chat, tool calls, approvals, diagnostics, workspace context,
  active tasks, and provider health in one terminal screen.
- Local-first runtime: transcripts, task state, and project memory stay on your
  machine by default.
- Permission-aware editing: file, shell, Git, and workflow mutations pass
  through approval modes before they touch your workspace.
- Multi-provider model access: DeepSeek, OpenAI, Anthropic, OpenAI-compatible
  gateways, Ollama, and an offline fallback provider.
- Developer workflow tools: file read/write/edit, search, shell, web, Git,
  sessions, resume, tasks, memory, and LSP diagnostics.
- Multi-platform release binaries for macOS, Linux, and Windows.

## Install

Download a release archive from
[RoboCode v0.1.3](https://github.com/wikieden/robocode/releases/tag/v0.1.3).

Available release targets:

- `aarch64-apple-darwin` for Apple Silicon macOS
- `x86_64-apple-darwin` for Intel macOS
- `x86_64-unknown-linux-gnu` for Linux x64
- `x86_64-pc-windows-msvc` for Windows x64

Install on macOS or Linux:

```bash
VERSION=0.1.3
TARGET=aarch64-apple-darwin
curl -L -O "https://github.com/wikieden/robocode/releases/download/v${VERSION}/robocode-v${VERSION}-${TARGET}.tar.gz"
tar -xzf "robocode-v${VERSION}-${TARGET}.tar.gz"
sudo install -m 755 "robocode-v${VERSION}-${TARGET}/robocode-cli" /usr/local/bin/robocode-cli
robocode-cli --help
```

Install on Windows PowerShell:

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

## Usage

Run an offline smoke test:

```bash
robocode-cli --provider fallback --model test-local
```

Start the TUI with the fallback provider:

```bash
robocode-cli --tui --provider fallback --model test-local
```

Start the TUI with DeepSeek V4 Flash:

```bash
export DEEPSEEK_API_KEY="sk-..."
robocode-cli --tui --provider deepseek --model deepseek-v4-flash
```

Start from source during development:

```bash
cargo run -p robocode-cli -- --tui --provider fallback --model test-local
```

Use an explicit config file:

```bash
robocode-cli --config .robocode/config.toml
```

Example provider config:

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

## TUI Controls

- `Enter` submits the composer; `Ctrl-J` is the explicit send action.
- `Ctrl-K` clears the composer, `Ctrl-R` regenerates, and `Ctrl-N` starts a new task.
- `?` opens the in-TUI help surface.
- `Esc` or `Ctrl-C` exits; `/quit` and `/exit` also close the TUI.
- Approval prompts default to `Approve`; press `y` to approve, `n` to deny,
  `d` to focus diff, or use `Tab` / arrow keys to move between actions.
- Slash commands start with `/`; useful starters include `/help`, `/provider`,
  `/status`, `/config`, `/permissions`, `/sessions`, `/resume latest`, `/task`,
  and `/memory`.

## Feedback

Please report bugs and feature requests through
[GitHub Issues](https://github.com/wikieden/robocode/issues).

Helpful issue details:

- RoboCode version or release asset name.
- Operating system and terminal app.
- Provider and model, for example `deepseek / deepseek-v4-flash`.
- The command you ran and the smallest reproduction steps.
- Relevant logs or screenshots, with API keys and private paths redacted.

## Documentation

README stays focused on product usage. Architecture and implementation details
live in the docs:

- [Architecture](docs/architecture.md)
- [Product Requirements](docs/product-requirements.md)
- [Staged Roadmap](docs/staged-roadmap.md)
- [Reference Analysis](docs/reference-analysis.md)
- [Provider Live Matrix](docs/provider-live-matrix.md)
- [TUI Cockpit Design](docs/tui-cockpit-design.md)

Maintainers can build a release archive locally with:

```bash
scripts/package-release.sh
```

Run the workspace test suite:

```bash
cargo test --workspace
```

## License

RoboCode is released under the MIT License. See [LICENSE](LICENSE).
