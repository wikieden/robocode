# 插件架构

English version: [plugin-architecture.md](plugin-architecture.md)

Viden 插件通过带版本的 descriptor 和 host 校验来扩展运行时行为。Rust 原生运行时仍然是本地状态、权限、transcript facts、canonical context storage、evidence verification 和 Merge Gate decisions 的权威来源。

## Context Reducer Capability

`context_reducer` 是可选优化能力，用于把 canonical context 压缩成有界的 provider-ready view。它默认关闭，只有配置显式 opt-in 到某个 adapter descriptor 时才会运行。

descriptor 由 `crates/plugin-api` 中的 `ContextReducerDescriptor` 表示：

- `reducer_id`、`display_name`、`version`；
- `supported_schema_versions`，当前 schema version 为 `1`；
- 支持的 `content_kinds`；
- input bytes、output bytes、output item count 和 depth 的硬限制；
- `default_enabled`，出于安全原因必须保持 `false`；
- `config_schema_version`。

未来的 adapter，包括 Headroom 风格 adapter，都通过这个中性的 descriptor 加可选 process config 表达。核心 crates 不依赖 Headroom、Python、Pyo3 或任何 adapter 专有运行时。

## Request Envelope

只有在原生 context storage 产生 canonical item 和 permission scope 之后，host 才会发送 `ContextReducerRequest`。请求包含：

- schema version 和 request id；
- content kind；
- canonical item id、canonical content SHA-256、可选 evidence id 和逻辑 reference；
- reduction budget 和 policy；
- role/task scope；
- permission snapshot reference；
- 可选 native baseline quality facts。

请求不得包含本地 storage path、原始 credential、API key，或 canonical store 的可变 handle。

## Response Envelope

adapter 返回 `ContextReducerResponse`，其中包含：

- 相同的 schema version、request id、canonical hash、permission snapshot reference、scope binding 和 content kind；
- 严格满足 byte/item/depth 限制的 UTF-8 reduced content；
- omission records；
- 已协商的 reducer id 和 version；
- 确定性的 quality facts，包括 score 和 evidence recall；
- latency 和 health metadata。

adapter 可以优化 provider input，但不能修改 canonical context、绕过权限、削弱 evidence recall，或改变 Merge Gate truth。

## Host Validation And Fallback

`crates/plugin-host` 在调用 adapter 前会协商 schema 和 content-kind support。生产 adapter 使用 `ContextReducerProcessConfig`：安全 executable path、字面 args、可选 cwd、显式 env allowlist，以及有界 stderr capture。host 不调用 shell，会先清空 environment 再应用 allowlist，通过有界 pipe 传输 request/response JSON，并执行 host wall-clock deadline。timeout 时，host 会 kill 并 wait/reap direct child，然后再返回 native fallback。由于跨平台 process-group kill 不可用时只能保证 direct child，adapter contract 禁止 shell wrapping 和自行 spawn child。

in-process closure executor 只用于可信测试和 cooperative local harness。它会在命名 worker thread 中运行 owned request value，并使用 `catch_unwind` 与 host wall-clock `recv_timeout`，但 timeout 后不能取消任意用户代码，因此不是生产 adapter boundary。runtime 只有在配置 process transport 时才使用外部 adapter，否则不使用 adapter。host 不信任 response 自报 latency 来做 timeout 决策。

只有 request id、canonical hash、permission snapshot reference、scope、content kind、已协商 reducer id/version、schema version、size、encoding、quality/evidence thresholds 全部通过时，外部结果才会被接受。

timeout、crash、adapter absent、malformed response、错误 schema version、错误 hash、错误 scope、oversize response 和 quality failure 都会产生有界 health evidence，并确定性回退到 native reducer。当 native reducer 健康时，startup 和 provider request 不能被 adapter 阻塞。

host 包含带有有界 `open_until` 的 circuit breaker。达到 failure threshold 后，调用会被跳过直到 monotonic deadline。deadline 之后的下一次调用是 half-open probe：成功会 reset breaker，失败会重新 open。telemetry 只记录 health、measured latency 和 failure category；不得包含 secrets 或原始本地路径。

runtime 会通过 `ContextReductionRecorded` / `ContextReductionRecord` 记录成功和 fallback 的 adapter attempt。record 包含 adapter id/version、有界 status/reason、host-measured latency、fallback flag、item/view binding 和 timestamp。它刻意不包含 request content、canonical storage path、credential、raw stderr 或 raw adapter output。默认关闭的 adapter 不会产生 failure noise。
