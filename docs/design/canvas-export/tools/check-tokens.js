/**
 * check-tokens.js · token 纪律防漂移扫描（design-spec-kit · 项目通用）
 *
 * ★ 拿到本脚本：按你的目录调下方 SCAN_ROOTS（默认已列常见样式/页面目录，不存在的自动跳过）。
 *   首次跑会自动生成 baseline（接受现状），之后只报新增违规。
 *
 * 用途：扫描项目里的 .css/.js/.jsx/.ts/.tsx/.html/.vue，找违反 token 纪律的代码：
 *   ❌ 裸 hex          `#abc` / `#abcdef` / `#abcdef88`
 *   ❌ 裸 rgba         `rgba(0,0,0,.5)`
 *   ❌ 假 fallback     `var(--x, #fff)` / `var(--x, rgba(...))`
 *                     （允许 `var(--x, var(--y))` token→token fallback）
 *
 * CLAUDE.md 约定：颜色一律 `var(--*)`，数值唯一真源在 tokens.css。
 * 本脚本把这条「自觉纪律」变成可机检的 DoD 守卫——直接挡住「页面漂移」里最常见的一种：
 * 这页 #3b82f6、那页 #3a80f5，颜色越走越散。
 *
 * ─────────────────────────────────────────────────────────────
 *  扫哪些 / 跳哪些
 * ─────────────────────────────────────────────────────────────
 *  自动遍历 SCAN_ROOTS（不硬编码文件名 → 新增页不漏扫、不需维护清单）。
 *  跳过：
 *    · 任何 `tokens.css`  —— token 唯一真源，hex/rgba 合法定义于此
 *    · SKIP_DIRS 里的目录 —— 依赖 / 构建产物 / 归档 / 工具本身
 *    · 非代码文件         —— 按扩展名只收下面 CODE_EXT
 *
 * ─────────────────────────────────────────────────────────────
 *  baseline 机制
 * ─────────────────────────────────────────────────────────────
 *  tools/check-tokens.baseline.json 列出「已认证保留」的违规快照。
 *  脚本只报增量：清掉旧违规 = OK / 新增违规 = FAIL。
 *  要把当前所有违规重新固化为 baseline，把下方 args 设成 ['--write-baseline']。
 *
 * ─────────────────────────────────────────────────────────────
 *  怎么跑：read_file 本文件 → 整个粘到 run_script。
 *  只用沙箱 helper：readFile / saveFile / ls / log。末行 `RESULT: PASS|FAIL`。
 * ═════════════════════════════════════════════════════════════*/

// ─── 配置（接手第一件事：按你的项目改这里）──────────────────────

const args = [];   // 例：['--write-baseline'] 把当前扫描结果固化为新 baseline

// 放样式 / 组件 / 页面的目录。多列无妨——不存在的目录会被自动跳过。
const SCAN_ROOTS = ['Core', 'GUI', 'TUI', 'styles', 'css', 'src', 'components', 'pages', 'app'];
const ROOT_FILES = ['index.html'];          // 项目根的散件
const CODE_EXT   = /\.(css|scss|less|js|jsx|ts|tsx|vue|svelte|html)$/i;

// 整目录级 skip（依赖 / 构建产物 / 归档 / 工具 / 版本库）
const SKIP_DIRS = new Set(['node_modules', 'dist', 'build', '.git', '_archive', 'tools', 'uploads', 'vendor']);
// 整文件级 skip：token 唯一真源（hex/rgba 合法定义于此）
const isSkipFile = p => /(^|\/)tokens\.css$/i.test(p);

const BASELINE_PATH = 'tools/check-tokens.baseline.json';

// ─── 规则 ──────────────────────────────────────────────────────

// 单一组合 regex，按 alternative 顺序匹配：fake-fallback 优先 → 裸 hex/rgba 兜底
const RE = /var\(\s*--[a-z0-9-]+\s*,\s*#[0-9A-Fa-f]{3,8}\s*\)|var\(\s*--[a-z0-9-]+\s*,\s*rgba?\([^)]*\)\s*\)|#[0-9A-Fa-f]{3,8}\b|\brgba?\(\s*\d+\s*,\s*\d+\s*,\s*\d+\s*(?:,\s*[\d.]+\s*)?\)/gi;

function classify(m) {
  if (m.startsWith('var(')) return m.includes('#') ? 'fake-fallback-hex' : 'fake-fallback-rgba';
  return m.startsWith('#') ? 'bare-hex' : 'bare-rgba';
}

// 用空格替换注释内容（保留位置，方便行号反查）
const stripCss  = s => s.replace(/\/\*[\s\S]*?\*\//g, m => ' '.repeat(m.length));
const stripHtml = s => s.replace(/<!--[\s\S]*?-->/g, m => ' '.repeat(m.length));
const stripJs   = s => s.replace(/\/\*[\s\S]*?\*\//g, m => ' '.repeat(m.length))
                       .replace(/\/\/[^\n]*/g, m => ' '.repeat(m.length));
const extOf  = p => p.slice(p.lastIndexOf('.')).toLowerCase();
const strip  = (s, ext) => ext === '.css' || ext === '.scss' || ext === '.less' ? stripCss(s)
                         : ext === '.html' || ext === '.vue' || ext === '.svelte' ? stripHtml(s)
                         : stripJs(s);

function lineOf(src, idx) {
  let l = 1;
  for (let i = 0; i < idx; i++) if (src.charCodeAt(i) === 10) l++;
  return l;
}

// ─── 收集文件（递归遍历 SCAN_ROOTS）─────────────────────────────

async function walk(dir, out) {
  let entries;
  try { entries = await ls(dir); } catch { return; }
  if (!entries || entries.length === 0) return;   // 文件 ls → []，自然终止
  for (const name of entries) {
    const path = dir ? dir + '/' + name : name;
    if (CODE_EXT.test(name)) {
      if (!isSkipFile(path)) out.push(path);
    } else if (!name.includes('.') && !SKIP_DIRS.has(name)) {
      // 无扩展名 → 当目录递归（dotfiles / 图片等被扩展名过滤天然排除）
      await walk(path, out);
    }
  }
}

async function collectFiles() {
  const out = [];
  for (const r of SCAN_ROOTS) await walk(r, out);
  for (const f of ROOT_FILES) if (!isSkipFile(f)) out.push(f);
  return [...new Set(out)];   // SCAN_ROOTS 可能重叠，去重
}

// ─── 扫描 ──────────────────────────────────────────────────────

const PARALLEL_BATCH = 24;

async function scanAll(files) {
  const allHits = [];
  for (let i = 0; i < files.length; i += PARALLEL_BATCH) {
    const batch = files.slice(i, i + PARALLEL_BATCH);
    const contents = await Promise.all(batch.map(async f => {
      try { return { f, src: await readFile(f) }; }
      catch { return { f, src: null }; }
    }));
    for (const { f, src } of contents) {
      if (!src) continue;
      const ext = extOf(f);
      const cleaned = strip(src, ext);
      let m; RE.lastIndex = 0;
      while ((m = RE.exec(cleaned)) !== null) {
        allHits.push({ file: f, line: lineOf(src, m.index), kind: classify(m[0]), match: m[0] });
      }
    }
  }
  return allHits;
}

// ─── Baseline diff ─────────────────────────────────────────────

function keyOf(h) { return `${h.file}::${h.kind}::${h.match}`; }

function baselineKeys(b) {
  const s = new Set();
  if (!b || !b.files) return s;
  for (const [f, arr] of Object.entries(b.files)) {
    for (const e of arr) s.add(`${f}::${e.kind}::${e.match}`);
  }
  return s;
}

function buildBaseline(hits, reason) {
  const grouped = {};
  for (const h of hits) (grouped[h.file] = grouped[h.file] || []).push({
    line: h.line, kind: h.kind, match: h.match
  });
  for (const f of Object.keys(grouped)) {
    grouped[f].sort((a, b) => a.line - b.line || a.match.localeCompare(b.match));
  }
  return {
    note: '已认证保留的 token 违规清单。新增违规需修代码或显式加到这里。',
    generatedAt: new Date().toISOString().slice(0, 10),
    reason: reason || 'baseline write',
    totalEntries: hits.length,
    files: grouped,
  };
}

// ─── Main（top-level await — run_script 直接执行）──────────────

const writeBaseline = args.includes('--write-baseline');

const files = await collectFiles();
const hits = await scanAll(files);
log(`scanned ${files.length} files · ${hits.length} violations`);

if (writeBaseline) {
  await saveFile(BASELINE_PATH, JSON.stringify(buildBaseline(hits, 'manual --write-baseline'), null, 2) + '\n');
  log(`✓ baseline rewritten: ${BASELINE_PATH} (${hits.length} entries)`);
} else {
  let baseline = null;
  try { baseline = JSON.parse(await readFile(BASELINE_PATH)); } catch { /* no baseline */ }

  if (!baseline) {
    await saveFile(BASELINE_PATH, JSON.stringify(buildBaseline(hits, 'first run'), null, 2) + '\n');
    log(`✓ baseline created: ${BASELINE_PATH} (${hits.length} entries) — 复查后再跑一次进入 diff 模式`);
  } else {
    const allowed = baselineKeys(baseline);
    const news    = hits.filter(h => !allowed.has(keyOf(h)));
    const removed = [...allowed].filter(k => !hits.some(h => keyOf(h) === k));

    log(`baseline: ${allowed.size} entries · removed: ${removed.length} · new: ${news.length}`);

    if (removed.length > 0) {
      log(`\n✓ ${removed.length} 处 baseline 违规已被清理（干得漂亮）`);
      for (const k of removed.slice(0, 20)) log('    cleaned: ' + k);
      if (removed.length > 20) log(`    ... 还有 ${removed.length - 20} 处`);
      log(`  → 跑一次 args=['--write-baseline'] 同步 baseline\n`);
    }

    if (news.length > 0) {
      log(`\n✗ ${news.length} 处新增违规：`);
      const byFile = {};
      for (const h of news) (byFile[h.file] = byFile[h.file] || []).push(h);
      for (const [f, arr] of Object.entries(byFile)) {
        log(`  ${f}`);
        for (const h of arr) log(`    L${h.line}  [${h.kind}]  ${h.match}`);
      }
      log(`\n修法：`);
      log(`  1. 优先把 hex / rgba 收编进 tokens.css（推荐）`);
      log(`  2. 确实必须保留：args=['--write-baseline'] 并在 CHANGELOG 写明理由`);
      log(`\nRESULT: FAIL`);
    } else if (removed.length === 0) {
      log('✓ check-tokens: 0 新增 · 0 减少 · baseline 保持不变');
      log(`\nRESULT: PASS`);
    } else {
      log(`\nRESULT: PASS`);
    }
  }
}
