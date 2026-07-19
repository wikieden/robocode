/**
 * check-tui-glyphs.js · TUI 字形纪律防漂移扫描（Viden · check-icons.js 的 TUI 兄弟）
 *
 * TUI 图标 = 等宽栅格里内联的 Unicode 几何字形（◆ ▶ ✓ ▣ ◌ ⏸ ✗ · box-drawing），
 * 不是 SVG，无法（也不该）抽成代码模块。它的单一真源是**字形词表**：
 *   语义→字形+颜色 见 DESIGN-REF「TUI 字形词表」(锚定 T4 §08「颜色即状态」)。
 * 本 guard 守住其中**可机判**的一条铁律：TUI 里**不准出现 emoji**
 *   —— emoji 按字体/终端各家渲染不一、撑破等宽栅格（项目原则：用几何字形，不用 emoji）。
 *
 * 只扫 **TUI/**（GUI 走 check-icons.js）。跳过 `_archive`。
 * 命中规则：
 *   ❌ emoji  astral 象形面 `\u{1F000}-\u{1FAFF}` 或 VS16 `\uFE0F` 强制 emoji
 *             （刻意不扫 ◆ ▶ ✓ ✗ ⏸ ◌ ● ○ ⠋ 等 BMP 几何/box-drawing/braille——那是 TUI 设计语言）
 *
 * baseline：tools/check-tui-glyphs.baseline.json（grandfather 借鉴页里**有意展示的反面 emoji 样例**，
 *   如 opencode/hermes 借鉴的 😀「每行 emoji 图标 — 与字形集冲突」自带规则说明）。
 *   只报增量：新增 emoji = FAIL。重写 baseline → args=['--write-baseline']。
 *
 *  ⚠ CJK/括号文件名盲区：同 check-tokens/check-icons——BLOCKED 时按打印清单 ASCII 暂存到 _scan/ 重跑。
 *  怎么跑：read_file → 整个粘到 run_script。helper：readFile / saveFile / ls / log。末行 `RESULT: PASS|FAIL|BLOCKED`。
 * ═════════════════════════════════════════════════════════════*/

const args = [];

const SCAN_ROOTS = ['TUI'];                  // 只扫 TUI（GUI 走 check-icons.js）
const CODE_EXT   = /\.(js|jsx|ts|tsx|html|vue|css)$/i;
const SKIP_DIRS  = new Set(['node_modules', 'dist', 'build', '.git', '_archive', 'vendor']);
const isSkipFile = () => false;

const BASELINE_PATH = 'tools/check-tui-glyphs.baseline.json';

const RULES = [
  { kind: 'emoji', re: /[\u{1F000}-\u{1FAFF}\u{FE0F}]/gu },
];

const stripHtml = s => s.replace(/<!--[\s\S]*?-->/g, m => ' '.repeat(m.length));
const stripJs   = s => s.replace(/\/\*[\s\S]*?\*\//g, m => ' '.repeat(m.length)).replace(/\/\/[^\n]*/g, m => ' '.repeat(m.length));
const stripCss  = s => s.replace(/\/\*[\s\S]*?\*\//g, m => ' '.repeat(m.length));
const extOf  = p => p.slice(p.lastIndexOf('.')).toLowerCase();
const strip  = (s, ext) => ext === '.html' || ext === '.vue' ? stripHtml(s) : ext === '.css' ? stripCss(s) : stripJs(s);
const lineOf = (src, idx) => { let l = 1; for (let i = 0; i < idx; i++) if (src.charCodeAt(i) === 10) l++; return l; };

async function walk(dir, out) {
  let entries; try { entries = await ls(dir); } catch { return; }
  if (!entries || !entries.length) return;
  for (const name of entries) {
    const path = dir ? dir + '/' + name : name;
    if (CODE_EXT.test(name)) { if (!isSkipFile(path)) out.push(path); }
    else if (!name.includes('.') && !SKIP_DIRS.has(name)) await walk(path, out);
  }
}
async function collectFiles() { const out = []; for (const r of SCAN_ROOTS) await walk(r, out); return [...new Set(out)]; }

const PARALLEL_BATCH = 24;
const STAGE_DIR = '_scan';
const isSandboxBlocked = p => /[()]/.test(p) || /[^\x00-\x7F]/.test(p);

function scanText(src, file, readPath) {
  const hits = [];
  const ext = extOf(readPath || file);
  const cleaned = strip(src, ext);
  for (const { kind, re } of RULES) {
    let m; re.lastIndex = 0;
    while ((m = re.exec(cleaned)) !== null) { hits.push({ file, line: lineOf(src, m.index), kind, match: m[0] }); if (m.index === re.lastIndex) re.lastIndex++; }
  }
  return hits;
}
async function readStageManifest() { try { return JSON.parse(await readFile(STAGE_DIR + '/_manifest.json')); } catch { return null; } }

async function scanAll(files, stage) {
  const allHits = [], blocked = []; stage = stage || {};
  for (let i = 0; i < files.length; i += PARALLEL_BATCH) {
    const batch = files.slice(i, i + PARALLEL_BATCH);
    await Promise.all(batch.map(async f => {
      if (isSandboxBlocked(f)) {
        const staged = stage[f];
        if (staged) { try { allHits.push(...scanText(await readFile(staged), f, staged)); return; } catch { blocked.push(f); return; } }
        blocked.push(f); return;
      }
      try { allHits.push(...scanText(await readFile(f), f, f)); } catch { /* 读不到 */ }
    }));
  }
  return { allHits, blocked };
}

const keyOf = h => `${h.file}::${h.kind}::${h.match}`;
function baselineKeys(b) { const s = new Set(); if (b && b.files) for (const [f, arr] of Object.entries(b.files)) for (const e of arr) s.add(`${f}::${e.kind}::${e.match}`); return s; }
function buildBaseline(hits, reason) {
  const grouped = {};
  for (const h of hits) (grouped[h.file] = grouped[h.file] || []).push({ line: h.line, kind: h.kind, match: h.match });
  for (const f of Object.keys(grouped)) grouped[f].sort((a, b) => a.line - b.line || a.match.localeCompare(b.match));
  return { note: '已认证保留的 TUI emoji 命中（借鉴页有意展示的反面样例）。新增 emoji 需修代码或显式加到这里。', generatedAt: new Date().toISOString().slice(0, 10), reason: reason || 'baseline write', totalEntries: hits.length, files: grouped };
}

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
    if (removed.length > 0) { log(`\n✓ ${removed.length} 处 baseline 命中已清理`); for (const k of removed.slice(0, 20)) log('    cleaned: ' + k); log(`  → 跑一次 args=['--write-baseline'] 同步 baseline\n`); }
    if (news.length > 0) {
      log(`\n✗ ${news.length} 处新增 emoji（TUI 禁 emoji）：`);
      const byFile = {};
      for (const h of news) (byFile[h.file] = byFile[h.file] || []).push(h);
      for (const [f, arr] of Object.entries(byFile)) { log(`  ${f}`); for (const h of arr) log(`    L${h.line}  [${h.kind}]  ${h.match}`); }
      log(`\n修法：换成 DESIGN-REF「TUI 字形词表」里的几何字形（✓ done · ✗ fail · ⏸ gate · ◆ clarify · ▶ run · ◌ wait · ▣ skill…），颜色按 T4 §08。`);
      log(`确属有意保留（反面样例）→ args=['--write-baseline'] 并在 CHANGELOG 写明理由。`);
      log(`\nRESULT: FAIL`);
    } else if (removed.length === 0) { log('✓ check-tui-glyphs: 0 新增 · 0 减少 · baseline 保持不变'); log(`\nRESULT: PASS`); }
    else { log(`\nRESULT: PASS`); }
  }
}
