/* ════════════════════════════════════════════════════════════════
 * gui-inbox.jsx · Viden 团队收件箱 / 简报 / 变更通告（canonical · 单一真源）
 *
 * 「团队 · 人+agent 通信频道」：闸收件箱(按 viden.toml ownership 路由) +
 * 团队 roster + 移交/自动驾驶 + 团队简报 + 变更通告(通道 A 送到人 / 通道 B
 * 送到 agent memo + ack 追踪)。视觉母版 = D7；抽成组件后旗舰 D1 驾驶舱与
 * D7 设计稿共用同一份，改一处全改。样式自注入(.gi-root 作用域隔离)。
 *
 * 用法（页面已加载 React + Babel + tokens.css）：
 *   <script type="text/babel" src="gui-inbox.jsx"></script>   // 主脚本之前
 *   <InboxView/>            // 自带 收件箱/简报/通告 三标签 + 团队 rail
 *   props: defaultTab 'inbox'|'briefing'|'notice'（默认 'inbox'）
 *          onToast(msg)     认领时回调（可省，内部也有浮层）
 * ════════════════════════════════════════════════════════════════ */
(function () {
  const STYLE_ID = 'gui-inbox-style';
  if (!document.getElementById(STYLE_ID)) {
    const css = `
.gi-root{flex:1;min-height:0;display:flex;flex-direction:column;position:relative;background:var(--bg-base);font-family:var(--font-sans);color:var(--fg-primary);}
.gi-root .vbody{flex:1;display:flex;min-height:0;}
.gi-root .vbodymain{flex:1;min-width:0;display:flex;flex-direction:column;}
.gi-root .cmdbar{flex:none;display:flex;align-items:center;gap:16px;padding:0 18px;border-bottom:1px solid var(--border);background:var(--bg-void);height:50px;}
.gi-root .cmdbar .t{font-size:15px;font-weight:700;display:flex;align-items:center;gap:9px;}
.gi-root .cmdbar .t .pulse{width:8px;height:8px;border-radius:50%;background:var(--accent);box-shadow:0 0 0 3px color-mix(in srgb,var(--accent) 12%,transparent);}
.gi-root .vtabs{display:flex;align-self:stretch;}
.gi-root .vtab{display:flex;align-items:center;gap:7px;padding:0 15px;font-size:13px;font-weight:600;color:var(--fg-muted);cursor:pointer;position:relative;}
.gi-root .vtab:hover{color:var(--fg-secondary);}
.gi-root .vtab.on{color:var(--fg-primary);}
.gi-root .vtab.on::after{content:"";position:absolute;left:10px;right:10px;bottom:-1px;height:2px;background:var(--accent);}
.gi-root .vtab .n{font-family:var(--font-mono);font-size:9.5px;color:var(--accent-bright);background:var(--bg-sel);border:1px solid var(--accent-dim);border-radius:9px;padding:0 6px;}
.gi-root .cmdbar .sp{flex:1;}
.gi-root .cmdbar .meta{font-family:var(--font-mono);font-size:10px;color:var(--fg-faint);}
.gi-root .cmdbar .me{display:flex;align-items:center;gap:7px;font-family:var(--font-mono);font-size:10.5px;color:var(--fg-secondary);}
.gi-root .av{width:24px;height:24px;border-radius:50%;background:var(--bg-sel);border:1px solid var(--accent-dim);display:grid;place-items:center;font-family:var(--font-mono);font-size:9px;font-weight:700;color:var(--accent-bright);flex:none;}
.gi-root .av.g2{border-color:color-mix(in srgb,var(--gold) 50%,transparent);color:var(--gold-bright);}
.gi-root .av.g3{border-color:color-mix(in srgb,var(--progress) 50%,transparent);color:var(--progress);}
.gi-root .av.bot{border-style:dashed;color:var(--fg-muted);border-color:var(--border);}
.gi-root .inner{flex:1;display:grid;grid-template-columns:1fr var(--rail-right);min-height:0;}
.gi-root .inner.full{grid-template-columns:1fr;}
.gi-root .maincol{min-width:0;overflow:hidden auto;scrollbar-width:thin;scrollbar-color:var(--border) transparent;background:var(--bg-base);}
.gi-root .sec{font-family:var(--font-mono);font-size:10px;font-weight:700;letter-spacing:1.5px;text-transform:uppercase;color:var(--fg-muted);display:flex;align-items:center;gap:8px;margin:18px 18px 9px;}
.gi-root .sec::after{content:"";flex:1;height:1px;background:var(--border-soft);}
.gi-root .sec .hint{font-weight:400;letter-spacing:0;text-transform:none;color:var(--fg-faint);}
.gi-root .sec .n{color:var(--fg-faint);}
.gi-root .irow{display:flex;align-items:center;gap:11px;margin:0 18px 8px;background:var(--bg-panel);border:1px solid var(--border-soft);border-radius:10px;padding:calc(var(--card-pad-y) + 2px) 14px;cursor:pointer;}
.gi-root .irow:hover{border-color:var(--fg-muted);}
.gi-root .irow .ki{font-family:var(--font-mono);font-size:12px;width:18px;text-align:center;flex:none;}
.gi-root .irow .tx .t1{font-size:13px;font-weight:600;color:var(--fg-primary);display:flex;align-items:center;gap:8px;flex-wrap:wrap;}
.gi-root .irow .tx .t2{font-family:var(--font-mono);font-size:9.5px;color:var(--fg-muted);margin-top:4px;}
.gi-root .irow .tx .t2 .why{color:var(--fg-faint);}
.gi-root .irow .tx .t2 .a{color:var(--accent);}
.gi-root .irow .sp{flex:1;}
.gi-root .irow .age{font-family:var(--font-mono);font-size:9.5px;color:var(--fg-faint);}
.gi-root .irow .age.hot{color:var(--warning);}
.gi-root .chip{font-family:var(--font-mono);font-size:9px;font-weight:600;border-radius:5px;padding:1px 7px;white-space:nowrap;border:1px solid var(--border);color:var(--fg-secondary);display:inline-flex;align-items:center;gap:5px;}
.gi-root .chip .dot{width:6px;height:6px;border-radius:2px;}
.gi-root .chip.gate{color:var(--gold-bright);border-color:color-mix(in srgb,var(--gold) 50%,transparent);background:color-mix(in srgb,var(--gold) 6%,transparent);}
.gi-root .chip.ask{color:var(--warning);border-color:color-mix(in srgb,var(--warning) 50%,transparent);background:color-mix(in srgb,var(--warning) 6%,transparent);}
.gi-root .chip.field{color:var(--error);border-color:color-mix(in srgb,var(--error) 45%,transparent);background:color-mix(in srgb,var(--error) 6%,transparent);}
.gi-root .ibtn{font-family:var(--font-sans);font-size:11.5px;font-weight:600;border-radius:7px;padding:5px 13px;cursor:pointer;border:1px solid var(--border);color:var(--fg-secondary);background:transparent;white-space:nowrap;}
.gi-root .ibtn:hover{color:var(--fg-primary);border-color:var(--fg-muted);}
.gi-root .ibtn.primary{color:var(--on-accent);background:var(--accent);border-color:var(--accent);font-weight:700;}
.gi-root .ibtn.primary:hover{background:var(--accent-bright);}
.gi-root .irow.dim{opacity:.65;}
.gi-root .irow .who{display:flex;align-items:center;gap:6px;font-family:var(--font-mono);font-size:9.5px;color:var(--fg-muted);}
.gi-root .rail{background:var(--bg-panel);border-left:1px solid var(--border-soft);display:flex;flex-direction:column;overflow:hidden auto;scrollbar-width:thin;scrollbar-color:var(--border) transparent;}
.gi-root .rail .hd{padding:15px 17px 9px;font-family:var(--font-mono);font-size:10px;font-weight:700;letter-spacing:1.5px;text-transform:uppercase;color:var(--fg-muted);display:flex;align-items:center;gap:8px;}
.gi-root .rail .hd::after{content:"";flex:1;height:1px;background:var(--border-soft);}
.gi-root .mem{display:flex;align-items:center;gap:10px;margin:0 13px 8px;background:var(--bg-void);border:1px solid var(--border-soft);border-radius:10px;padding:10px 12px;}
.gi-root .mem .tx .t1{font-size:12.5px;font-weight:700;color:var(--fg-primary);display:flex;align-items:center;gap:7px;}
.gi-root .mem .tx .t1 .on{width:6px;height:6px;border-radius:50%;background:var(--success);}
.gi-root .mem .tx .t1 .off{width:6px;height:6px;border-radius:50%;background:var(--fg-faint);}
.gi-root .mem .tx .t2{font-family:var(--font-mono);font-size:9px;color:var(--fg-muted);margin-top:3px;line-height:1.6;}
.gi-root .mem .sp{flex:1;}
.gi-root .mem .pend{font-family:var(--font-mono);font-size:9px;color:var(--gold-bright);text-align:right;line-height:1.5;}
.gi-root .handcard{margin:0 13px 8px;background:var(--bg-void);border:1px dashed var(--border);border-radius:10px;padding:10px 13px;font-family:var(--font-mono);font-size:10px;color:var(--fg-secondary);line-height:1.8;}
.gi-root .handcard b{color:var(--fg-primary);font-weight:600;}
.gi-root .handcard .a{color:var(--accent);}
.gi-root .handcard .g{color:var(--gold-bright);}
.gi-root .rail .note{margin-top:auto;border-top:1px solid var(--border-soft);padding:12px 17px;font-family:var(--font-mono);font-size:9.5px;color:var(--fg-faint);line-height:1.8;}
.gi-root .rail .note .a{color:var(--accent);}
.gi-root .brief{max-width:780px;margin:18px auto;background:var(--bg-panel);border:1px solid var(--border);border-radius:13px;overflow:hidden;}
.gi-root .brief .bhd{padding:18px 24px 14px;border-bottom:1px solid var(--border-soft);background:var(--bg-void);}
.gi-root .brief .bhd .t{font-size:17px;font-weight:700;display:flex;align-items:baseline;gap:10px;}
.gi-root .brief .bhd .t .d{font-family:var(--font-mono);font-size:11px;color:var(--accent);font-weight:600;}
.gi-root .brief .bhd .m{font-family:var(--font-mono);font-size:9.5px;color:var(--fg-faint);margin-top:5px;}
.gi-root .brief .bhd .m .a{color:var(--accent);}
.gi-root .bsec{padding:14px 24px 4px;}
.gi-root .bsec .bt{font-family:var(--font-mono);font-size:10px;font-weight:700;letter-spacing:1.5px;text-transform:uppercase;color:var(--fg-muted);display:flex;align-items:center;gap:8px;margin-bottom:9px;}
.gi-root .bsec .bt .n{color:var(--gold-bright);}
.gi-root .bsec .bt::after{content:"";flex:1;height:1px;background:var(--border-soft);}
.gi-root .bitem{display:flex;gap:10px;padding:calc(var(--row-pad-y) + 1px) 0;font-size:12.5px;color:var(--fg-secondary);border-bottom:1px solid var(--border-soft);align-items:baseline;}
.gi-root .bitem:last-child{border-bottom:none;}
.gi-root .bitem .ic{font-family:var(--font-mono);font-size:10.5px;width:16px;flex:none;text-align:center;}
.gi-root .bitem .ic.ok{color:var(--success);}
.gi-root .bitem .ic.no{color:var(--error);}
.gi-root .bitem .ic.bk{color:var(--warning);}
.gi-root .bitem .ic.ct{color:var(--gold);}
.gi-root .bitem b{color:var(--fg-primary);font-weight:600;}
.gi-root .bitem .sp{flex:1;}
.gi-root .bitem .by{font-family:var(--font-mono);font-size:9.5px;color:var(--fg-faint);white-space:nowrap;}
.gi-root .bitem .by .a{color:var(--accent);}
.gi-root .bfoot{padding:13px 24px 16px;font-family:var(--font-mono);font-size:9.5px;color:var(--fg-faint);line-height:1.8;border-top:1px solid var(--border-soft);margin-top:10px;}
.gi-root .bfoot .a{color:var(--accent);}
.gi-root .ncard{max-width:780px;margin:18px auto 0;background:var(--bg-panel);border:1px solid var(--border);border-radius:13px;padding:18px 24px;}
.gi-root .ncard .t{font-size:15.5px;font-weight:700;display:flex;align-items:center;gap:10px;flex-wrap:wrap;}
.gi-root .ncard .m{font-family:var(--font-mono);font-size:9.5px;color:var(--fg-faint);margin-top:5px;}
.gi-root .ncard .body{font-size:13px;color:var(--fg-secondary);line-height:1.7;margin-top:10px;text-wrap:pretty;}
.gi-root .ncard .body b{color:var(--fg-primary);}
.gi-root .chan{max-width:780px;margin:14px auto 0;display:grid;grid-template-columns:1fr 1fr;gap:12px;}
.gi-root .chcard{background:var(--bg-panel);border:1px solid var(--border-soft);border-radius:12px;padding:14px 17px;}
.gi-root .chcard .ct{font-family:var(--font-mono);font-size:10px;font-weight:700;letter-spacing:1.5px;text-transform:uppercase;display:flex;align-items:center;gap:8px;margin-bottom:9px;}
.gi-root .chcard .ct.hum{color:var(--accent);}
.gi-root .chcard .ct.ag{color:var(--gold);}
.gi-root .chcard .ct::after{content:"";flex:1;height:1px;background:var(--border-soft);}
.gi-root .chcard ul{margin:0;padding-left:17px;font-size:12px;color:var(--fg-secondary);line-height:1.8;}
.gi-root .chcard li b{color:var(--fg-primary);font-weight:600;}
.gi-root .memo{max-width:780px;margin:14px auto 0;background:var(--bg-void);border:1px solid var(--border);border-radius:12px;padding:13px 17px;font-family:var(--font-mono);font-size:11px;line-height:1.9;color:var(--fg-secondary);}
.gi-root .memo .hd2{color:var(--fg-faint);font-size:9.5px;margin-bottom:6px;}
.gi-root .memo .hd2 .a{color:var(--accent);}
.gi-root .memo .k{color:var(--gold-bright);}
.gi-root .acktable{max-width:780px;margin:14px auto 24px;width:100%;}
.gi-root .acktable table{width:100%;border-collapse:collapse;font-size:12px;background:var(--bg-panel);border:1px solid var(--border);border-radius:12px;overflow:hidden;display:table;}
.gi-root .acktable th{font-family:var(--font-mono);font-size:9px;font-weight:700;letter-spacing:1px;text-transform:uppercase;color:var(--fg-muted);text-align:left;padding:8px 14px;border-bottom:1px solid var(--border);background:var(--bg-void);}
.gi-root .acktable td{padding:8px 14px;border-bottom:1px solid var(--border-soft);color:var(--fg-secondary);font-family:var(--font-mono);font-size:10.5px;}
.gi-root .acktable tr:last-child td{border-bottom:none;}
.gi-root .acktable td b{color:var(--fg-primary);font-weight:600;}
.gi-root .acktable .ok{color:var(--success);}
.gi-root .acktable .run{color:var(--progress);}
.gi-root .acktable .pd{color:var(--fg-faint);}
.gi-root .cwrap{padding-bottom:10px;}
.gi-root .toast{position:absolute;left:50%;bottom:24px;transform:translateX(-50%);background:var(--bg-elev);border:1px solid var(--accent-dim);color:var(--fg-primary);font-size:12.5px;border-radius:9px;padding:9px 16px;box-shadow:var(--shadow-toast);display:flex;gap:10px;align-items:center;white-space:nowrap;z-index:5;}
.gi-root .toast .mono{font-family:var(--font-mono);font-size:10px;color:var(--fg-muted);}`;
    const el = document.createElement('style');
    el.id = STYLE_ID;
    el.textContent = css;
    document.head.appendChild(el);
  }

  const { useState } = React;
  const t = window.t || ((en) => en);

  const inboxData = () => {
  const MEMBERS = [
    { id: 'zl', av: 'ZL', cls: '', nm: t('You · Zhou Le', '你 · 周乐'), role: t('Lead designer · owner: contracts/** · balance', '主策划 · owner: contracts/** · 数值'), on: true, pend: t('3 to review', '待批 3'), lanes: 'L1 · L4' },
    { id: 'cw', av: 'CW', cls: 'g2', nm: t('Chen Wei', '陈未'), role: 'gameplay owner · src/player/**', on: true, pend: t('2 to review', '待批 2'), lanes: 'L2 · L3' },
    { id: 'qh', av: 'QH', cls: 'g3', nm: t('Qin Hao', '秦昊'), role: t('Render / VFX · shaders/**', '渲染 / VFX · shaders/**'), on: false, pend: t('offline · 1 gate backlog', '离线 · 闸积压 1'), lanes: 'L6' },
  ];
  const MINE = [
    { ic: '§', icc: 'var(--gold)', t: t('Contract change: feel param ranges v1.0 → v1.1', '契约变更：手感参数区间 v1.0 → v1.1'), tag: ['gate', t('contract gate', '契约闸')], proj: 'boss-rush · L1 claude', why: t('route: contracts/** owner = you', '路由理由: contracts/** owner = 你'), age: '25m', hot: false },
    { ic: '⏸', icc: 'var(--gold)', t: t('Gallery gate: 20 level variants keep / kill', '画廊闸：关卡变体 20 个 keep / kill'), tag: ['gate', t('gallery gate', '画廊闸')], proj: t('boss-rush · L4 built-in', 'boss-rush · L4 内置'), why: t('route: levels/** reviewer = lead designer', '路由理由: levels/** 评审人 = 主策划'), age: '1h', hot: false },
    { ic: '?', icc: 'var(--warning)', t: t('L4 ask: content-gen budget almost exhausted', 'L4 问询：内容生成预算即将用尽'), tag: ['ask', t('lane ask', 'lane 问询')], proj: t('boss-rush · L4 built-in', 'boss-rush · L4 内置'), why: t('route: budget decision = lane owner (you)', '路由理由: budget 决策 = lane owner（你）'), age: '18m', hot: true },
  ];
  const UNCLAIMED = [
    { ic: '⏸', icc: 'var(--gold)', t: t('Replay-regression gate: hitstun 0.18s → 0.24s', '回放回归闸：受击硬直 0.18s → 0.24s'), tag: ['gate', t('replay-regression gate', '回放回归闸')], proj: 'boss-rush · L2 codex', why: t('src/enemy/** has no owner · anyone can claim', 'src/enemy/** 无 owner · 任何人可认领'), age: '42m' },
    { ic: '⏸', icc: 'var(--error)', t: t('Staged hardware gate: PID re-tune requesting field', '分级实机闸：PID 重整定申请 field'), tag: ['field', t('needs 2 approvers', '需 2 人批准')], proj: 'arm-ctrl · L7 claude', why: t('field strong gate · 1/2 approved (CW) · 1 more needed', 'field 强闸 · 已批 1/2（CW）· 还差 1 人'), age: '2h' },
  ];
  const THEIRS = [
    { who: 'CW', cls: 'g2', t: t('Replay-regression gate: Boss phase-2 bullet density', '回放回归闸：Boss 二阶段弹幕密度'), proj: 'boss-rush · L2', st: t('reviewing · Decision Center open 12m', '评审中 · 决策中心打开 12m'), age: '12m' },
    { who: 'CW', cls: 'g2', t: t('Integration gate: vd/dash-cancel → main', '集成闸：vd/dash-cancel → main'), proj: 'boss-rush · L2', st: t('queued', '队列中'), age: '33m' },
    { who: 'QH', cls: 'g3', t: t('Golden-frame gate: cavern volumetric fog', '金样画面闸：溶洞场景体积雾'), proj: 'boss-rush · L6', st: t('QH offline · backlogged to morning', 'QH 离线 · 积压至明早'), age: '3h' },
    { who: 'QH', cls: 'g3', t: t('Frame-budget gate: particle cap 2k → 3.5k', '帧预算闸：粒子上限 2k → 3.5k'), proj: 'boss-rush · L6', st: t('QH offline · can be claimed for them', 'QH 离线 · 可代为认领'), age: '5h' },
  ];
  return { MEMBERS, MINE, UNCLAIMED, THEIRS };
  };

  function Inbox({ claim, claimed }) {
    const { MEMBERS, MINE, UNCLAIMED, THEIRS } = inboxData();
    return (
      <div className="inner">
        <div className="maincol" data-screen-label="闸收件箱">
          <div className="sec">{t('Assigned to you', '指派给你')} <span className="n">3</span> <span className="hint">{t('· routed by viden.toml ownership', '· 按 viden.toml ownership 路由')}</span></div>
          {MINE.map((r, i) => (
            <div key={i} className="irow">
              <span className="ki" style={{ color: r.icc }}>{r.ic}</span>
              <div className="tx">
                <div className="t1">{r.t} <span className={"chip " + r.tag[0]}>{r.tag[1]}</span></div>
                <div className="t2"><span className="a">{r.proj}</span> · <span className="why">{r.why}</span></div>
              </div>
              <span className="sp"></span>
              <span className={"age " + (r.hot ? 'hot' : '')}>{r.age}</span>
              <button className="ibtn primary">{t('Open in Decision Center ↗', '在决策中心打开 ↗')}</button>
            </div>
          ))}
          <div className="sec">{t('Unclaimed', '未认领')} <span className="n">2</span> <span className="hint">{t('· no owner or needs multiple approvers', '· 无 owner 或需多人批准')}</span></div>
          {UNCLAIMED.map((r, i) => (
            <div key={i} className="irow">
              <span className="ki" style={{ color: r.icc }}>{r.ic}</span>
              <div className="tx">
                <div className="t1">{r.t} <span className={"chip " + (r.tag[0] === 'field' ? 'field' : 'gate')}>{r.tag[1]}</span></div>
                <div className="t2"><span className="a">{r.proj}</span> · <span className="why">{r.why}</span></div>
              </div>
              <span className="sp"></span>
              <span className="age">{r.age}</span>
              {claimed[i]
                ? <button className="ibtn primary">{t('Open in Decision Center ↗', '在决策中心打开 ↗')}</button>
                : <button className="ibtn" onClick={() => claim(i)}>{t('Claim', '认领')}</button>}
            </div>
          ))}
          <div className="sec">{t('Teammates working', '队友在办')} <span className="n">4</span></div>
          {THEIRS.map((r, i) => (
            <div key={i} className="irow dim">
              <span className={"av " + r.cls} style={{ width: 22, height: 22, fontSize: 8.5 }}>{r.who}</span>
              <div className="tx">
                <div className="t1">{r.t}</div>
                <div className="t2"><span className="a">{r.proj}</span> · <span className="why">{r.st}</span></div>
              </div>
              <span className="sp"></span>
              <span className="age">{r.age}</span>
            </div>
          ))}
          <div style={{ height: 16 }}></div>
        </div>
        <div className="rail" data-screen-label="团队状态">
          <div className="hd">{t('Team', '团队')}</div>
          {MEMBERS.map(m => (
            <div key={m.id} className="mem">
              <span className={"av " + m.cls}>{m.av}</span>
              <div className="tx">
                <div className="t1">{m.nm} <span className={m.on ? 'on' : 'off'}></span></div>
                <div className="t2">{m.role}<br />lanes: {m.lanes}</div>
              </div>
              <span className="sp"></span>
              <span className="pend">{m.pend}</span>
            </div>
          ))}
          <div className="hd">{t('Handoff & autopilot', '移交与自动驾驶')}</div>
          <div className="handcard"><b>L7 · pid-retune</b> {t('handing off', '移交中')}<br /><span className="a">CW</span> → <span className="a">{t('you', '你')}</span>{t(' · with session context & device lease', ' · 含会话上下文与设备租约')}<br /><span className="g">{t('awaiting your accept', '待你接受')}</span>{t(' · reject returns it', ' · 拒绝则退回')}</div>
          <div className="handcard"><b>{t('Night autopilot', '夜间自动驾驶')}</b> 22:00 – 08:00<br />{t('lanes keep running · gate backlog not pushed', 'lane 继续跑 · 闸积压不推送')}<br />{t('morning inbox sorted by ', '晨间收件箱按 ')}<span className="a">{t('priority + age', '优先级 + 等龄')}</span></div>
          <div className="note">{t('The inbox aggregates all projects; the approval action itself happens in the ', '收件箱聚合全部项目；批闸动作本身发生在')}<span className="a">{t('Decision Center', '决策中心')}</span>{t('. Every claim / handoff enters the team timeline (audit trail).', '。每次认领 / 移交都进团队时间线（审计流）。')}</div>
        </div>
      </div>
    );
  }

  function Briefing() {
    return (
      <div className="inner full">
        <div className="maincol cwrap" data-screen-label="团队简报">
          <div className="brief">
            <div className="bhd">
              <div className="t">{t('Team briefing', '团队简报')} <span className="d">2026-06-13 · {t('Fri', '周五')}</span></div>
              <div className="m">{t('Generated by the scribe lane ', '由书记官 lane ')}<span className="a">L0 scribe</span>{t(' at 09:00 · scope: since 09:00 yesterday · organizes only, never commands · distributed by subscription graph', ' 生成于 09:00 · 范围: 昨日 09:00 起 · 只整理，不指挥 · 按订阅图分发')}</div>
            </div>
            <div className="bsec">
              <div className="bt">{t('Contract changes', '契约变更')} <span className="n">1</span></div>
              <div className="bitem"><span className="ic ct">§</span><span><b>design-spec.md</b> v1.0 → v1.1 · {t('§4 feel param ranges aligned to jump-feel measurements', '§4 手感参数区间对齐 jump-feel 实测')}</span><span className="sp"></span><span className="by">{t('you approved · 16:40 · subscriber ack ', '你批准 · 16:40 · 订阅方 ack ')}<span className="a">2/2 ✓</span></span></div>
            </div>
            <div className="bsec">
              <div className="bt">{t('Gate decisions', '闸决策')} <span className="n">8</span> <span className="hint" style={{ fontWeight: 400, letterSpacing: 0, textTransform: 'none', color: 'var(--fg-faint)' }}>{t('6 approved · 1 rejected · 1 returned', '批 6 · 驳 1 · 退回 1')}</span></div>
              <div className="bitem"><span className="ic ok">✓</span><span><b>{t('replay-regression gate', '回放回归闸')}</b> · {t('jump feel: lower peak, longer airtime (L2)', '跳跃手感：降低峰值、加长滞空（L2）')}</span><span className="sp"></span><span className="by">{t('you approved · 16:32', '你批准 · 16:32')}</span></div>
              <div className="bitem"><span className="ic ok">✓</span><span><b>{t('eval gate', 'eval 闸')}</b> · {t('warmup schedule + grad clipping, 4/4 benchmarks hold (spatial-lm · L5)', 'warmup 调度 + 梯度裁剪，4/4 基准不回退（spatial-lm · L5）')}</span><span className="sp"></span><span className="by">{t('you approved · 15:18', '你批准 · 15:18')}</span></div>
              <div className="bitem"><span className="ic bk">↩</span><span><b>{t('golden-frame gate', '金样画面闸')}</b> · {t('cavern volumetric fog: shadow banding over tolerance, returned to L6 to adjust dither', '溶洞体积雾：暗部色带超容差，退回 L6 调 dither')}</span><span className="sp"></span><span className="by">{t('QH returned · 14:02', 'QH 退回 · 14:02')}</span></div>
              <div className="bitem"><span className="ic no">✕</span><span><b>{t('contract gate', '契约闸')}</b> · {t('L3 proposed raising retry cap to 8: conflicts with stability goal', 'L3 提议放宽 retry 上限至 8 次：与稳定性目标冲突')}</span><span className="sp"></span><span className="by">{t('CW rejected · 11:47', 'CW 驳回 · 11:47')}</span></div>
            </div>
            <div className="bsec">
              <div className="bt">{t('Merged to main', '合入 main')} <span className="n">2</span></div>
              <div className="bitem"><span className="ic ok">⇡</span><span><b>vd/jump-feel</b> → main · {t('integration gate passed, replay baseline updated', '集成闸通过，回放基线已更新')}</span><span className="sp"></span><span className="by">{t('you merged · 17:05', '你合并 · 17:05')}</span></div>
              <div className="bitem"><span className="ic ok">⇡</span><span><b>vd/retry-policy</b> → main · {t('option B (openai first) · 2 follow-ups created', '方案 B（仅 openai 先行）· follow-up ×2 已建')}</span><span className="sp"></span><span className="by">{t('CW merged · 18:21', 'CW 合并 · 18:21')}</span></div>
            </div>
            <div className="bsec">
              <div className="bt">{t('Risks & backlog', '风险与积压')} <span className="n">3</span></div>
              <div className="bitem"><span className="ic bk">⚠</span><span><b>{t('arm-ctrl field gate', 'arm-ctrl field 闸')}</b> {t('needs 1 more approver (1/2) · device lease expires 14:00', '还差 1 人批准（已 1/2）· 设备租约 14:00 过期')}</span><span className="sp"></span><span className="by">{t('unclaimed · 2h', '未认领 · 2h')}</span></div>
              <div className="bitem"><span className="ic bk">⚠</span><span><b>{t('QH offline', 'QH 离线')}</b> · {t('render track has 2 gates backlogged, frame-budget gate can be claimed for them', '渲染向 2 个闸积压，帧预算闸可代为认领')}</span><span className="sp"></span><span className="by">3h – 5h</span></div>
              <div className="bitem"><span className="ic bk">⚠</span><span><b>{t('L4 token budget', 'L4 token 预算')}</b> {t('82% · ask awaiting your reply in queue', '82% · 问询在你队列待回复')}</span><span className="sp"></span><span className="by">18m</span></div>
            </div>
            <div className="bfoot">{t('You received this briefing because: ', '你收到本简报因为: ')}<span className="a">owner: contracts/**</span> + <span className="a">watch: src/player/**</span>{t(' · same day scribe updated CHANGELOG.md and AGENTS.md §3 · full audit trail in the team timeline', ' · 同日 scribe 已更新 CHANGELOG.md 与 AGENTS.md §3 · 完整审计流见团队时间线')}</div>
          </div>
        </div>
      </div>
    );
  }

  function Notice() {
    return (
      <div className="inner full">
        <div className="maincol cwrap" data-screen-label="变更通告">
          <div className="ncard">
            <div className="t">{t('Contract-change notice · design-spec.md v1.0 → v1.1', '契约变更通告 · design-spec.md v1.0 → v1.1')} <span className="chip gate">{t('§ contract gate approved', '§ 契约闸已批')}</span><span className="chip">{t('non-breaking · into briefing', '非破坏性 · 进简报')}</span></div>
            <div className="m">{t('2026-06-12 16:40 · proposed by L1 claude · approver: you · routing: subscription graph (dependencies + ownership + watch declarations) · not broadcast to all', '2026-06-12 16:40 · 提议 L1 claude · 批准人: 你 · 路由: 订阅图（依赖关系 + ownership + watch 声明）· 不全员广播')}</div>
            <div className="body">{t('§4 feel param ranges tightened with a new variable jump height: jump peak ', '§4 手感参数区间收紧并新增可变跳高：跳跃峰值 ')}<b>110±10 → 100±8 px</b>{t(', airtime ', '，滞空 ')}<b>0.60–0.64 → 0.64–0.70s</b>{t(', added release cut ×0.40–0.50. The change is “implementation written back to contract” — making the contract reflect the approved actual feel. ', '，新增松键截断 ×0.40–0.50。变更性质为「实现回写契约」—— 使契约反映已批准的实际手感。')}<b>{t('Breaking changes are pushed instantly', '破坏性变更会即时推送')}</b>{t('; this one is non-breaking and folded into the daily briefing.', '；本条为非破坏性，归入当日简报。')}</div>
          </div>
          <div className="chan">
            <div className="chcard">
              <div className="ct hum">{t('Channel A · to people', '通道 A · 送到人')}</div>
              <ul>
                <li><b>{t('You (owner: contracts)', '你（owner: contracts）')}</b>{t(': informed at approval time', '：已在批准时知情')}</li>
                <li><b>{t('CW (watch: src/player)', 'CW（watch: src/player）')}</b>{t(': inbox + daily briefing', '：收件箱 + 当日简报')}</li>
                <li><b>QH</b>{t(': not in the subscription graph → not disturbed', '：不在订阅图内 → 不打扰')}</li>
                <li>{t('From V2, forwardable to IM (one-tap approve / reject, no real work in IM)', 'V2 起可转发 IM（一键批 / 驳，不在 IM 干活）')}</li>
              </ul>
            </div>
            <div className="chcard">
              <div className="ct ag">{t('Channel B · to agents (memo)', '通道 B · 送到 agent（memo）')}</div>
              <ul>
                <li>{t('Written as a ', '写成 ')}<b>{t('context patch', '上下文补丁')}</b>{t(' injected into subscriber lanes\u2019 memos/', ' 注入订阅 lane 的 memos/')}</li>
                <li>{t('Required reading at session start · agent ', 'session 启动必读 · agent ')}<b>{t('must ack', '必须 ack')}</b></li>
                <li>{t('Agent decides the action: rebase / update callers / raise a review request', 'agent 自行决定动作：rebase / 更新调用方 / 发起评审请求')}</li>
                <li>{t('ack status flows back to the Decision Center and briefing', 'ack 状态回流决策中心与简报')}</li>
              </ul>
            </div>
          </div>
          <div className="memo">
            <div className="hd2">{t('memos/2026-06-12-design-spec-v1.1.md · generated by scribe · injected into subscriber lanes', 'memos/2026-06-12-design-spec-v1.1.md · 由 scribe 生成 · 注入订阅 lane')}</div>
            <span className="k">to:</span> L2, L4 &nbsp;·&nbsp; <span className="k">ack:</span> required &nbsp;·&nbsp; <span className="k">source:</span> contracts/design-spec.md@v1.1<br />
            <span className="k">summary:</span> {t('§4 feel ranges tightened (peak 100±8px · airtime 0.64–0.70s) + added variable jump height.', '§4 手感区间收紧（峰值 100±8px · 滞空 0.64–0.70s）+ 新增可变跳高。')}<br />
            <span className="k">suggested-action:</span> {t('Verify this lane\u2019s output falls within the new ranges; open a follow-up session for any that exceed.', '校验本 lane 产物是否落在新区间；超出者开 follow-up session。')}
          </div>
          <div className="acktable" data-screen-label="memo ack 追踪">
            <table>
              <thead><tr><th>{t('Subscriber lane', '订阅 lane')}</th><th>{t('Injected', '注入')}</th><th>ack</th><th>{t('agent action', 'agent 动作')}</th><th>{t('Status', '状态')}</th></tr></thead>
              <tbody>
                <tr><td><b>L2</b> · codex · gameplay</td><td>16:41</td><td className="ok">✓ 16:58</td><td>{t('no change needed — jump-feel is the source, params already aligned', '无需变更 —— jump-feel 即本次来源，参数已一致')}</td><td className="ok">{t('Done', '完成')}</td></tr>
                <tr><td><b>L4</b> · {t('built-in · content gen', '内置 · 内容生成')}</td><td>16:41</td><td className="ok">✓ 17:12</td><td>{t('re-check reachability of 20 keep variants against new ranges', '对 20 个 keep 变体按新区间复检可达性')}</td><td className="run">{t('▶ in progress · 14/20', '▶ 进行中 · 14/20')}</td></tr>
                <tr><td><b>L6</b> · kiro · {t('render', '渲染')}</td><td>{t('at next session start', '下次 session 启动时')}</td><td className="pd">—</td><td>{t('expected: none (no feel-param dependency, only watches asset spec)', '预计：无（不依赖手感参数，仅 watch 资产规范）')}</td><td className="pd">{t('pending inject', '待注入')}</td></tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>
    );
  }

  function InboxView(props) {
    const tabs = ['inbox', 'briefing', 'notice'];
    const init = tabs.indexOf(props.defaultTab) >= 0 ? tabs.indexOf(props.defaultTab) : 0;
    const [tab, setTab] = useState(init);
    const [claimed, setClaimed] = useState({});
    const [toast, setToast] = useState(null);
    const claim = (i) => {
      setClaimed(p => ({ ...p, [i]: true }));
      const msg = i === 1
        ? t('Claimed · field gate 2/2 complete, into your queue', '已认领 · field 闸 2/2 凑齐，进入你的队列')
        : t('Claimed, into your queue', '已认领，进入你的队列');
      if (props.onToast) props.onToast(msg);
      setToast(msg);
      setTimeout(() => setToast(null), 2800);
    };
    return (
      <div className="gi-root" data-screen-label="团队协作">
        <div className="cmdbar">
          <span className="t"><span className="pulse"></span>{t('Team', '团队')}</span>
          <div className="vtabs">
            <span className={"vtab " + (tab === 0 ? 'on' : '')} onClick={() => setTab(0)}>⏸ {t('Inbox', '收件箱')} <span className="n">5</span></span>
            <span className={"vtab " + (tab === 1 ? 'on' : '')} onClick={() => setTab(1)}>¶ {t('Team briefing', '团队简报')}</span>
            <span className={"vtab " + (tab === 2 ? 'on' : '')} onClick={() => setTab(2)}>✉ {t('Change notice', '变更通告')} <span className="n">1</span></span>
          </div>
          <span className="sp"></span>
          <span className="meta">{t('3 people × 9 lanes × 3 projects · the timeline is the audit trail', '3 人 × 9 lanes × 3 项目 · 时间线即审计流')}</span>
          <span className="me"><span className="av">ZL</span>{t('lead designer', '主策划')}</span>
        </div>
        {tab === 0 && <Inbox claim={claim} claimed={claimed} />}
        {tab === 1 && <Briefing />}
        {tab === 2 && <Notice />}
        {toast && <div className="toast">✓ {toast}<span className="mono">timeline: claim</span></div>}
      </div>
    );
  }

  window.InboxView = InboxView;
})();
