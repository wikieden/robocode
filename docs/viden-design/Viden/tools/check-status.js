/**
 * check-status.js · 屏幕状态真源防漂移扫描（Viden · design-spec-kit 风格）
 *
 * docs/screens-status.js 是门户 index.html 运行时直读的机读真源。
 * 本守卫把「加/删/改屏 → 改这一处」这条 DoD 变成可机检：
 *
 *   ❌ HARD FAIL  重复 id（id 必须唯一·废弃也不复用）
 *   ❌ HARD FAIL  非法 state / kind 枚举
 *   ❌ HARD FAIL  track 不在 tracks[] 里
 *   ❌ HARD FAIL  file 指向的 HTML 实际不存在（悬空链接 → 门户点了 404）
 *   ⚠  WARN       state=BUILT/REVIEWED 却无 file（已建却点不进）
 *   ⚠  WARN       同一 file 被多个屏引用
 *
 * ─────────────────────────────────────────────────────────────
 *  怎么跑：read_file 本文件 → 整个粘到 run_script。
 *  只用沙箱 helper：readFile / ls / log。纯只读、无写盘。
 *  末行 `RESULT: PASS|FAIL`。
 * ═════════════════════════════════════════════════════════════*/

const STATUS_PATH = 'docs/screens-status.js';
const STATES = ['PLANNED','WIP','BUILT','REVIEWED','ARCHIVED','DROPPED'];
const KINDS  = ['kit','inline','doc','arch','ref','star'];
const DONE   = ['BUILT','REVIEWED'];
const OFFLEDGER = ['ARCHIVED','DROPPED'];

// ── 读真源（浏览器 global 风格 .js）：在隔离 window 上求值 ──
const code = await readFile(STATUS_PATH);
const w = {};
new Function('window', code)(w);
const S = w.VIDEN_STATUS;
if (!S || !Array.isArray(S.screens)) { log('✗ 解析失败：缺 window.VIDEN_STATUS.screens'); log('\nRESULT: FAIL'); }
else {

const trackIds = (S.tracks || []).map(t => t.id);
let fail = false;

// 1) 重复 id
const idHits = {};
S.screens.forEach(s => { (idHits[s.id] = idHits[s.id] || []).push(s); });
const dupIds = Object.entries(idHits).filter(([, a]) => a.length > 1);

// 2) 枚举 / track 合法性
const badState = S.screens.filter(s => !STATES.includes(s.state));
const badKind  = S.screens.filter(s => s.kind && !KINDS.includes(s.kind));
const badTrack = S.screens.filter(s => !trackIds.includes(s.track));

// 3) file 存在性（按目录 ls 比对）
const dirCache = {};
async function exists(path) {
  const i = path.lastIndexOf('/');
  const dir = i === -1 ? '' : path.slice(0, i);
  const name = i === -1 ? path : path.slice(i + 1);
  if (!(dir in dirCache)) {
    try { dirCache[dir] = await ls(dir); } catch (e) { dirCache[dir] = null; }
  }
  const listing = dirCache[dir];
  if (!listing) return false;
  return listing.some(f => f === name || f === path || (f.name && f.name === name));
}
const dangling = [];
for (const s of S.screens) {
  if (s.file && !(await exists(s.file))) dangling.push(s);
}

// 4) WARN：已建无 file / 同 file 多屏
const builtNoFile = S.screens.filter(s => DONE.includes(s.state) && !s.file);
const fileHits = {};
S.screens.forEach(s => { if (s.file) (fileHits[s.file] = fileHits[s.file] || []).push(s.id); });
const dupFiles = Object.entries(fileHits).filter(([, ids]) => ids.length > 1);

// ── 报告 ──
const ledger = S.screens.filter(s => !OFFLEDGER.includes(s.state));
const builtN = ledger.filter(s => DONE.includes(s.state)).length;
log(`scanned ${STATUS_PATH} · ${S.screens.length} 屏 · ${trackIds.length} 轨 · 进度 ${builtN}/${ledger.length} 已建 · updated ${S.updated || '—'}`);

if (dupIds.length) { fail = true; log(`\n✗ ${dupIds.length} 个重复 id：`); dupIds.forEach(([id, a]) => log(`    ${id} ×${a.length}`)); }
else log('✓ id 唯一');

if (badState.length) { fail = true; log(`\n✗ 非法 state：`); badState.forEach(s => log(`    ${s.id} → "${s.state}"（合法：${STATES.join('/')}）`)); }
else log('✓ state 枚举合法');

if (badKind.length) { fail = true; log(`\n✗ 非法 kind：`); badKind.forEach(s => log(`    ${s.id} → "${s.kind}"（合法：${KINDS.join('/')}）`)); }
else log('✓ kind 枚举合法');

if (badTrack.length) { fail = true; log(`\n✗ track 不在 tracks[]：`); badTrack.forEach(s => log(`    ${s.id} → "${s.track}"（合法：${trackIds.join('/')}）`)); }
else log('✓ track 合法');

if (dangling.length) { fail = true; log(`\n✗ ${dangling.length} 条 file 悬空（文件不存在）：`); dangling.forEach(s => log(`    ${s.id} → ${s.file}`)); }
else log('✓ 所有 file 均存在');

if (builtNoFile.length) { log(`\n⚠ ${builtNoFile.length} 屏已建却无 file（门户点不进）：`); builtNoFile.forEach(s => log(`    ${s.id} [${s.state}]`)); }
else log('✓ 已建屏均有 file');

if (dupFiles.length) { log(`\n⚠ ${dupFiles.length} 个 file 被多屏引用：`); dupFiles.forEach(([f, ids]) => log(`    ${f} ← ${ids.join(', ')}`)); }
else log('✓ file 未被多屏复用');

// 5) 设计稿 doc[] 校验（索引页导航真源）
const docs = Array.isArray(S.docs) ? S.docs : [];
if (docs.length) {
  const dBadTrack = docs.filter(d => !trackIds.includes(d.track));
  const dBadKind  = docs.filter(d => d.kind && !KINDS.includes(d.kind));
  const dDangling = [];
  for (const d of docs) { if (d.file && !(await exists(d.file))) dDangling.push(d); }
  const dBuilt = docs.filter(d => DONE.includes(d.state)).length;
  log(`\ndocs[] 设计稿 · ${docs.length} 项 · ${dBuilt} 已建`);
  if (dBadTrack.length) { fail = true; log(`✗ doc track 非法：`); dBadTrack.forEach(d => log(`    ${d.track}/${d.nav}`)); } else log('✓ doc track 合法');
  if (dBadKind.length)  { fail = true; log(`✗ doc kind 非法：`); dBadKind.forEach(d => log(`    ${d.track}/${d.nav} → ${d.kind}`)); } else log('✓ doc kind 合法');
  if (dDangling.length) { fail = true; log(`✗ ${dDangling.length} 条 doc file 悬空：`); dDangling.forEach(d => log(`    ${d.track}/${d.nav} → ${d.file}`)); } else log('✓ 所有 doc file 均存在');
}

log(`\nRESULT: ${fail ? 'FAIL' : 'PASS'}`);
}
