# V2-D Structured Views 设计

## 目标

本文档定义 Viden 在 V2-D 阶段的第一批终端呈现层设计。目标是在不改变命令语义、工具执行、权限行为和 transcript 存储模型的前提下，让现有命令输出更容易扫描和理解。

这一批的确认方向如下：

- 先只处理 LSP 相关输出
- 保持现有 REPL 和命令执行链路不变
- 在 `viden-core` 内增加一个面向 presentation 的模块
- 改进输出结构，而不是扩张功能范围
- full-screen TUI 推迟到文本渲染层稳定之后

## 产品目标

Viden 不仅要能输出正确的开发信息，还要能以适合日常终端使用的方式把这些信息呈现出来。

在这一批里，用户执行 LSP 命令时应该能快速回答：

- 结果属于哪个文件
- 问题或引用位于什么位置
- 当前展示的是哪种 symbol
- 这一行是否属于某个 container，例如函数或模块

输出仍然是纯文本终端输出，但读起来应当像“结构化视图”，而不是原始行文本堆叠。

## 范围

范围内：

- 为以下命令提供结构化渲染：
  - `/lsp diagnostics`
  - `/lsp symbols`
  - `/lsp references`
- 在 `viden-core` 中新增可复用的 presentation helper 模块
- 增加锁定输出形状的 renderer 测试
- 尽量保持路径按相对 `cwd` 渲染

范围外：

- full-screen TUI
- 新的 UI crate 或依赖
- 修改 slash command 名称或参数
- 修改 tool contract
- 修改 transcript schema
- 在第一批中处理 tasks、memory、sessions、diff、approval 的渲染

## 架构

### 模块边界

新增一个 presentation 导向的内部模块：

- `viden-core/src/presentation.rs`

职责：

- 小型文本渲染 helper
- section 和 subsection 的格式化
- 结构化命令输出的分组和行拼接

不负责：

- 命令解析
- 业务逻辑
- 工具执行
- transcript 写入
- 重度 ANSI 终端行为

`viden-core/src/lib.rs` 仍然是命令路由入口。它继续负责命令分发和领域相关的渲染决策，但把通用格式化行为委托给 `presentation.rs`。

### 职责拆分

- `viden-core`
  负责命令处理、领域感知的格式化，以及最终命令输出
- `presentation.rs`
  负责可复用的文本布局 helper
- `viden-cli`
  继续保持轻量输出层，不在这一批里增加命令特定的 view 逻辑

这样可以保持当前的核心边界：命令输出仍通过 engine 内部统一生成，并走共享 runtime path 对外输出。

## 目标行为

### Diagnostics

`/lsp diagnostics` 应当：

- 按相对文件路径分组
- 每个文件用一个 header，后面跟缩进条目
- 用稳定的 `line:character` 形式展示位置
- 在有值时保留 severity、source 和 code
- 当同一文件有多个 diagnostic 时仍然易读

示例形状：

```text
Diagnostics:
src/lib.rs:
  2:4 error [rust-analyzer/clippy] unused variable
  8:1 warning [rust-analyzer] dead code
```

### Symbols

`/lsp symbols` 应当：

- 每个 symbol 保持一行
- 用可读的 kind 标签替代原始数字
- 展示相对路径
- 在已知 container 时显示 `in <container>`
- 优先保证紧凑和可扫描，而不是冗长的多行块

示例形状：

```text
Symbols:
src/lib.rs:
  main [function] 3:1
  value [variable] 4:5 in main
```

### References

`/lsp references` 应当：

- 保持稳定排序和去重结果
- 尽量使用相对路径
- 每个引用一行，并使用紧凑的位置格式
- 避免每一行都重复无意义的冗余文案

示例形状：

```text
References:
  src/lib.rs:4:5
  src/engine.rs:18:9
```

## 数据与 API 影响

这一批不会新增持久化状态，也不会改变对外的 tool contract。

预期代码影响：

- 在 `presentation.rs` 中新增内部 helper 函数
- 修改现有：
  - `render_lsp_diagnostics`
  - `render_lsp_symbols`
  - `render_lsp_locations`

无需修改：

- `viden-tools`
- `viden-lsp`
- `viden-session`
- `viden-permissions`

## 测试策略

测试应先锁定 view 形状，再修改实现。

必需覆盖：

- diagnostics 能按文件分组渲染
- diagnostics 保留 severity/source/code 和相对路径格式
- symbols 展示可读的 kind 标签
- symbols 在有 container 时展示上下文
- references 保持相对路径和稳定顺序
- 为 section title 和基础行拼接提供 presentation helper 测试

这一批的验证命令：

```bash
cargo test -p viden-core render_lsp_
cargo test -p viden-core presentation
```

收尾前执行：

```bash
cargo test -p viden-core
cargo test --workspace --quiet
```

## 风险与约束

约束：

- 不新增渲染依赖
- 输出必须保持 transcript-safe 的纯文本
- diff 保持小而可逆

风险：

- 过早把 presentation 抽象得太重
- 不小心把业务逻辑混入 formatter helper
- 输出虽然更好看，但测试稳定性变差

缓解方式：

- helper 保持窄职责
- 分组决策仍靠近现有 LSP renderer
- 用聚焦测试锁定输出形状，再做更大范围重构

## 后续工作

如果这一批成功，后续 V2-D 工作可以把同一套 presentation layer 扩展到：

- `/sessions`
- `/tasks`
- `/memory`
- `/diff`
- approval prompts

后续仍保持同一边界：

- `viden-core` 负责结构化文本输出
- `viden-cli` 继续保持轻量终端壳层
