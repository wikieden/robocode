/**
 * check-changelog.js · CHANGELOG 卫生防漂移扫描（design-spec-kit · 项目通用）
 *
 * 把 CLAUDE.md「Changelog 维护」里可机判的三条约定变成 DoD 守卫
 * （改 docs/CHANGELOG.md 后必跑）：
 *
 *   ❌ HARD FAIL  同一日期出现 >1 个 `## YYYY-MM-DD` 段
 *                 （硬规则：写前先 grep 同日段、命中就 append，绝不新开第二段）
 *   ⚠  WARN       文件总行数 > WARN_LINES → 把窗口外早期整段移到
 *                 docs/_archive/CHANGELOG-YYYY-MM.md（主文件留最近约 2 个会话日）
 *   ⚠  WARN       单条目子 bullet > MAX_SUB → 验尸报告化，细节该分流到
 *                 docs/ 下对应 doc（只点名，不 fail）
 *
 * ─────────────────────────────────────────────────────────────
 *  怎么跑：read_file 本文件 → 整个粘到 run_script。
 *  只用沙箱 helper：readFile / log。无 baseline、无写盘（纯只读扫描）。
 *  退出语义：有 HARD FAIL 时末行打印 `RESULT: FAIL`，否则 `RESULT: PASS`。
 * ═════════════════════════════════════════════════════════════*/

// ─── 配置 ──────────────────────────────────────────────────────

const CHANGELOG_PATH = 'docs/CHANGELOG.md';   // ← 你的 CHANGELOG 路径
const WARN_LINES = 200;   // 超过此行数 → 提示归档（CLAUDE.md：留最近 ~2 会话日 / 超 ~200 行归档）
const MAX_SUB    = 3;     // 单条目允许的子 bullet 上限（深度上限：1 行标题 + 最多 3 子 bullet）

// ─── 解析 ──────────────────────────────────────────────────────

const RE_DATE_H  = /^##\s+(\d{4}-\d{2}-\d{2})\b/;   // 真实日期段（## YYYY-MM-DD）
const RE_ANY_H2  = /^##\s+/;                         // 任意 H2（模块索引 / 约定段等）
const RE_TOP_LI  = /^-\s+\S/;                        // 顶层条目（- 开头）
const RE_SUB_LI  = /^\s+-\s+\S/;                     // 子 bullet（缩进 - 开头）

const src = await readFile(CHANGELOG_PATH);
const lines = src.split('\n');
const totalLines = lines.length;

// 1) 重复同日段
const dateHits = {};   // date -> [lineNo,...]
lines.forEach((ln, i) => {
  const m = ln.match(RE_DATE_H);
  if (m) (dateHits[m[1]] = dateHits[m[1]] || []).push(i + 1);
});
const dupDates = Object.entries(dateHits).filter(([, ls]) => ls.length > 1);

// 2) 条目深度：只扫真实日期段内的条目
const entries = [];   // {date, line, title, subCount}
let inDated = false, curDate = null, cur = null;
const pushCur = () => { if (cur) { entries.push(cur); cur = null; } };
lines.forEach((ln, i) => {
  const dm = ln.match(RE_DATE_H);
  if (dm) { pushCur(); inDated = true; curDate = dm[1]; return; }
  if (RE_ANY_H2.test(ln)) { pushCur(); inDated = false; curDate = null; return; }  // 非日期 H2（模块索引 / 约定段）
  if (!inDated) return;
  if (RE_TOP_LI.test(ln)) {
    pushCur();
    cur = { date: curDate, line: i + 1, title: ln.replace(/^-\s+/, '').replace(/\*\*/g, '').slice(0, 64), subCount: 0 };
  } else if (cur && RE_SUB_LI.test(ln)) {
    cur.subCount++;
  }
});
pushCur();

const fatEntries = entries.filter(e => e.subCount > MAX_SUB).sort((a, b) => b.subCount - a.subCount);

// ─── 报告 ──────────────────────────────────────────────────────

let fail = false;
log(`scanned ${CHANGELOG_PATH} · ${totalLines} 行 · ${entries.length} 条目 · ${Object.keys(dateHits).length} 个日期段`);

// 重复同日段（HARD FAIL）
if (dupDates.length > 0) {
  fail = true;
  log(`\n✗ ${dupDates.length} 个日期出现重复 \`## YYYY-MM-DD\` 段（硬规则：同日只能一段）：`);
  for (const [d, ls] of dupDates) log(`    ${d}  ×${ls.length}  → 行 ${ls.join(', ')}`);
  log(`  修法：把这些段的条目合并到第一段（最上方那个），删掉多余 \`## ${dupDates[0][0]}\` 标题与其间的 \`---\` 分隔。`);
} else {
  log('✓ 同日合并：每个日期仅一段');
}

// 文件超长（WARN）
if (totalLines > WARN_LINES) {
  log(`\n⚠ 文件 ${totalLines} 行 > ${WARN_LINES} 行阈值 → 建议归档`);
  const dates = Object.keys(dateHits).sort();
  log(`    当前日期段（旧→新）：${dates.join(' · ')}`);
  log(`    把最旧的几段整段移到 docs/_archive/CHANGELOG-YYYY-MM.md（原样保真），主文件留最近约 2 个会话日 + 底部「更早条目」链接。`);
} else {
  log(`✓ 文件长度：${totalLines} 行（≤ ${WARN_LINES}）`);
}

// 条目深度（WARN，只点名）
if (fatEntries.length > 0) {
  log(`\n⚠ ${fatEntries.length} 条条目子 bullet > ${MAX_SUB}（验尸报告化，细节该分流到 docs/）：`);
  for (const e of fatEntries.slice(0, 8)) log(`    L${e.line}  [${e.date}]  ${e.subCount} bullets  ·  ${e.title}…`);
  if (fatEntries.length > 8) log(`    …还有 ${fatEntries.length - 8} 条`);
  log(`    深内容指向 DESIGN-REF.md 等 doc，条目里只留一句话 + 指路。`);
} else {
  log(`✓ 条目深度：均 ≤ ${MAX_SUB} 子 bullet`);
}

log(`\nRESULT: ${fail ? 'FAIL' : 'PASS'}`);
