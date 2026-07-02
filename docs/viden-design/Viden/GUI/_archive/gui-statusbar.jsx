/* ════════════════════════════════════════════════════════════════
 * gui-statusbar.jsx · Viden GUI 窗口级底部状态栏（canonical · 单一真源）
 *
 * 所有 GUI 桌面窗口共用的底栏：28px · 等宽 · 左 ⚙ + 中段滚动 ticker + 右状态段。
 * 视觉母版 = D1 驾驶舱 .statusbar；抽成组件后各页零重造、改一处全改。
 *
 * 用法（页面已加载 React + Babel）：
 *   <script type="text/babel" src="gui-statusbar.jsx"></script>   // 在主脚本之前
 *   渲染在 .frame / .win 的最后一个子节点：
 *     <GuiStatusBar items={[['lane','L2 · gameplay'], ...]} right="⏸ 1 gate waiting" />
 *
 * props（都可省，给的是 Aurora 默认占位）：
 *   items : Array<[label, value] | string>  中段滚动指标
 *   right : ReactNode                        右侧固定状态段（默认隐藏）
 *   样式自注入一次，无需单独 link。class 前缀 gui-sb（与 D1 .statusbar 不冲突）。
 * ════════════════════════════════════════════════════════════════ */
(function () {
  const STYLE_ID = 'gui-sb-style';
  if (!document.getElementById(STYLE_ID)) {
    const css = `
.gui-sb{flex:none;height:28px;display:flex;align-items:center;gap:0;padding:0 12px;
  background:var(--bg-void);border-top:1px solid var(--border-soft);
  font-family:var(--font-mono);font-size:11px;color:var(--fg-secondary);overflow:hidden;}
.gui-sb .gui-sb-gear{color:var(--fg-muted);flex:none;padding-right:10px;
  border-right:1px solid var(--border-soft);}
.gui-sb-tickwrap{flex:1;overflow:hidden;
  -webkit-mask-image:linear-gradient(90deg,transparent,black 3%,black 97%,transparent);
  mask-image:linear-gradient(90deg,transparent,black 3%,black 97%,transparent);}
.gui-sb-track{display:inline-flex;white-space:nowrap;animation:gui-sb-roll 30s linear infinite;}
.gui-sb-track:hover{animation-play-state:paused;}
@keyframes gui-sb-roll{from{transform:translateX(0);}to{transform:translateX(-50%);}}
.gui-sb-it{padding:0 13px;flex:none;}
.gui-sb-it .lbl{color:var(--fg-faint);}
.gui-sb-it b{color:var(--fg-primary);font-weight:600;}
.gui-sb-sep{color:var(--fg-faint);flex:none;}
.gui-sb-right{flex:none;color:var(--warning);padding-left:10px;
  border-left:1px solid var(--border-soft);}
.gui-sb .ok{color:var(--success);}.gui-sb .gd{color:var(--gold);}.gui-sb .ac{color:var(--accent);}`;
    const el = document.createElement('style');
    el.id = STYLE_ID;
    el.textContent = css;
    document.head.appendChild(el);
  }

  // Aurora 默认占位（驾驶舱常见指标）——页面可用 items 覆盖
  const DEFAULT_ITEMS = [
    ['provider', 'deepseek · healthy'],
    ['model', 'deepseek-chat'],
    ['ctx', '34.2k / 64k'],
    ['cost', '$0.21'],
    ['mode', 'auto-edit'],
    ['perm', 'ask'],
    ['branch', 'main'],
  ];

  function Item(d) {
    if (Array.isArray(d)) {
      return [React.createElement('span', { className: 'lbl', key: 'l' }, d[0] + ' '),
              React.createElement('b', { key: 'v' }, d[1])];
    }
    return d;
  }

  function GuiStatusBar(props) {
    const items = props.items || DEFAULT_ITEMS;
    const right = props.right;
    const groups = [0, 1].map((g) =>
      React.createElement('span', { key: g, style: { display: 'inline-flex' }, 'aria-hidden': g === 1 },
        items.map((d, i) =>
          React.createElement('span', { key: i, style: { display: 'inline-flex' } },
            React.createElement('span', { className: 'gui-sb-it' }, Item(d)),
            React.createElement('span', { className: 'gui-sb-sep' }, '┆')
          )
        )
      )
    );
    return React.createElement('div', { className: 'gui-sb' },
      React.createElement('span', { className: 'gui-sb-gear', title: '状态项' }, '⚙'),
      React.createElement('div', { className: 'gui-sb-tickwrap' },
        React.createElement('div', {
          className: 'gui-sb-track',
          style: { animationDuration: Math.max(18, items.length * 4) + 's' },
        }, groups)
      ),
      right != null ? React.createElement('span', { className: 'gui-sb-right' }, right) : null
    );
  }

  window.GuiStatusBar = GuiStatusBar;
})();
