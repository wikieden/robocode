# Agent Harness 方向

## 目的

本文记录 2026-08-16 DeepSeek Harness v0.1 对标调研后被接受的后续计划。
DeepSeek Harness 独立验证了 Viden 的三个架构判断——append-only 会话事实作为
唯一事实来源、单一 Core 契约服务多前端、权限检查先于变更。下列条目把其余
洞察转化为 Viden 的开发方向。每一条都带有明确状态；只有标注**已实现**的条目
描述已验证的行为。

英文对照：[harness-direction.md](harness-direction.md)。

## 1. 「模型可见即已记录」（已实现：契约测试）

不变量：进入 provider 请求的每个字节，必须仅凭 append-only JSONL transcript
即可重建。

- 状态：契约测试已落地于
  `crates/runtime/src/tests/transcript_contract_tests.rs`。测试运行一个真实
  turn（含工具调用与拒绝路径），从 JSONL transcript 重建会话，并要求两侧
  历史的模型可见投影（role、content、tool name、tool call id）完全一致。
- 待扩展：provider 请求还会合并请求时构建的 context-bundle 投影。把不变量
  扩展到 context 投影（记录或可确定性重推导）是 Core 契约候选，列入下方
  排期。

## 2. viden-tools 的 OS 能力接缝（已接受，已排期）

DeepSeek Harness 换一个 filesystem/subprocess provider，Bash、PTY、LSP 就
一起迁移到远程沙箱，工具零改动。Viden 的 `ToolExecutionContext` 目前只暴露
`cwd` 和语义（LSP）provider，各工具直接调用 `std::fs` 和进程 spawn。

计划形态：

- 在 `crates/tools` 中与 `SemanticToolProvider` 并列定义 filesystem 与
  process 能力 trait；
- 默认实现精确保持现有本地行为；
- 从文件与 shell 工具开始，增量迁移工具到该接缝；
- 之后沙箱或远程执行只是换 provider，而不是重写工具。

趁工具面还小时做；注册表长大后再改造是昂贵路径。

## 3. 工具执行 Pre/Post 接缝（已接受，已排期）

`ToolRegistry::execute` 目前直接分发；权限检查发生在执行前的调用点。在
registry 层加 pre-execute/post-execute 接缝，可让权限、成本计量、evidence
采集统一挂接，而不是依赖每个调用点自觉。「权限先于变更」从纪律变成结构性
保证。范围：纯增量，默认行为不变；`handle_tool_call` 的运行时调用点是第一个
消费者。

## 4. 可复现评测的最小工具预设（提案）

一个只暴露 `shell` 与文件编辑工具的注册表预设，对应 DeepSeek Harness 的
Minimal 模式所镜像的学术 harness 基线。用途：可对比的 SWE-bench /
Terminal-Bench 类评测，以及对 context 引擎效果的低成本 A/B 隔离。因为
`ToolRegistry` 是注册制，成本很低。归属：Core；作为 Core 拥有的配置面，
而非前端开关。

## 5. 代码编排模式（提案，长期）

DeepSeek Harness 的 Code 模式生成带类型的 SDK，让模型写一段程序而不是发起
多次往返工具调用。对 Viden 这是未来的 Core 契约候选（新工具族加权限方案），
不是近期工作。仅记录；在 V3 契约冻结完成前不启动。

## 战略立场

「一切皆插件」的复杂度税（无类型跨插件注入、加载顺序冲突）是社区对
DeepSeek Harness 的主要批评。Viden 的差异化是同样的可组合性判断，但接缝在
编译期检查、插件面有界。保持接缝少、有类型、由 Core 拥有；不追求动态插件
的对等能力。

## 排期

1. 第 1 条的扩展（覆盖 context 投影）——下一次 Core 契约讨论。
2. 第 2 条——随下一个 `crates/tools` 变更集排期。
3. 第 3 条——随下一个 tools/permissions 变更集排期。
4. 第 4、5 条——在 `frontend-contract-v1` checkpoint 交付后再评估。
