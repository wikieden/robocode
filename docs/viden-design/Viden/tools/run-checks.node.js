#!/usr/bin/env node
/**
 * run-checks.node.js · 裸 node 复现 guard（编码侧 CI 用 · 复审回执 🟢5）
 *
 * 浏览器沙箱(run_script)里的 check-*.js 依赖 4 个 helper：readFile / saveFile / ls / log。
 * 本 wrapper 用 fs 提供同名 helper，把每个 check 脚本包成 async 函数执行——
 * **check 脚本零改动**，两个环境跑同一份源。
 *
 * 用法（项目根目录）：
 *   node tools/run-checks.node.js                 # 全部 check
 *   node tools/run-checks.node.js tokens icons    # 只跑指定项
 *   node tools/run-checks.node.js --write-baseline tokens   # 重固化 baseline
 *
 * 与沙箱的差异：node 的 fs 读 CJK/括号文件名无障碍 → 不需要 _scan 暂存机制；
 * wrapper 注入 `__NODE__ = true`，check 脚本里的 BLOCKED 分支不会触发
 * （isSandboxBlocked 被 stub 成恒 false）。
 * 退出码：全 PASS = 0；任何 FAIL/BLOCKED = 1（CI 直接用）。
 */
'use strict';
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const argv = process.argv.slice(2);
const writeBaseline = argv.includes('--write-baseline');
const picks = argv.filter(a => !a.startsWith('--'));

const CHECKS = ['tokens', 'icons', 'tui-glyphs', 'changelog', 'status'];
const toRun = picks.length ? picks : CHECKS;

function makeHelpers(logs) {
  return {
    log: (...a) => { const s = a.join(' '); logs.push(s); console.log('   ' + s); },
    readFile: async p => fs.promises.readFile(path.join(ROOT, p), 'utf8'),
    saveFile: async (p, data) => {
      const abs = path.join(ROOT, p);
      await fs.promises.mkdir(path.dirname(abs), { recursive: true });
      await fs.promises.writeFile(abs, typeof data === 'string' ? data : JSON.stringify(data));
    },
    ls: async p => {
      try { return await fs.promises.readdir(path.join(ROOT, p || '.')); }
      catch { return []; }
    },
  };
}

async function runCheck(name) {
  const file = path.join(__dirname, `check-${name}.js`);
  if (!fs.existsSync(file)) { console.log(`✗ tools/check-${name}.js 不存在`); return 'FAIL'; }
  let src = fs.readFileSync(file, 'utf8');
  // node 环境：文件名无 CJK 盲区 → stub 掉沙箱暂存机制
  // 兼容两种写法：function isSandboxBlocked(p){…} 与 const isSandboxBlocked = p => …;
  src = src
    .replace(/function isSandboxBlocked\([^)]*\)\s*\{[^}]*\}/,
             'function isSandboxBlocked(){return false;}')
    .replace(/const isSandboxBlocked\s*=\s*[^;]*;/,
             'const isSandboxBlocked = () => false;');
  if (writeBaseline) src = src.replace(/const args = \[\];?/, "const args = ['--write-baseline'];");
  const logs = [];
  const h = makeHelpers(logs);
  console.log(`\n▶ check-${name}`);
  try {
    const fn = new Function('readFile', 'saveFile', 'ls', 'log',
      `return (async () => { ${src}\n })();`);
    await fn(h.readFile, h.saveFile, h.ls, h.log);
  } catch (e) {
    console.log('   ✗ 执行异常: ' + e.message);
    return 'FAIL';
  }
  // RESULT 行可能带前导换行/空白（如 log('\nRESULT: PASS')）—— 用 includes 定位、截取尾部
  const last = [...logs].reverse().find(l => l.includes('RESULT:'));
  return last ? last.slice(last.indexOf('RESULT:') + 7).trim() : 'FAIL';
}

(async () => {
  const results = {};
  for (const name of toRun) results[name] = await runCheck(name);
  console.log('\n══ summary ══');
  let bad = false;
  for (const [k, v] of Object.entries(results)) {
    console.log(`  ${v === 'PASS' ? '✓' : '✗'} ${k}: ${v}`);
    if (v !== 'PASS') bad = true;
  }
  process.exit(bad ? 1 : 0);
})();
