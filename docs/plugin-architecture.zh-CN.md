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
- `config_schema_version`；
- 可选 `ContextReducerProcessDescriptor`，只能由 plugin host/install boundary 提供。

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

`crates/plugin-host` 在调用 adapter 前会协商 schema 和 content-kind support。runtime config 只能 enable/select 已注册 reducer id，不能提供 executable 或 cwd。生产 adapter 使用由 plugin host/install boundary 注册的 `ContextReducerProcessDescriptor`：canonical absolute executable、字面 args、同一 canonical trusted plugin root 下的可选 cwd、显式 env allowlist、有界 stderr capture，以及把 adapter id/version 绑定到 executable identity 和 permission snapshot reference 的 `ContextReducerProcessAuthorization`。

host 会在 spawn 前拒绝 PATH-relative executable、symlink escape、trusted root 外的 cwd、不安全 reducer id/version，以及缺失或不匹配的 process authorization。host 不调用 shell，会先清空 environment 再应用 allowlist，把 request JSON 写入私有 read-only stdin temp file，将 stdout/stderr 路由到私有 temp file，并执行 host wall-clock deadline。host 只在 child exit 或 timeout+reap 后读取有界 prefix，因此不会等待 descendant 持有的 pipe EOF。Unix 上 adapter 会在独立 process group 中启动，timeout/oversize 会先 kill process group，再 wait/reap direct child；非 Unix 的 documented fallback boundary 是 direct-child kill/wait。adapter contract 仍禁止 shell wrapping 和自行 spawn child。

process stdout bound 取 runtime limits、descriptor limits、request policy 和 hard global cap 的最小值。host 会轮询 stdout temp-file length，并最多读取超过 bound 的一个 sentinel byte；触发 sentinel overflow 时会 kill 并 wait/reap process boundary，然后 native fallback。stderr 使用独立 temp file 和 hard cap，只会以 redacted bounded health 形式出现。temp artifacts 必须私有，并在 adapter call 结束后清理。

in-process closure executor 只用于可信测试和 cooperative local harness。它会在命名 worker thread 中运行 owned request value，并使用 `catch_unwind` 与 host wall-clock `recv_timeout`，但 timeout 后不能取消任意用户代码，因此不是生产 adapter boundary。runtime 只有在存在已注册 process descriptor 时才使用外部 adapter，否则不使用 adapter。host 不信任 response 自报 latency 来做 timeout 决策。

只有 request id、canonical hash、permission snapshot reference、scope、content kind、已协商 reducer id/version、schema version、size、encoding、quality/evidence thresholds 全部通过时，外部结果才会被接受。

timeout、crash、adapter absent、malformed response、错误 schema version、错误 hash、错误 scope、oversize response 和 quality failure 都会产生有界 health evidence，并确定性回退到 native reducer。当 native reducer 健康时，startup 和 provider request 不能被 adapter 阻塞。

host 包含带有有界 `open_until` 的 circuit breaker。达到 failure threshold 后，调用会被跳过直到 monotonic deadline。deadline 之后的下一次调用是 half-open probe：成功会 reset breaker，失败会重新 open。telemetry 只记录 health、measured latency 和 failure category；不得包含 secrets 或原始本地路径。

runtime 会通过 `ContextReductionRecorded` / `ContextReductionRecord` 记录成功和 fallback 的 adapter attempt。record 包含 adapter id/version、有界 status/reason、host-measured latency、fallback flag、item/view binding 和 timestamp。它刻意不包含 request content、canonical storage path、credential、raw stderr 或 raw adapter output。默认关闭的 adapter 不会产生 failure noise。
