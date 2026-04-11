# 参考工程分析

英文版： [reference-analysis.md](reference-analysis.md)

这份文档总结了 `.ref/claude-code-main` 中对 Rust 重实现真正重要的部分。

## 参考工程是什么

- 一个大型的 TypeScript 终端 Agent 平台，核心围绕共享 query loop 展开
- 工具执行、权限、会话持久化、slash commands 都是运行时的一等概念
- 很多部分偏产品化和平台化，比如 remote bridge、MCP、voice、cron、LSP、feature flags

## 需要保留的架构主梁

### `main.tsx`

- 启动编排
- 启动时引导配置、工具 registry、命令 registry、会话状态和运行环境

### `QueryEngine.ts`

- 持有整个对话循环
- 接收用户输入、调用模型、执行工具、写 transcript，并在 assistant 完成前持续循环

### `Tool.ts` 和 `tools.ts`

- 定义工具契约和统一 registry
- 所有工具调用都经过同一条带权限判断的运行时路径

### `types/permissions.ts`

- 权限模式属于领域模型，而不是 UI 状态
- 规则可来自多个来源，并对不同工具产生不同影响

### `utils/sessionStorage.ts`

- JSONL transcript 是持久化事实源
- session 索引、resume 行为、transcript 派生元数据都建立在 canonical transcript 之上

## V1 需要承接的部分

- 共享 session engine
- 共享工具执行管线
- 多权限模式
- append-only JSONL transcript
- 一组核心 runtime 控制命令
- resume 支持
- 可以面向多家 API 的 provider 抽象，而不是只锁单一厂商
- 内置 web search 和 web fetch，并且继续走统一权限化工具运行时

## 暂缓的部分

- MCP 和 remote server resources
- bridge 和 remote control
- LSP 集成
- cron 和 automations
- voice
- team agents 和 swarms
- 更重的终端 UI overlays

## 可以丢弃或明显简化的部分

- Bun 专属 feature flags
- 产品 analytics 和增长实验逻辑
- 复杂的环境相关启动逻辑
- 很多高度产品定制化命令
- 非常大的 UI 组件树

## Rust 转译策略

- 把参考工程当作行为规范，而不是逐行移植模板
- 保持核心循环显式、强类型
- 倾向小 crate 和清晰领域边界
- 优先选择可调试、基于文件的状态，而不是隐藏的全局进程状态
- 围绕可移植性设计，让同一套 engine 能支撑 POSIX 和 PowerShell 执行适配器
