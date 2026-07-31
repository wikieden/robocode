import en from "./en.json";
import zhCN from "./zh-CN.json";

export type Locale = "en" | "zh-CN";

export const MESSAGE_ARGUMENT_NAMES = {
  "app.name": [],
  "connection.pending": [],
  "connection.unavailable": [],
  "d11.cancel": [],
  "d11.confirm": [],
  "d11.confirmed": [],
  "d11.credential": [],
  "d11.history": [],
  "d11.intake": [],
  "d11.lane.create": [],
  "d11.lane.ready": [],
  "d11.mode": ["mode"],
  "d11.noProject": [],
  "d11.preview": [],
  "d11.probe": [],
  "d11.projectBootstrapUnavailable": [],
  "d11.projectPath": [],
  "d11.providerWarning": ["provider", "status"],
  "d11.step.config": [],
  "d11.step.lanes": [],
  "d11.step.mode": [],
  "d11.step.project": [],
  "d11.waiting": [],
  "d4.approval.allow": [],
  "d4.approval.deny": [],
  "d4.back": [],
  "d4.base": [],
  "d4.branch": [],
  "d4.budget.default": [],
  "d4.cancel": [],
  "d4.create": [],
  "d4.create.planDisabled": [],
  "d4.gate": [],
  "d4.laneId": [],
  "d4.preset.coder": [],
  "d4.preset.reviewer": [],
  "d4.preset.tester": [],
  "d4.preview": [],
  "d4.repreview": [],
  "d4.role": [],
  "d4.route": [],
  "d4.skip": [],
  "d4.step.gates": [],
  "d4.step.review": [],
  "d4.step.role": [],
  "d4.step.runtime": [],
  "d4.target": [],
  "d4.title": [],
  "d4.waiting": [],
  "d4.worktree": [],
  "d1.activity": [],
  "d1.activity.diagnostics": [],
  "d1.activity.evidence": [],
  "d1.activity.git": [],
  "d1.activity.inbox": [],
  "d1.activity.lanes": [],
  "d1.activity.search": [],
  "d1.activity.work": [],
  "d1.cancel": [],
  "d1.composer.placeholder": [],
  "d1.composer.prompt": [],
  "d1.composer.queue": [],
  "d1.composer.send": [],
  "d1.composer.inputLabel": [],
  "d1.composer.sendLabel": [],
  "d1.environment": [],
  "d1.environment.cost": [],
  "d1.environment.mode": [],
  "d1.environment.model": [],
  "d1.environment.permission": [],
  "d1.environment.provider": [],
  "d1.environment.tokens": [],
  "d1.lanes": [],
  "d1.lane.create": [],
  "d1.agentMenu.title": [],
  "d1.agentMenu.newLane": [],
  "d1.agentMenu.agentLabel": [],
  "d1.agentMenu.viden": [],
  "d1.agentMenu.ready": [],
  "d1.agentMenu.probing": [],
  "d1.agentMenu.empty": [],
  "d1.task.label": [],
  "d1.task.placeholder": [],
  "d1.task.hint": ["branch", "worktree"],
  "d1.task.directWorkspaceHint": [],
  "d1.task.submit": [],
  "d1.task.cancel": [],
  "d1.task.nativeTitle": [],
  "d1.task.acpTitle": ["agent"],
  "d1.session.retry": [],
  "d1.liveWork": [],
  "d1.workSurface": [],
  "d1.workspaceChange": [],
  "d1.workspaceChange.emptyPatch": [],
  "d1.checkRun": [],
  "d1.checkRun.emptyCommand": [],
  "d1.checkRun.emptyResult": [],
  "d1.mutation.noOwner": [],
  "d1.mutation.duplicateSession": [],
  "d1.mutation.ownerMismatch": [],
  "d1.mutation.staleLane": [],
  "d1.status.added": [],
  "d1.status.modified": [],
  "d1.status.deleted": [],
  "d1.status.renamed": [],
  "d1.status.untracked": [],
  "d1.status.queued": [],
  "d1.status.running": [],
  "d1.status.passed": [],
  "d1.status.failed": [],
  "d1.status.cancelled": [],
  "d1.risk.low": [],
  "d1.risk.medium": [],
  "d1.risk.high": [],
  "d1.risk.critical": [],
  "d1.permission.region": [],
  "d1.permission.target": [],
  "d1.permission.reason": [],
  "d1.permission.scope": [],
  "d1.permission.policy": [],
  "d1.permission.expires": [],
  "d1.permission.defaultAction": [],
  "d1.permission.audit": [],
  "d1.permission.unavailable": [],
  "d1.permission.plan": [],
  "d1.permission.once": [],
  "d1.permission.session": [],
  "d1.permission.repoAllowlist": [],
  "d1.permission.always": [],
  "d1.permission.edit": [],
  "d1.permission.deny": [],
  "d1.permission.target.local": [],
  "d1.permission.target.repoPath": [],
  "d1.permission.policy.commandNotAllowlisted": [],
  "d1.permission.policy.requiresApproval": [],
  "d1.newOutput": ["count"],
  "d1.queued": [],
  "d1.stream.active": ["lane"],
  "d1.stream.idle": [],
  "d1.title": [],
  "d1.transcript": [],
  "d1.transcript.userUnavailable": [],
  "d1.transcript.assistantUnavailable": [],
  "d1.transcript.switching": [],
  "d1.context.switching": [],
  "d1.context.changesSource": [],
  "d1.context.context": [],
  "d1.context.laneAgent": [],
  "d1.context.sources": [],
  "d1.context.mcp": [],
  "d1.context.lsp": [],
  "d1.context.taskChecklist": [],
  "d1.context.noSource": [],
  "d1.context.noTypedContext": [],
  "d1.context.noAgentOwner": [],
  "d1.context.noServices": [],
  "d1.context.noChecklist": [],
  "d1.context.lane": [],
  "d1.context.ahead": [],
  "d1.context.behind": [],
  "d1.context.dirty": [],
  "d1.context.dirty.true": [],
  "d1.context.dirty.false": [],
  "d1.context.budget": [],
  "d1.context.remaining": [],
  "d1.context.route": [],
  "d1.context.status": [],
  "d1.context.session": [],
  "d1.context.native": [],
  "d1.context.acp": [],
  "d1.context.connected": [],
  "d1.context.ready": [],
  "d1.context.degraded": [],
  "d1.context.offline": [],
  "d1.unavailable": [],
  "d1.welcome.eyebrow": [],
  "d1.welcome.getStarted": [],
  "d1.welcome.noProject": [],
  "d1.welcome.openProject": [],
  "d1.welcome.openFolderTitle": [],
  "d1.welcome.recent": [],
  "d1.welcome.recentHint": [],
  "d1.welcome.recentUnavailable": [],
  "d1.welcome.subtitle": [],
  "d1.welcome.title": [],
  "d1.welcome.windowTitle": [],
  "preferences.unavailable": ["capability"],
  "preferences.missing": ["key"],
  "preferences.theme": ["mode", "skin"],
  "preferences.density": ["density"],
  "preferences.motion": ["motion"],
} as const;

export type MessageKey = keyof typeof MESSAGE_ARGUMENT_NAMES;
export type MessageArguments = {
  [K in MessageKey]: Record<(typeof MESSAGE_ARGUMENT_NAMES)[K][number], string>;
};
export type Catalog = Record<MessageKey, string>;

export const CATALOGS: Readonly<Record<Locale, Catalog>> = {
  en,
  "zh-CN": zhCN,
};

const PLACEHOLDER_PATTERN = /\{([A-Za-z][A-Za-z0-9_]*)\}/g;

export function catalogKeys(catalog: Catalog): string[] {
  return Object.keys(catalog).sort();
}

export function catalogPlaceholders(catalog: Catalog): Record<string, string[]> {
  return Object.fromEntries(
    catalogKeys(catalog).map((key) => {
      const placeholders = Array.from(
        catalog[key as MessageKey].matchAll(PLACEHOLDER_PATTERN),
        (match) => match[1],
      ).sort();
      return [key, placeholders];
    }),
  );
}

export function translate<K extends MessageKey>(
  locale: Locale,
  key: K,
  args: MessageArguments[K],
): string {
  const visited = new Set<Locale>();
  for (const candidate of [locale, "en"] as const) {
    if (visited.has(candidate)) {
      continue;
    }
    visited.add(candidate);
    const template = CATALOGS[candidate][key];
    if (typeof template === "string") {
      return template.replace(PLACEHOLDER_PATTERN, (_, name: string) => {
        const value = (args as Record<string, string>)[name];
        return value ?? `{${name}}`;
      });
    }
  }
  return `[missing:${String(key)}]`;
}
