# TUI 监督决策检查点

英文版：[checkpoints.md](checkpoints.md)

日期：2026-08-30

本证据只描述本地候选。这里没有任何内容被发布、签名、公证、push、merge、tag 或经过
live provider 认证。工作以两个提交的形式存在于隔离 worktree 中的特性分支上。

## 候选线

| 项目 | SHA / 路径 |
| --- | --- |
| 基线 `origin/main` | `126a5321` |
| 阶段 1 — 监督基础 | `claude/tui-supervision-foundation` 上的 `2aafa99b` |
| 阶段 2 — 决策工作流 | `claude/tui-supervision-decisions` 的分支头 |
| Worktree | `.worktrees/tui-supervision-decisions` |
| 组件 | TUI `0.3.3`，`min_core_version` `0.3.4` |

阶段 2 从阶段 1 分出，而不是从 `main` 分出。两个分支都未被 merge，也未被 push。

## 交付的界面

| 界面 | 行为 |
| --- | --- |
| 决策中心（`OverlayKind::Decisions`） | 将审批、merge gate、待裁决 review request、待重新验证 conflict bounce 合并为一个有序可选列表，使用登记过的 `⏸` / `◌` / `⚠` 字形并带选中标记。审批行仍然路由到固定的 Approval 覆盖层。 |
| 监督决策覆盖层（`OverlayKind::SupervisionDecision`） | 新覆盖层，由 `SupervisionTarget`（gate、review 或 bounce）参数化。键盘优先的动作栏：方向键与数字键选择，`Enter` 确认，`Esc` 先关闭原因输入行再关闭覆盖层。 |
| 动作可用性 | 由每帧从 `RuntimeViewState` 重新读取的完整 Core 记录推导，绝不使用精简投影行。对记录当前状态永远不适用的动作不会列出。 |
| 原因 / 反馈输入 | 覆盖层内的单行输入。拒绝、回滚、回弹必须填写文本；两种 review 结论的反馈都是选填。必填文本为空或超过 Core 的 500 字符 trust-text 上限时在本地拒绝，且不发送任何命令。 |
| 分发与结果 | 每个动作通过 Core 客户端发送一条 `RuntimeCommand` 并登记 confirm-on-fact 期望。结果同时在覆盖层与状态栏渲染：pending `◌`、confirmed `✓`、rejected `✗` 并原样展示 Core 给出的原因。 |
| 重置与逃生 | 已裁决结果在发起下一次监督动作或关闭覆盖层时重置为 idle；pending 绝不自动重置。覆盖层与决策中心页脚提供 `dismiss` 动作，清除滞留的 pending 归属但不裁决结果，也不取消 Core 命令。 |
| 墙钟时间展示 | cost-blind lane 的运行统计把墙钟时间渲染为毫秒、带一位小数的秒或分加秒，取代原来的原始毫秒数。 |

## 与 GUI 对齐的载荷推导

TUI 发送与 GUI 集成门适配器完全相同的载荷形状，因为 Core 就是按这些精确取值校验接受操作的
（`apps/gui/src-tauri/src/projection.rs`、`apps/gui/src-tauri/src/adapter.rs`）：

| 命令 | 推导 |
| --- | --- |
| `AcceptMergeGate.actor` | `d12_accept_actor`：存在 validator 时，取带 Lane id 的 validator owner，否则没有可用 actor；不存在 validator 时取 gate owner。 |
| `AcceptMergeGate.reviewed_evidence` | `d12_reviewed_evidence`：存在 validator 时原样使用 review request 记录的 `evidence_bindings`；默认 owner 的 gate 发送空列表；其余情况按 `d12_canonical_bindings` 从 `latest_evidence` 重建规范绑定并排序去重。任一列出的证据缺少规范引用即表示没有合法载荷，该动作在本地被拒绝。 |
| `RejectMergeGate.actor` | `d12_reject_actor`：gate owner 非默认时取 gate owner，否则取带 Lane id 且非默认的 validator owner。 |
| `RevertAppliedChange.owner` | gate owner；为默认 owner 时拒绝。 |
| `BounceMergeConflict` | owner 与 `original_lane_id` 都从 gate owner 回放，符合 `validate_conflict_bounce` 的要求。 |
| `RevalidateMergeConflict` | `bounce_id` 与 `actor` 来自待处理的 conflict 记录；`evidence` 取 `source_hash` 与全部 baseline 哈希都不同的那条规范绑定，符合 `validate_conflict_revalidation` 的要求。 |
| `DecideReview.actor` | gate validator 指向该 review 时取其 owner，否则复现 Core 自身的 `reviewer_owner_from_requester`：把 requester owner 指向 reviewer Lane，并清空 session 与 turn 身份。 |

## Confirm-On-Fact 期望

| 动作 | 确认用的 Core 事实 |
| --- | --- |
| 接受 gate | 命中该 gate id 且状态为 `Accepted` 的 `MergeGateUpdated` |
| 拒绝 gate | 命中该 gate id 且状态为 `NeedsChanges` 的 `MergeGateUpdated` |
| 重新验证冲突 | 命中该 gate id 且状态为 `CollectingEvidence` 的 `MergeGateUpdated`（`trust_loop::revalidate_merge_conflict` 先发布 `MergeConflictBounced`，再把 gate 退回证据收集态） |
| 回滚 | 该 gate 的 `RevertRecorded` |
| 回弹 | 该 gate 的 `MergeConflictBounced` |
| 裁决 review | 命中该 review id 且状态与结论对应的 `ReviewRequestUpdated` |

`CommandAccepted` 绝不确认。相同 command id 的 `CommandRejected` 是唯一的本地失败路径，
并原样携带 Core 给出的原因。

## 固定测试

已加入 `scripts/tui-turn-controller-smoke.sh`：

```text
tui::pending::tests::abandon_clears_a_stranded_pending_decision_without_settling_it
tui::pending::tests::only_a_settled_outcome_resets_to_idle
tui::decision::tests::action_availability_follows_the_records_current_status
tui::decision::tests::accept_payload_mirrors_the_gui_actor_and_reviewed_evidence_derivation
tui::decision::tests::reject_revert_and_bounce_replay_core_owned_parties_and_require_a_reason
tui::decision::tests::revalidation_carries_the_bounce_identity_and_a_changed_canonical_receipt
tui::decision::tests::review_verdicts_carry_the_reviewer_lane_actor_and_optional_feedback
tui::decision::tests::decision_picks_list_approvals_gates_pending_reviews_then_pending_bounces
tui::app::tests::decision_center_lists_supervision_rows_and_routes_every_pick
tui::app::tests::supervision_overlay_unwinds_escape_in_order_and_yields_to_a_pinned_approval
tui::app::tests::supervision_overlay_only_lists_actions_the_gate_status_can_accept
tui::app::tests::a_required_reason_is_enforced_locally_and_nothing_is_sent
tui::app::tests::every_supervision_decision_round_trips_through_its_exact_core_fact
tui::app::tests::core_rejection_renders_its_own_reason_and_frees_the_decision_slot
tui::app::tests::a_second_supervision_action_while_one_is_pending_sends_nothing
tui::app::tests::dismiss_releases_a_stranded_pending_decision_without_sending_anything
tui::app::tests::a_settled_outcome_resets_on_the_next_action_and_on_overlay_close
tui::app::tests::composer_stays_editable_while_the_supervision_overlay_is_open_during_a_stream
tui::app::tests::blind_lane_wall_time_is_rendered_at_the_scale_an_operator_reads
tui::modal::tests::decisions_overlay_projects_typed_gates_recovery_and_pending_core_command
tui::modal::tests::blind_lane_inspector_shows_bounded_run_facts_and_never_fabricates_zeros
```

`every_supervision_decision_round_trips_through_its_exact_core_fact` 通过伪 Core 客户端驱动
全部七条命令，并对每一条断言：发送的 `RuntimeCommandEnvelope` 精确匹配、回执只保持 pending、
只有匹配的业务事实才确认。

## 确定性证据

| 命令 | 结果 |
| --- | --- |
| `cargo test -p viden-tui` | PASS，306 个库测试 + 1 个 API 测试 |
| `bash scripts/tui-turn-controller-smoke.sh` | PASS |
| `bash scripts/rc-tui-stability-smoke.sh` | PASS |
| `bash scripts/tui-regression.sh` | PASS |
| `bash scripts/tui-previews.sh` | PASS，全部 preview 断言成立 |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets` | PASS，`viden-tui` 无新增告警 |
| `cargo test --workspace --quiet` | PASS |
| `bash scripts/check-dependency-boundaries.sh` | PASS |
| `git diff --check` | PASS |
| 对改动 Markdown 运行 `scripts/check-doc-pairs.sh` / `scripts/check-doc-links.sh` | PASS |

`apps/tui/release-manifest.toml` 中的 i18n catalog 摘要已按新增的监督键重算，两个语言包保持
精确的键与参数对等。

## 本阶段未交付（T1b）

- 基于 `AuditRecord` 时间线的审计 / 历史面板。
- handoff、contract、dependency 的创建流程。其 intent 构造器已存在于
  `apps/tui/src/tui/supervision.rs`，尚未被分发，暂由局部 `#[allow(dead_code)]` 标注。
- 专用的证据检视器。决策覆盖层展示证据数量与标识，不展示证据内容。
- 多选或批量监督决策。按设计，同一时刻只允许一条监督命令在途。
