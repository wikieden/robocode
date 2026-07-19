/**
 * check-icons.js · GUI 图标纪律防漂移扫描（Viden · 对标 check-tokens.js）
 *
 * 把 SPEC `D-ICON` / DESIGN-REF「GUI 图标目录」里**可机判**的纪律变成 DoD 守卫，
 * 直接挡住「图标漂移」最常见的三种：每页各自内联画一套 rail/工具图标、
 * 重新定义 VRAIL_ICONS 注册表、往 GUI 里塞 emoji。
 *
 * 只扫 **GUI/**（TUI 是另一套规范——box-drawing / Unicode 几何符合法，绝不在此扫）。
 * 跳过 `_archive`（冻结快照）与 `gui-icons.jsx` 本体（图标真源，合法定义于此）。
 *
 * 命中规则（在 GUI 页 / 组件，而非 gui-icons.jsx 里出现即违规）：
 *   ❌ emoji          astral 象形 emoji `\u{1F000}-\u{1FAFF}` 或 VS16 `\uFE0F`
 *                     （刻意不扫 ✓ ✕ ⏸ ◆ ◉ ● ▸ 等 BMP 几何/box-drawing 字符——那是设计语言）
 *   ❌ icon-registry  重新定义 `VRAIL_ICONS` / `VRAILICON` 之类页内图标注册表
 *   ❌ inline-ic      页面自己 author `<svg class="ic">` / `className="ic sm"`
 *                     （图标该走 {ICONS.x} / <GuiIcon/>，页面不该手画 kit 图标）
 *
 * ─────────────────────────────────────────────────────────────
 *  baseline：tools/check-icons.baseline.json 列「已认证保留」的命中快照。
 *  只报增量：清掉旧命中 = OK / 新增命中 = FAIL。重写 baseline 把 args 设 ['--write-baseline']。
 *  （组件库 GUI 的 showcase 内联 .ic 图标是有意陈列 → 首跑进 baseline 被 grandfather。）
 *
 *  ⚠ CJK / 括号文件名盲区：同 check-tokens——run_script readFile 拒收含 CJK 或 () 的路径。
 *  BLOCKED 时按打印的清单 ASCII 暂存到 _scan/（copy_files + _manifest.json），重跑即补扫，扫完 delete _scan/。
 *
 *  怎么跑：read_file 本文件 → 整个粘到 run_script。helper：readFile / saveFile / ls / log。末行 `RESULT: PASS|FAIL|BLOCKED`。
 * ═════════════════════════════════════════════════════════════*/

// ─── 配置 ──────────────────────────────────────────────────────

const args = [];   // 例：['--write-baseline'] 把当前命中固化为新 baseline

const SCAN_ROOTS = ['GUI'];                 // 只扫 GUI（TUI 另一套规范，不在此）
const CODE_EXT   = /\.(js|jsx|ts|tsx|html|vue)$/i;
const SKIP_DIRS  = new Set(['node_modules', 'dist', 'build', '.git', '_archive', 'vendor']);
// 图标真源本体合法 → 跳过
const isSkipFile = p => /(^|\/)gui-icons\.jsx$/i.test(p);

const BASELINE_PATH = 'tools/check-icons.baseline.json';

// ─── 规则 ──────────────────────────────────────────────────────

const RULES = [
  { kind: 'emoji',         re: /[\u{1F000}-\u{1FAFF}\u{FE0F}]/gu },
  { kind: 'icon-registry', re: /\bVRAIL_?ICONS?\b|\bVRAILICON\b/g },
  { kind: 'inline-ic',     re: /<svg[^>]*class(?:Name)?=["'][^"']*\bic\b[^"']*["']/gi },
];

// 注释置空（保留位置/行号），避免注释里的样例触发
const stripHtml = s => s.replace(/<!--[\s\S]*?-->/g, m => ' '.repeat(m.length));
const stripJs   = s => s.replace(/\/\*[\s\S]*?\*\//g, m => ' '.repeat(m.length))
                       .replace(/\/\/[^\n]*/g, m => ' '.repeat(m.length));
const extOf  = p => p.slice(p.lastIndexOf('.')).toLowerCase();
const strip  = (s, ext) => ext === '.html' || ext === '.vue' ? stripHtml(s) : stripJs(s);

function lineOf(src, idx) { let l = 1; for (let i = 0; i < idx; i++) if (src.charCodeAt(i) === 10) l++; return l; }

// ─── 收集文件 ──────────────────────────────────────────────────

async function walk(dir, out) {
  let entries;
  try { entries = await ls(dir); } catch { return; }
  if (!entries || entries.length === 0) return;
  for (const name of entries) {
    const path = dir ? dir + '/' + name : name;
    if (CODE_EXT.test(name)) { if (!isSkipFile(path)) out.push(path); }
    else if (!name.includes('.') && !SKIP_DIRS.has(name)) await walk(path, out);
  }
}
async function collectFiles() { const out = []; for (const r of SCAN_ROOTS) await walk(r, out); return [...new Set(out)]; }

// ─── 扫描 ──────────────────────────────────────────────────────

const PARALLEL_BATCH = 24;
const STAGE_DIR = '_scan';
function isSandboxBlocked(path) { return /[()]/.test(path) || /[^\x00-\x7F]/.test(path); }

function scanText(src, file, readPath) {
  const hits = [];
  const ext = extOf(readPath || file);
  const cleaned = strip(src, ext);
  for (const { kind, re } of RULES) {
    let m; re.lastIndex = 0;
    while ((m = re.exec(cleaned)) !== null) {
      let match = m[0];
      if (kind === 'inline-ic') match = match.slice(0, 40);            // 截断，稳定 key
      hits.push({ file, line: lineOf(src, m.index), kind, match });
      if (m.index === re.lastIndex) re.lastIndex++;
    }
  }
  return hits;
}

async function readStageManifest() {
  try { return JSON.parse(await readFile(STAGE_DIR + '/_manifest.json')); } catch { return null; }
}

async function scanAll(files, stage) {
  const allHits = [], blocked = [];
  stage = stage || {};
  for (let i = 0; i < files.length; i += PARALLEL_BATCH) {
    const batch = files.slice(i, i + PARALLEL_BATCH);
    await Promise.all(batch.map(async f => {
      if (isSandboxBlocked(f)) {
        const staged = stage[f];
        if (staged) { try { allHits.push(...scanText(await readFile(staged), f, staged)); return; } catch { blocked.push(f); return; } }
        blocked.push(f); return;
      }
      try { allHits.push(...scanText(await readFile(f), f, f)); } catch { /* 读不到：忽略 */ }
    }));
  }
  return { allHits, blocked };
}

// ─── Baseline diff ─────────────────────────────────────────────

function keyOf(h) { return `${h.file}::${h.kind}::${h.match}`; }
function baselineKeys(b) { const s = new Set(); if (b && b.files) for (const [f, arr] of Object.entries(b.files)) for (const e of arr) s.add(`${f}::${e.kind}::${e.match}`); return s; }
function buildBaseline(hits, reason) {
  const grouped = {};
  for (const h of hits) (grouped[h.file] = grouped[h.file] || []).push({ line: h.line, kind: h.kind, match: h.match });
  for (const f of Object.keys(grouped)) grouped[f].sort((a, b) => a.line - b.line || a.match.localeCompare(b.match));
  return { note: '已认证保留的 GUI 图标违规清单。新增需修代码或显式加到这里。', generatedAt: new Date().toISOString().slice(0, 10), reason: reason || 'baseline write', totalEntries: hits.length, files: grouped };
}

// ─── Main ──────────────────────────────────────────────────────

const writeBaseline = args.includes('--write-baseline');
const files = await collectFiles();
const stage = await readStageManifest();
const { allHits: hits, blocked } = await scanAll(files, stage);
log(`scanned ${files.length} files · ${hits.length} hits` + (stage ? ` · 暂存补扫生效` : '') + (blocked.length ? ` · ⚠ ${blocked.length} 覆盖缺口` : ''));

if (blocked.length > 0) {
  log(`\n✗ ${blocked.length} 个文件 run_script 读不到（路径含 CJK / 括号），未被扫描。按下面清单 ASCII 暂存后重跑：`);
  const cf = blocked.map((f, i) => ({ asset: '', dest: `${STAGE_DIR}/${i}.html`, src: f }));
  const manifest = {}; blocked.forEach((f, i) => { manifest[f] = `${STAGE_DIR}/${i}.html`; });
  log(`\n【步骤 1】copy_files 参数 files=\n` + JSON.stringify(cf, null, 0));
  log(`\n【步骤 2】把以下写入 ${STAGE_DIR}/_manifest.json（saveFile），再重跑本脚本：\n` + JSON.stringify(manifest, null, 0));
  log(`\n【步骤 3】扫完 delete_file ["${STAGE_DIR}"]。`);
  log(`\nRESULT: BLOCKED`);
} else if (writeBaseline) {
  await saveFile(BASELINE_PATH, JSON.stringify(buildBaseline(hits, 'manual --write-baseline'), null, 2) + '\n');
  log(`✓ baseline rewritten: ${BASELINE_PATH} (${hits.length} entries)`);
} else {
  let baseline = null;
  try { baseline = JSON.parse(await readFile(BASELINE_PATH)); } catch { /* none */ }
  if (!baseline) {
    await saveFile(BASELINE_PATH, JSON.stringify(buildBaseline(hits, 'first run'), null, 2) + '\n');
    log(`✓ baseline created: ${BASELINE_PATH} (${hits.length} entries) — 复查后再跑一次进入 diff 模式`);
  } else {
    const allowed = baselineKeys(baseline);
    const news = hits.filter(h => !allowed.has(keyOf(h)));
    const removed = [...allowed].filter(k => !hits.some(h => keyOf(h) === k));
    log(`baseline: ${allowed.size} entries · removed: ${removed.length} · new: ${news.length}`);
    if (removed.length > 0) {
      log(`\n✓ ${removed.length} 处 baseline 命中已清理`);
      for (const k of removed.slice(0, 20)) log('    cleaned: ' + k);
      log(`  → 跑一次 args=['--write-baseline'] 同步 baseline\n`);
    }
    if (news.length > 0) {
      log(`\n✗ ${news.length} 处新增图标违规：`);
      const byFile = {};
      for (const h of news) (byFile[h.file] = byFile[h.file] || []).push(h);
      for (const [f, arr] of Object.entries(byFile)) { log(`  ${f}`); for (const h of arr) log(`    L${h.line}  [${h.kind}]  ${h.match}`); }
      log(`\n修法：`);
      log(`  1. rail/工具图标 → 取 {ICONS.x} 或 <GuiIcon name=.../>（真源 GUI/gui-icons.jsx）`);
      log(`  2. 缺的图标 → 先加进 gui-icons.jsx + DESIGN-REF「GUI 图标目录」登记，再引用`);
      log(`  3. GUI 禁 emoji → 换 gui-icons 线性 SVG（lock/robot/game/ml…）`);
      log(`  4. 确属有意保留 → args=['--write-baseline'] 并在 CHANGELOG 写明理由`);
      log(`\nRESULT: FAIL`);
    } else if (removed.length === 0) {
      log('✓ check-icons: 0 新增 · 0 减少 · baseline 保持不变');
      log(`\nRESULT: PASS`);
    } else { log(`\nRESULT: PASS`); }
  }
}
