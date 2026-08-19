//! Shared permission gate: the decide -> ask -> apply_approval sequence,
//! written once.
//!
//! Every call site that resolves a permission interactively must produce the
//! same fail-closed sequence: a pure `decide()`, an operator prompt only for
//! `Ask`, and an `apply_approval()` whose plan-mode re-check can still deny an
//! operator "allow" (the double checkpoint in `viden-permissions`). Hand-writing
//! that sequence per call site is exactly how a future site forgets a step, so
//! [`resolve`] is the one place it exists. Sites keep their own transcript,
//! event, and rendering work inside the `ask_flow` closure so observable
//! behavior stays byte-identical to the pre-gate code.
//!
//! [`PermissionBackstopInterceptor`] is the structural second line: registered
//! on the `ToolRegistry`, it re-checks the pure `decide()` immediately before
//! any tool executes, so a call site that forgets the gate entirely still
//! cannot mutate against a `Deny` (plan mode included) or an unresolved `Ask`.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock};

use viden_permissions::{PermissionContext, PermissionEngine};
use viden_tools::{InterceptVerdict, ToolExecutionContext, ToolExecutionInterceptor};
use viden_types::{
    ApprovalResponse, PermissionAskDecision, PermissionDecision, PermissionMode, PermissionPrompt,
    PermissionRule, ToolCall, ToolInput, ToolResult, ToolSpec,
};

/// The minimal engine surface the gate needs. Implemented by the plain
/// `PermissionEngine` (lane approval re-checks, ACP-local engines) and by
/// [`SharedPermissionEngine`] (the session engine's shared handle).
pub(crate) trait PermissionDecider {
    fn decide(&self, tool: &ToolSpec, input: &ToolInput) -> PermissionDecision;
    fn apply_approval(
        &mut self,
        response: ApprovalResponse,
        ask: &PermissionAskDecision,
        tool: &ToolSpec,
        input: &ToolInput,
    ) -> PermissionDecision;
}

impl PermissionDecider for PermissionEngine {
    fn decide(&self, tool: &ToolSpec, input: &ToolInput) -> PermissionDecision {
        PermissionEngine::decide(self, tool, input)
    }

    fn apply_approval(
        &mut self,
        response: ApprovalResponse,
        ask: &PermissionAskDecision,
        tool: &ToolSpec,
        input: &ToolInput,
    ) -> PermissionDecision {
        PermissionEngine::apply_approval(self, response, ask, tool, input)
    }
}

/// Resolve a tool permission through the shared gate.
///
/// `ask_flow` runs only when `decide()` returns `Ask`. It receives the ask
/// decision and the rendered prompt, performs the call-site-specific work
/// (transcript entries, runtime events, task updates) around obtaining the
/// operator response, and returns that response. The gate then applies it via
/// `apply_approval`, which re-checks plan mode; the returned decision is never
/// `Ask`, so callers may treat that arm as unreachable.
pub(crate) fn resolve<D: PermissionDecider>(
    decider: &mut D,
    tool: &ToolSpec,
    prompt_tool_name: &str,
    input: &ToolInput,
    ask_flow: impl FnOnce(&PermissionAskDecision, PermissionPrompt) -> ApprovalResponse,
) -> PermissionDecision {
    let decision = decider.decide(tool, input);
    let PermissionDecision::Ask(ask) = decision else {
        return decision;
    };
    let prompt = PermissionEngine::prompt_for(prompt_tool_name, &ask, input);
    let response = ask_flow(&ask, prompt);
    decider.apply_approval(response, &ask, tool, input)
}

/// Upper bound for remembered interactive clearances. One clearance is
/// consumed by the registry execution that immediately follows its approval;
/// the cap only bounds entries whose approved call never reached the registry
/// (for example `context_read`, which executes outside `ToolRegistry`).
const MAX_EXECUTION_CLEARANCES: usize = 32;

/// The session engine's permission engine behind a shared handle, so the
/// registry-level [`PermissionBackstopInterceptor`] always observes the
/// current mode and rules, including engines swapped in wholesale by
/// `set_permission_mode`/`set_work_mode`.
///
/// It also records one-shot execution clearances: an interactive `Ask`
/// resolved to `Allow` with scope `Once` installs no rule, so a pure
/// re-`decide()` would still say `Ask`. The clearance is how the backstop
/// distinguishes that approved call from one that skipped approval entirely.
#[derive(Clone)]
pub(crate) struct SharedPermissionEngine {
    engine: Arc<RwLock<PermissionEngine>>,
    cleared_executions: Arc<Mutex<VecDeque<String>>>,
}

impl SharedPermissionEngine {
    pub(crate) fn new(engine: PermissionEngine) -> Self {
        Self {
            engine: Arc::new(RwLock::new(engine)),
            cleared_executions: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, PermissionEngine> {
        self.engine
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, PermissionEngine> {
        self.engine
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn mode(&self) -> PermissionMode {
        self.read().mode()
    }

    pub(crate) fn set_mode(&self, mode: PermissionMode) {
        self.write().set_mode(mode);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn add_rule(&self, rule: PermissionRule) {
        self.write().add_rule(rule);
    }

    pub(crate) fn context_snapshot(&self) -> PermissionContext {
        self.read().context_snapshot()
    }

    pub(crate) fn restore_context(&self, context: PermissionContext) {
        self.write().restore_context(context);
    }

    /// Clone of the current engine for consumers that need a detached copy
    /// (lane workers, prepare-then-commit mode changes).
    pub(crate) fn engine_snapshot(&self) -> PermissionEngine {
        self.read().clone()
    }

    /// Swap in a fully prepared engine. Used by the durable-first mode
    /// changes that build the next engine, persist metadata, then publish.
    pub(crate) fn replace_engine(&self, engine: PermissionEngine) {
        *self.write() = engine;
    }

    pub(crate) fn decide(&self, tool: &ToolSpec, input: &ToolInput) -> PermissionDecision {
        self.read().decide(tool, input)
    }

    pub(crate) fn apply_approval(
        &self,
        response: ApprovalResponse,
        ask: &PermissionAskDecision,
        tool: &ToolSpec,
        input: &ToolInput,
    ) -> PermissionDecision {
        let decision = self.write().apply_approval(response, ask, tool, input);
        // Remember interactively resolved allows so the registry backstop can
        // tell an approved once-off from a call that skipped approval.
        if matches!(decision, PermissionDecision::Allow(_)) {
            let mut cleared = self
                .cleared_executions
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            cleared.push_back(clearance_key(tool, input));
            while cleared.len() > MAX_EXECUTION_CLEARANCES {
                cleared.pop_front();
            }
        }
        decision
    }

    fn take_execution_clearance(&self, tool: &ToolSpec, input: &ToolInput) -> bool {
        let key = clearance_key(tool, input);
        let mut cleared = self
            .cleared_executions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match cleared.iter().position(|entry| *entry == key) {
            Some(index) => {
                cleared.remove(index);
                true
            }
            None => false,
        }
    }
}

impl PermissionDecider for SharedPermissionEngine {
    fn decide(&self, tool: &ToolSpec, input: &ToolInput) -> PermissionDecision {
        SharedPermissionEngine::decide(self, tool, input)
    }

    fn apply_approval(
        &mut self,
        response: ApprovalResponse,
        ask: &PermissionAskDecision,
        tool: &ToolSpec,
        input: &ToolInput,
    ) -> PermissionDecision {
        SharedPermissionEngine::apply_approval(self, response, ask, tool, input)
    }
}

fn clearance_key(tool: &ToolSpec, input: &ToolInput) -> String {
    // Unit separator keeps tool name and encoded input unambiguous.
    format!(
        "{}\u{1f}{}",
        tool.name,
        viden_types::encode_tool_input(input)
    )
}

/// Defense-in-depth registry interceptor: re-checks the pure `decide()` right
/// before a mutating tool executes. It never prompts; `Ask` here means the
/// call site failed to resolve approval (no recorded clearance), so the call
/// is rejected fail-closed. This makes permission-before-mutation structural:
/// a future call site that forgets the gate still cannot mutate in plan mode
/// or against a deny rule.
pub(crate) struct PermissionBackstopInterceptor {
    permissions: SharedPermissionEngine,
}

impl PermissionBackstopInterceptor {
    pub(crate) fn new(permissions: SharedPermissionEngine) -> Self {
        Self { permissions }
    }
}

impl ToolExecutionInterceptor for PermissionBackstopInterceptor {
    fn before_execute(
        &self,
        spec: &ToolSpec,
        call: &ToolCall,
        _ctx: &ToolExecutionContext,
    ) -> InterceptVerdict {
        // Non-mutating tools cannot violate permission-before-mutation; the
        // interactive gate still governs their ask flows (for example shell
        // prompts) at the call site.
        if !spec.is_mutating {
            return InterceptVerdict::Proceed;
        }
        match self.permissions.decide(spec, &call.input) {
            PermissionDecision::Allow(_) => {
                // Consume a matching clearance if one exists so a rule-backed
                // allow does not leave a stale once-off entry behind.
                let _ = self.permissions.take_execution_clearance(spec, &call.input);
                InterceptVerdict::Proceed
            }
            PermissionDecision::Ask(_) => {
                if self.permissions.take_execution_clearance(spec, &call.input) {
                    InterceptVerdict::Proceed
                } else {
                    InterceptVerdict::Reject {
                        message: format!(
                            "permission backstop blocked `{}`: approval was not resolved before execution",
                            spec.name
                        ),
                    }
                }
            }
            PermissionDecision::Deny(deny) => InterceptVerdict::Reject {
                message: format!(
                    "permission backstop blocked `{}`: {}",
                    spec.name, deny.message
                ),
            },
        }
    }

    fn after_execute(&self, _call: &ToolCall, _result: &ToolResult) {}
}
