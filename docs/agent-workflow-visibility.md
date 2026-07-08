# Agent Workflow Visibility

Chinese version: [agent-workflow-visibility.zh-CN.md](agent-workflow-visibility.zh-CN.md)

Status: product and frontend interaction proposal.

This document defines how Viden should explain an active agent workflow to the
user. The goal is to make orchestration legible without forcing the user to read
raw logs.

## User Questions

Every workflow view must answer six questions:

1. What will the agents do next?
2. What is each agent doing now?
3. What work has finished?
4. What has passed acceptance and is safe to merge, apply, or publish?
5. Why was this work assigned to this agent, tool, MCP capability, or skill?
6. What is the cost and budget impact of the current division of labor?

These are product-level questions. TUI, GUI, CLI status output, and future API
clients should answer them from the same runtime facts.

## Core Concept: Mission Control

Represent a workflow as a Mission Control board backed by `AgentDagRecord`,
`AgentTaskRecord`, `EvidenceView`, and `MergeGateRecord`.

The primary view has five sections:

| Section | User meaning | Runtime source |
| --- | --- | --- |
| Assignment | Why tasks are split this way and who owns each part | DAG planner output, agent capability/cost profile |
| Plan | Planned next work and dependency order | queued `AgentTaskRecord`, DAG dependencies |
| Now | Active agents and current step | running task status, activity, active tool/provider events |
| Done | Completed agent output that exists but may still need review | completed tasks, produced artifacts, evidence ids |
| Acceptance | Evidence checklist and merge/release decision | `MergeGateRecord`, required evidence, accepted artifacts |
| Blocked | Missing approval, failed evidence, conflicts, or user input needed | blocked/failed tasks, errors, next actions |
| Cost | Budget, spend so far, and expected remaining spend | token/cost records, provider/model metadata |

The board should avoid fake percentages. Use real phases, evidence counts, and
timestamps instead of guessed progress.

## Status Model

Use a small, stable vocabulary:

| Status | Meaning | User action |
| --- | --- | --- |
| `planned` | Task exists but is not ready to run | inspect scope, edit plan, start |
| `queued` | Dependencies or scheduler are waiting | reorder, cancel, inspect blocker |
| `running` | Agent is actively reasoning or using tools | watch, cancel, queue follow-up |
| `waiting_approval` | User decision is required | approve, deny, edit scope |
| `collecting_evidence` | Output exists and checks/review are still running | wait, inspect checklist |
| `done` | Agent completed its assigned work | inspect output and evidence |
| `needs_changes` | Review, tests, or user rejected the result | request revision or retry |
| `accepted` | Required evidence is satisfied | merge/apply/publish when appropriate |
| `merged` | Accepted change has been applied | inspect final diff and transcript |
| `failed` | Task failed with classified reason | retry, change provider, shrink scope |
| `cancelled` | User or runtime cancelled the task | resume, retry, or archive |

## Task Card Contract

Each visible task card should show:

- role and agent: `planner`, `coder`, `reviewer`, `tester`, `doc-writer`,
  `release-operator`, external ACP agent, MCP tool, or skill;
- objective: one sentence, not the whole prompt;
- scope: files, worktree, repo area, or external system;
- status and current activity;
- dependency blockers and upstream tasks;
- next action;
- evidence checklist summary, for example `tests 1/1`, `review 0/1`,
  `patch 1/1`;
- cost and duration when available;
- assignment reason: why this role/agent/tool/skill was selected;
- cost profile: cheap/default/premium/manual, budget cap, and current spend;
- artifact links: patch, docs, logs, screenshots, release assets, or MCP output.

## Assignment And Collaboration View

The workflow must show division of labor at the workflow level, not only per
agent. Users need to understand how agents cooperate inside the engineering
project.

The Assignment view should show:

- task owner: role, concrete agent, provider/model, MCP server/tool, or skill;
- assignment reason: specialty match, file ownership, context locality, previous
  evidence, cost, latency, or user preference;
- collaboration pattern: sequential handoff, parallel fan-out, reviewer/tester
  fan-in, or manual approval gate;
- scope boundary: files, directories, worktree, external system, or read-only
  research scope;
- expected output: plan, patch, test evidence, review findings, docs, release
  artifact, or diagnostic report;
- dependency links: upstream tasks that unblock the current task and downstream
  tasks waiting on it;
- budget: estimated tokens/cost, spend so far, and max allowed spend.

Example:

```text
Assignment
  planner       Viden core      cheap model     split task, low mutation risk
  coder-a       Codex ACP       premium         Rust runtime patch, high code skill
  tester        local tools     free            run cargo tests after coder-a
  reviewer      Claude ACP      premium         architectural review, risk-focused
  doc-writer    Viden core      cheap model     update bilingual docs

Collaboration
  planner -> coder-a -> tester
                  \-> reviewer -> acceptance
  coder-a -> doc-writer after behavior is accepted

Budget
  estimated $0.42, spent $0.11, remaining $0.31
```

Assignment reasons are first-class product text. If Viden cannot explain why a
task was assigned to an agent, the scheduler should mark the reason as
`not reported` rather than hiding the decision.

## Cost-Aware Orchestration

Division of labor must consider both capability and cost.

Scheduling inputs:

- capability fit: coding, planning, review, tests, docs, release, research;
- context fit: which agent already has the relevant context or session;
- tool fit: whether the work is cheaper and safer as local tool/MCP/skill work
  instead of another LLM call;
- risk: mutation level, permission scope, required evidence;
- latency: expected wait time and whether parallelism is worth it;
- cost: model/provider price, token budget, free local tool alternatives, and
  remaining workflow budget.

Cost display rules:

- show estimated, spent, and remaining workflow cost when available;
- distinguish LLM/provider spend from local tool time;
- explain cost-saving substitutions, for example "tester uses local cargo test
  instead of premium model";
- flag premium-agent use when a cheaper role/tool could likely handle the task;
- allow users to choose strategy presets such as `fast`, `balanced`, `cheap`,
  and `high-confidence`.

Cost must not override safety. A cheap agent cannot receive a task if it lacks
permission, context, or required capability.

## Detail View

Selecting a task opens a detail view with six tabs or sections:

1. **Objective**: original goal, non-goals, role assignment, scope.
2. **Assignment**: owner, reason, capability fit, cost fit, dependencies.
3. **Plan**: substeps generated by planner or workflow template.
4. **Activity**: live provider/tool/MCP/skill events, coalesced and ordered.
5. **Artifacts**: patches, docs, reports, logs, screenshots, release assets.
6. **Evidence**: tests, reviews, diagnostics, approvals, merge gate state.
7. **Next Action**: retry, revise, approve, merge, cancel, archive.

The detail view must preserve history after completion. A completed workflow is
still useful as a replayable decision record.

## Interaction Pattern

### Starting a Workflow

When the user starts a larger goal, Viden should show a generated plan before
parallel work begins:

```text
Workflow: Refactor provider config
Plan
  1. planner: define scope and risks
  2. coder: update config loader
  3. tester: run config and runtime tests
  4. reviewer: inspect diff and missing cases
  5. doc-writer: update provider docs
Acceptance
  patch, test_result, review, doc_update
```

The user can approve the plan, edit scope, or start a subset.

### During Execution

The visible text should be specific but compact:

```text
Now
  coder      editing crates/config/src/lib.rs
  tester     queued, waiting for coder
  reviewer   planned

Blocked
  doc-writer needs accepted behavior summary
```

### After Output

Completion and acceptance are separate:

```text
Done
  coder      produced patch, 3 files changed
  tester     cargo test -p viden-config passed

Acceptance
  patch        1/1
  test_result  1/1
  review       0/1
  doc_update   0/1
  status       collecting_evidence
```

Only the Acceptance section can claim the work is ready.

## TUI Expression

TUI should favor dense, stable surfaces:

- top active strip: current workflow, active task count, blocked count, accepted
  gates, budget status;
- side rail: `Assignment`, `Plan`, `Now`, `Done`, `Acceptance`, `Blocked`,
  `Cost` counts or summaries;
- main transcript: major workflow events and user-facing summaries;
- task detail panel: selected card with evidence checklist and next action;
- no modal-only workflow state; closing a modal must not hide active work.

## GUI Expression

GUI can use a richer Mission Control layout:

- left: workflow/DAG tree and filters;
- center: board columns for Assignment, Plan, Now, Done, Acceptance, Blocked;
- right: selected task detail with activity timeline and evidence checklist;
- bottom: cost/time/status timeline and assignment rationale.

GUI should support drill-down without changing runtime state: filters, grouping,
sort order, and pinning are local UI state only.

## Runtime Requirements

The runtime must provide enough facts for the UI to avoid inference:

- task status and current activity;
- task assignment owner and assignment reason;
- task plan steps and dependency ids;
- collaboration pattern and fan-out/fan-in edges;
- active provider/tool/MCP/skill step;
- evidence checklist requirements and collected evidence ids;
- acceptance state and latest decision;
- blocker classification and recovery suggestion;
- token/cost/duration when available;
- workflow budget, task estimate, task spend, and cost strategy when available;
- stable event ordering for replay.

If a fact is missing, UI should show `unknown` or `not reported` rather than
inventing progress.

## Acceptance Criteria

The visibility model is acceptable when:

- a user can answer the six workflow questions without reading logs;
- planned, running, completed, and accepted states are visually distinct;
- completion does not imply acceptance;
- agent assignment reasons and cost impact are visible at workflow and task
  level;
- blocked tasks show a concrete next action;
- TUI and GUI render from the same `RuntimeViewState`;
- replaying workflow events reconstructs the same board state.
