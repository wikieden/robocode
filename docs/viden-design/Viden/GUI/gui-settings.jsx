/* ════════════════════════════════════════════════════════════════
 * gui-settings.jsx · Viden 设置/偏好（canonical · 单一真源）
 *
 * D1 驾驶舱第 8 视图（活动 rail 底部 ⚙ · view==='settings'）。
 * 不新开窗口 —— 沿用 view-state 路由，在 centerwrap 内渲染。
 * 结构：左分类栏(218px=--rail-left) + 右表单列(滚动 · max 760px)。
 *
 * ⚠ 文案纪律（SPEC D-COPY）：本组件是**产品 UI** —— 界面文案 = 产品
 *   微文案，一律走 t(en,zh) 随全局语言切换；设计侧备注（看板编号/
 *   映射表/引擎对应关系）只写在注释里，绝不落到界面上。
 *
 * 引擎对齐（robocode-config / robocode-types · 见 docs/NAMING-MAP.md）：
 *   · 配置链 = <project>/.viden/config.toml → ~/.config/viden/config.toml
 *   · permission_mode 枚举 = default|acceptEdits|bypassPermissions|dontAsk|plan
 *     （UI 标签映射 = NAMING-MAP §2）
 *   · provider registry = deepseek(默认)·anthropic·openai·openrouter·ollama…
 *   · Keyboard / Telemetry / Notifications = GUI 层实现，引擎无对应键
 *   · Notifications 节 = 审查看板 #13 离桌兜底（email/webhook escalation）
 *   · Language 项 = window.vSetLang(i18n.js v2)：live 页零刷新热切,其余页 reload
 *
 * 用法（页面已加载 React + Babel + tokens.css + i18n.js + gui-icons.jsx）：
 *   <script type="text/babel" src="gui-settings.jsx"></script>  // 主脚本之前
 *   <SettingsView/>
 * 样式自注入（.gset-root 作用域隔离，不污染宿主页）。
 * ════════════════════════════════════════════════════════════════ */
(function () {
  const STYLE_ID = 'gui-settings-style';
  if (!document.getElementById(STYLE_ID)) {
    const css = `
.gset-root{flex:1;min-height:0;display:flex;flex-direction:column;background:var(--bg-base);font-family:var(--font-sans);color:var(--fg-primary);}
.gset-root .cmdbar2{flex:none;display:flex;align-items:center;gap:14px;padding:0 18px;border-bottom:1px solid var(--border);background:var(--bg-void);height:50px;}
.gset-root .cmdbar2 .t{font-size:15px;font-weight:700;}
.gset-root .cmdbar2 .src{display:flex;align-items:center;gap:7px;font-family:var(--font-mono);font-size:10.5px;color:var(--fg-secondary);background:var(--bg-panel);border:1px solid var(--border-soft);border-radius:7px;padding:4px 10px;}
.gset-root .cmdbar2 .src .p{color:var(--accent);}
.gset-root .cmdbar2 .sp{flex:1;}
.gset-root .cmdbar2 .srch{display:flex;align-items:center;gap:7px;font-family:var(--font-mono);font-size:11px;color:var(--fg-faint);background:var(--bg-panel);border:1px solid var(--border-soft);border-radius:7px;padding:5px 11px;min-width:210px;}
.gset-root .gset-body{flex:1;display:grid;grid-template-columns:var(--rail-left) minmax(0,1fr);min-height:0;}
.gset-root .gset-nav{background:var(--bg-panel);border-right:1px solid var(--border-soft);padding:10px 8px;overflow:hidden auto;scrollbar-width:thin;scrollbar-color:var(--border) transparent;}
.gset-root .gset-navhd{font-family:var(--font-mono);font-size:10px;font-weight:700;letter-spacing:1.3px;text-transform:uppercase;color:var(--fg-muted);padding:6px 10px 5px;}
.gset-root .navrow{display:flex;align-items:center;gap:9px;padding:7px 10px;border-radius:7px;cursor:pointer;color:var(--fg-secondary);font-size:12.5px;font-weight:500;border:1px solid transparent;min-height:30px;}
.gset-root .navrow:hover{background:var(--bg-base);color:var(--fg-primary);}
.gset-root .navrow.on{background:var(--bg-sel);color:var(--accent-bright);box-shadow:inset 3px 0 0 var(--accent);font-weight:600;}
.gset-root .gset-main{overflow:hidden auto;scrollbar-width:thin;scrollbar-color:var(--border) transparent;padding:20px 26px 46px;}
.gset-root .gset-col{max-width:760px;}
.gset-root .gset-h{display:flex;align-items:center;gap:10px;margin:0 0 4px;}
.gset-root .gset-h h2{margin:0;font-size:17px;font-weight:700;}
.gset-root .gset-sub{font-size:12px;color:var(--fg-muted);line-height:1.6;margin:0 0 16px;}
.gset-root .gset-sub .cfg{font-family:var(--font-mono);font-size:10.5px;color:var(--accent);}
.gset-root .card{background:var(--bg-panel);border:1px solid var(--border-soft);border-radius:10px;margin-bottom:14px;overflow:hidden;}
.gset-root .card .chd{display:flex;align-items:center;gap:9px;padding:11px 15px 9px;font-family:var(--font-mono);font-size:10px;font-weight:700;letter-spacing:1.3px;text-transform:uppercase;color:var(--fg-muted);}
.gset-root .card .chd .k{font-weight:400;letter-spacing:0;text-transform:none;color:var(--fg-faint);}
.gset-root .card .chd .sp{flex:1;}
.gset-root .setrow{display:flex;align-items:center;gap:14px;padding:calc(var(--card-pad-y) + 1px) 15px;border-top:1px solid var(--border-soft);min-height:44px;}
.gset-root .setrow .lb{min-width:0;flex:1;}
.gset-root .setrow .lb .l1{font-size:12.5px;font-weight:600;color:var(--fg-primary);display:flex;align-items:center;gap:8px;flex-wrap:wrap;}
.gset-root .setrow .lb .l2{font-size:10.5px;color:var(--fg-muted);margin-top:2px;line-height:1.55;}
.gset-root .setrow .lb .l2 .cfg{font-family:var(--font-mono);font-size:9.5px;color:var(--fg-faint);}
.gset-root .tag{font-family:var(--font-mono);font-size:8.5px;font-weight:700;border:1px solid;border-radius:4px;padding:1px 5px;letter-spacing:.4px;}
.gset-root .tag.soon{color:var(--fg-muted);border-color:var(--border);border-style:dashed;}
.gset-root .tag.def{color:var(--accent);border-color:var(--accent-dim);}
.gset-root .tag.away{color:var(--gold-bright);border-color:color-mix(in srgb,var(--gold) 45%,transparent);}
/* controls */
.gset-root .tgl{flex:none;width:34px;height:19px;border-radius:10px;background:var(--bg-void);border:1px solid var(--border);position:relative;cursor:pointer;transition:all var(--motion-fast) var(--ease);}
.gset-root .tgl::after{content:"";position:absolute;top:2px;left:2px;width:13px;height:13px;border-radius:50%;background:var(--fg-muted);transition:all var(--motion-fast) var(--ease);}
.gset-root .tgl.on{background:var(--bg-sel);border-color:var(--accent-dim);}
.gset-root .tgl.on::after{left:17px;background:var(--accent-bright);}
.gset-root .seg{flex:none;display:flex;gap:3px;padding:3px;background:var(--bg-void);border:1px solid var(--border-soft);border-radius:8px;}
.gset-root .seg .s{font-family:var(--font-mono);font-size:10.5px;font-weight:600;color:var(--fg-muted);padding:4px 10px;border-radius:6px;cursor:pointer;white-space:nowrap;}
.gset-root .seg .s:hover{color:var(--fg-secondary);}
.gset-root .seg .s.on{color:var(--accent-bright);background:var(--bg-sel);}
.gset-root .selchip{flex:none;display:flex;align-items:center;gap:7px;font-family:var(--font-mono);font-size:11px;color:var(--fg-secondary);background:var(--bg-void);border:1px solid var(--border-soft);border-radius:7px;padding:5px 11px;cursor:pointer;white-space:nowrap;}
.gset-root .selchip:hover{border-color:var(--accent-dim);color:var(--fg-primary);}
.gset-root .selchip .v{color:var(--fg-primary);font-weight:600;}
.gset-root .selchip .car{color:var(--fg-faint);font-size:9px;}
.gset-root .txtin{flex:none;font-family:var(--font-mono);font-size:11px;color:var(--fg-primary);background:var(--bg-void);border:1px solid var(--border-soft);border-radius:7px;padding:5px 11px;min-width:190px;}
.gset-root .txtin .ph{color:var(--fg-faint);}
.gset-root .kbd{font-family:var(--font-mono);font-size:10px;color:var(--fg-secondary);background:var(--bg-void);border:1px solid var(--border);border-bottom-width:2px;border-radius:5px;padding:2px 7px;}
/* provider rows */
.gset-root .provrow{display:flex;align-items:center;gap:11px;padding:9px 15px;border-top:1px solid var(--border-soft);cursor:pointer;min-height:44px;}
.gset-root .provrow:hover{background:var(--bg-base);}
.gset-root .provrow.on{background:var(--bg-sel);box-shadow:inset 3px 0 0 var(--accent);}
.gset-root .provrow .pnm{font-size:12.5px;font-weight:600;min-width:118px;display:flex;align-items:center;gap:8px;}
.gset-root .provrow.on .pnm{color:var(--accent-bright);}
.gset-root .provrow .pmeta{font-family:var(--font-mono);font-size:10px;color:var(--fg-muted);min-width:0;flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}
.gset-root .provrow .pmeta .m{color:var(--fg-secondary);}
.gset-root .keychip{font-family:var(--font-mono);font-size:9.5px;border:1px solid;border-radius:5px;padding:2px 7px;white-space:nowrap;}
.gset-root .keychip.ok{color:var(--success);border-color:color-mix(in srgb,var(--success) 40%,transparent);}
.gset-root .keychip.miss{color:var(--warning);border-color:color-mix(in srgb,var(--warning) 45%,transparent);}
.gset-root .keychip.free{color:var(--fg-muted);border-color:var(--border);}
/* permission mode radio rows */
.gset-root .pmrow{display:flex;align-items:center;gap:11px;padding:9px 15px;border-top:1px solid var(--border-soft);cursor:pointer;min-height:44px;}
.gset-root .pmrow:hover{background:var(--bg-base);}
.gset-root .pmrow.on{background:var(--bg-sel);box-shadow:inset 3px 0 0 var(--accent);}
.gset-root .pmrow .rad{flex:none;width:13px;height:13px;border-radius:50%;border:1.5px solid var(--border);}
.gset-root .pmrow.on .rad{border-color:var(--accent-bright);box-shadow:inset 0 0 0 3px var(--bg-sel), inset 0 0 0 8px var(--accent-bright);}
.gset-root .pmrow .nm{font-size:12.5px;font-weight:600;min-width:96px;}
.gset-root .pmrow.on .nm{color:var(--accent-bright);}
.gset-root .pmrow .cli{font-family:var(--font-mono);font-size:9.5px;color:var(--fg-faint);min-width:132px;}
.gset-root .pmrow .ds{font-size:10.5px;color:var(--fg-muted);flex:1;min-width:0;line-height:1.5;}
.gset-root .pmrow .risk{font-family:var(--font-mono);font-size:8.5px;font-weight:700;border:1px solid;border-radius:4px;padding:1px 5px;}
.gset-root .risk.safe{color:var(--success);border-color:color-mix(in srgb,var(--success) 40%,transparent);}
.gset-root .risk.gated{color:var(--gold-bright);border-color:color-mix(in srgb,var(--gold) 45%,transparent);}
.gset-root .risk.hot{color:var(--error);border-color:color-mix(in srgb,var(--error) 45%,transparent);}
/* rules preview */
.gset-root .rulebox{margin:12px 15px;background:var(--bg-void);border:1px solid var(--border-soft);border-radius:8px;padding:10px 13px;font-family:var(--font-mono);font-size:10.5px;line-height:2;color:var(--fg-secondary);}
.gset-root .rulebox .al{color:var(--success);}
.gset-root .rulebox .dn{color:var(--error);}
.gset-root .rulebox .ak{color:var(--gold-bright);}
.gset-root .rulebox .cm{color:var(--fg-faint);}
/* skin dots */
.gset-root .skinrow{display:flex;gap:8px;flex-wrap:wrap;}
.gset-root .skdot{display:flex;align-items:center;gap:7px;font-family:var(--font-mono);font-size:10.5px;font-weight:600;color:var(--fg-muted);background:var(--bg-void);border:1px solid var(--border-soft);border-radius:8px;padding:6px 11px;cursor:pointer;min-height:30px;}
.gset-root .skdot:hover{color:var(--fg-secondary);border-color:var(--border);}
.gset-root .skdot.on{color:var(--accent-bright);border-color:var(--accent-dim);background:var(--bg-sel);}
.gset-root .skdot .d{width:9px;height:9px;border-radius:50%;flex:none;}
`;
    const el = document.createElement('style');
    el.id = STYLE_ID; el.textContent = css;
    document.head.appendChild(el);
  }

  const { useState } = React;
  const t = window.t || ((en) => en);

  /* —— 引擎真源数据（robocode registry / enums · NAMING-MAP §2/§3）—— */
  const PROVIDERS = [
    { id: 'deepseek',   nm: 'DeepSeek',   model: 'deepseek-v4-flash', keyEnv: 'DEEPSEEK_API_KEY',   key: 'ok',   def: true  },
    { id: 'anthropic',  nm: 'Anthropic',  model: 'claude-sonnet-4-5', keyEnv: 'ANTHROPIC_API_KEY',  key: 'ok'   },
    { id: 'openai',     nm: 'OpenAI',     model: 'gpt-5.2',           keyEnv: 'OPENAI_API_KEY',     key: 'miss' },
    { id: 'openrouter', nm: 'OpenRouter', model: 'auto',              keyEnv: 'OPENROUTER_API_KEY', key: 'miss' },
    { id: 'ollama',     nm: 'Ollama',     model: 'qwen3-coder:32b',   keyEnv: null,                 key: 'free' },
  ];

  function Toggle({ on, set }) { return <div className={'tgl ' + (on ? 'on' : '')} onClick={() => set(!on)}></div>; }
  function Seg({ opts, val, set }) {
    return <div className="seg">{opts.map(o => {
      const [key, label] = Array.isArray(o) ? o : [o, o];
      return <span key={key} className={'s ' + (key === val ? 'on' : '')} onClick={() => set(key)}>{label}</span>;
    })}</div>;
  }

  /* ── 各分区 ─────────────────────────────────────────── */
  function SecProvider() {
    const [cur, setCur] = useState('deepseek');
    return (
      <div>
        <div className="gset-h"><h2>{t('Provider & Models', 'Provider 与模型')}</h2></div>
        <p className="gset-sub">{t('API keys are referenced by environment variable and never stored in plain text. ', 'API key 只引用环境变量名，绝不明文落盘。')}<span className="cfg">VIDEN_*</span>{t(' variables and CLI flags override everything below.', ' 环境变量与 CLI 参数可覆盖以下全部设置。')}</p>
        <div className="card">
          <div className="chd">{t('Providers', 'Provider')}<span className="k">{t('click a row to set default', '点行设为默认')}</span><span className="sp"></span><span className="selchip">{t('+ Add provider…', '+ 添加 provider…')}</span></div>
          {PROVIDERS.map(p => (
            <div key={p.id} className={'provrow ' + (cur === p.id ? 'on' : '')} onClick={() => setCur(p.id)}>
              <span className="pnm">{p.nm}{p.def && <span className="tag def">{t('DEFAULT', '默认')}</span>}</span>
              <span className="pmeta">{t('model ', '模型 ')}<span className="m">{p.model}</span>{p.keyEnv && <span> · key ← <span className="m">{p.keyEnv}</span></span>}</span>
              {p.key === 'ok' && <span className="keychip ok">key ✓</span>}
              {p.key === 'miss' && <span className="keychip miss">{t('key missing', '缺 key')}</span>}
              {p.key === 'free' && <span className="keychip free">{t('no key', '免 key')}</span>}
            </div>
          ))}
        </div>
        <div className="card">
          <div className="chd">{t('Requests', '请求')}</div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Request timeout', '请求超时')}</div><div className="l2"><span className="cfg">request_timeout_secs</span></div></div><span className="selchip"><span className="v">90s</span><span className="car">▾</span></span></div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Max retries', '最大重试次数')}</div><div className="l2"><span className="cfg">max_retries</span></div></div><Seg opts={['0', '1', '2', '3']} val={'1'} set={() => {}}/></div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Provider plugin directories', 'Provider 插件目录')}</div><div className="l2">{t('Descriptors found here register as custom providers · ', '目录内的描述文件注册为自定义 provider · ')}<span className="cfg">provider_plugin_dirs</span></div></div><span className="txtin"><span className="ph">~/.viden/providers/…</span></span></div>
        </div>
      </div>
    );
  }

  function SecPermission() {
    const [mode, setMode] = useState('default');
    /* cli 名 = 引擎枚举；UI 标签映射 = NAMING-MAP §2 */
    const PMODES = [
      { id: 'default',           nm: 'Ask',        cli: 'default',           risk: 'safe',  ds: t('Risky actions pause for approval; safe reads run freely', '危险动作停下等批准；安全读取直接放行') },
      { id: 'acceptEdits',       nm: 'Auto Edit',  cli: 'acceptEdits',       risk: 'safe',  ds: t('File edits auto-approved; shell & out-of-scope still ask', '文件编辑自动放行；shell 与越界仍需批准') },
      { id: 'plan',              nm: 'Read Only',  cli: 'plan',              risk: 'gated', ds: t('Explore only — every mutating tool is blocked', '只读探索 —— 一切写入动作被拦下') },
      { id: 'dontAsk',           nm: 'Full Access',cli: 'dontAsk',           risk: 'hot',   ds: t('Run everything without prompts — sandboxes only. Same UI label as bypass (engine folds both to FullAccess); the cli_name tells them apart', '不问直接执行 —— 仅限沙箱。UI 标签与 bypass 同为 Full Access(引擎折叠),靠 cli_name 区分档位') },
      { id: 'bypassPermissions', nm: 'Full Access',cli: 'bypassPermissions', risk: 'hot',   ds: t('Skip the permission engine entirely; merges still gate', '完全跳过权限引擎；合入仍要过闸') },
    ];
    return (
      <div>
        <div className="gset-h"><h2>{t('Permissions', '权限')}</h2></div>
        <p className="gset-sub">{t('The default mode for new lanes — each lane can override it from the status bar. Permissions guard ', '新 lane 的默认档 —— 每条 lane 可在状态栏单独改。权限管的是')}<b>{t('execution', '执行')}</b>{t('; merges are always reviewed later at a gate.', '；合入永远在闸上另行评审。')}</p>
        <div className="card">
          <div className="chd">{t('Default permission mode', '默认权限模式')}<span className="k">permission_mode</span></div>
          {PMODES.map(m => (
            <div key={m.id} className={'pmrow ' + (mode === m.id ? 'on' : '')} onClick={() => setMode(m.id)}>
              <span className="rad"></span>
              <span className="nm">{m.nm}</span>
              <span className="cli">{m.cli}</span>
              <span className="ds">{m.ds}</span>
              <span className={'risk ' + m.risk}>{m.risk === 'safe' ? t('SAFE', '安全') : m.risk === 'gated' ? t('GATED', '受限') : t('RISK', '高危')}</span>
            </div>
          ))}
        </div>
        <div className="card">
          <div className="chd">{t('Rules', '规则')}<span className="k">{t('deny → ask → allow, then mode', 'deny → ask → allow，再落模式')}</span><span className="sp"></span><span className="selchip">{t('Edit rules…', '编辑规则…')}</span></div>
          <div className="rulebox">
            <div><span className="al">allow</span>  shell(cargo test *)          <span className="cm"># project</span></div>
            <div><span className="al">allow</span>  shell(cargo clippy *)         <span className="cm"># project</span></div>
            <div><span className="ak">ask</span>    shell(git push *)             <span className="cm"># global</span></div>
            <div><span className="dn">deny</span>   shell(rm -rf /*)              <span className="cm"># global</span></div>
          </div>
        </div>
        <div className="card">
          <div className="chd">{t('Working directory scope', '工作目录范围')}</div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Additional working directories', '额外工作目录')}</div><div className="l2">{t('Paths agents may touch outside their worktree — anything else is denied', 'agent 在 worktree 之外允许触达的路径 —— 其余一律拒绝')}</div></div><span className="txtin">../shared-fixtures</span></div>
        </div>
      </div>
    );
  }

  function SecAppearance() {
    const RC = window.RC || null;
    const schemes = (RC && RC.SCHEMES) || [['aurora', 'Aurora', '青', ['dark', 'light']], ['ice', 'Ice', '蓝', ['dark', 'light']], ['mono', 'Mono', '灰', ['dark', 'light']], ['amber', 'Amber', '琥珀', ['dark']], ['phosphor', 'Phosphor', '绿', ['dark']]];
    const doc = document.documentElement;
    const [skin, setSkinS] = useState(doc.getAttribute('data-skin') || 'aurora');
    const [mode, setModeS] = useState(doc.getAttribute('data-mode') || 'dark');
    const [dens, setDensS] = useState(doc.getAttribute('data-density') || 'regular');
    const lang = window.vLang || 'en';
    const setSkin = v => { setSkinS(v); if (RC) RC.setSkin(v); };
    const setMode = v => { setModeS(v); if (RC && RC.supports(skin, v)) RC.setMode(v); };
    const setDens = v => { setDensS(v); if (RC) RC.setDensity(v); };
    const setLang = v => { if (v === lang) return; if (window.vSetLang) { window.vSetLang(v); } else { try { localStorage.setItem('viden-lang', v); } catch (e) {} location.reload(); } };
    return (
      <div>
        <div className="gset-h"><h2>{t('Appearance', '外观')}</h2></div>
        <p className="gset-sub">{t('Changes apply instantly, everywhere.', '所有改动即时全局生效。')}</p>
        <div className="card">
          <div className="chd">{t('Language', '语言')}</div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Display language', '界面语言')}</div><div className="l2">{t('Applies to the whole app', '作用于整个应用')}</div></div><Seg opts={[['en', 'English'], ['zh', '中文']]} val={lang} set={setLang}/></div>
        </div>
        <div className="card">
          <div className="chd">{t('Skin', '皮肤')}</div>
          <div className="setrow" style={{ paddingTop: 12, paddingBottom: 14 }}>
            <div className="skinrow">
              {schemes.map(s => (
                <span key={s[0]} className={'skdot ' + (skin === s[0] ? 'on' : '')} onClick={() => setSkin(s[0])}>
                  <span className="d" style={{ background: `var(--accent)`, filter: skin === s[0] ? 'none' : 'saturate(.55) opacity(.7)' }}></span>{s[1]}
                  {s[3] && s[3].length === 1 && <span style={{ fontSize: 8.5, color: 'var(--fg-faint)' }}>{t('dark only', '仅深色')}</span>}
                </span>
              ))}
            </div>
          </div>
        </div>
        <div className="card">
          <div className="chd">{t('Mode & density', '明暗与密度')}</div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Mode', '明暗')}</div><div className="l2">{t('Amber and Phosphor are dark-only', 'Amber 与 Phosphor 仅支持深色')}</div></div><Seg opts={[['dark', t('dark', '深色')], ['light', t('light', '浅色')]]} val={mode} set={setMode}/></div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Density', '密度')}</div></div><Seg opts={[['compact', t('compact', '紧凑')], ['regular', t('regular', '标准')], ['comfy', t('comfy', '宽松')]]} val={dens} set={setDens}/></div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Reduce motion', '减弱动效')}</div><div className="l2">{t('Disables pulses and scrolling tickers', '关闭脉冲与滚动指标条')}</div></div><Seg opts={[['system', t('system', '跟随系统')], ['off', t('off', '关')], ['on', t('on', '开')]]} val={'system'} set={() => {}}/></div>
        </div>
      </div>
    );
  }

  function SecKeyboard() {
    const KEYS = [
      [t('Command palette', '命令面板'), '⌘K'], [t('Focus mode', '专注模式'), '⌘.'], [t('New lane / delegate', '新建 lane / 委派'), '⌘L'],
      [t('Pop out thread', '弹出会话窗口'), '⇧⌘O'], [t('Toggle terminal dock', '开关终端坞'), '⌘J'], [t('Next / previous lane', '下一条 / 上一条 lane'), '⌃⇥ / ⌃⇧⇥'],
      [t('Jump to decision center', '跳转决策中心'), '⌘G'], [t('Approve focused prompt', '批准当前提示'), 'Y'], [t('Deny focused prompt', '拒绝当前提示'), 'N'],
    ];
    return (
      <div>
        <div className="gset-h"><h2>{t('Keyboard', '快捷键')}</h2></div>
        <p className="gset-sub">{t('Click a shortcut to re-record it. Conflicts are flagged immediately.', '点击键位即可重录，冲突会立即标出。')}</p>
        <div className="card">
          <div className="chd">{t('Shortcuts', '快捷键')}<span className="sp"></span><span className="selchip">{t('Reset all', '全部重置')}</span></div>
          {KEYS.map((k, i) => (
            <div key={i} className="setrow"><div className="lb"><div className="l1">{k[0]}</div></div><span className="kbd">{k[1]}</span></div>
          ))}
        </div>
      </div>
    );
  }

  function SecNotify() {
    const [desk, setDesk] = useState(true);
    const [mail, setMail] = useState(true);
    const [hook, setHook] = useState(false);
    return (
      <div>
        <div className="gset-h"><h2>{t('Notifications', '通知')}</h2></div>
        <p className="gset-sub">{t("When you're at the desk, gates wait quietly in the status bar. When you're away, email or a webhook brings you back — a gate should never wait overnight on a screen nobody's watching.", '人在桌前，闸安静地等在状态栏；人不在，email 或 webhook 把你拉回来 —— 闸不该在没人看的屏幕上等一夜。')}</p>
        <div className="card">
          <div className="chd">{t('Channels', '通道')}</div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Desktop notifications', '桌面通知')}</div><div className="l2">{t('Suppressed while the app is focused', '应用聚焦时自动抑制')}</div></div><Toggle on={desk} set={setDesk}/></div>
          <div className="setrow"><div className="lb"><div className="l1">Email <span className="tag away">{t('AWAY FALLBACK', '离桌兜底')}</span></div><div className="l2">{t('Sent only after the escalation delay — never per event', '仅在升级等待期过后发送 —— 绝不逐事件轰炸')}</div></div><Toggle on={mail} set={setMail}/></div>
          <div className="setrow"><div className="lb"><div className="l1">Webhook <span className="tag away">{t('AWAY FALLBACK', '离桌兜底')}</span></div><div className="l2">{t('POST JSON to Slack, Feishu or your own endpoint', 'POST JSON 到 Slack、飞书或自建端点')}</div></div><Toggle on={hook} set={setHook}/></div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Direct messages (Slack / Telegram) ', '私信直达（Slack / Telegram）')}<span className="tag soon">{t('SOON', '即将推出')}</span></div><div className="l2">{t('Use a webhook to cover this today', '当前可用 webhook 覆盖同一场景')}</div></div><Toggle on={false} set={() => {}}/></div>
        </div>
        <div className="card">
          <div className="chd">{t('Escalation', '升级')}</div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Escalate when unanswered for', '无人响应多久后升级')}</div><div className="l2">{t('Prompts that auto-deny on countdown are never escalated', '倒计时自动拒绝的提示不参与升级')}</div></div><span className="selchip"><span className="v">10 min</span><span className="car">▾</span></span></div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Events that escalate', '会升级的事件')}</div><div className="l2">{t('Waiting gates · high-risk permissions · stalled lanes · integration bounces', '等待中的闸 · 高危权限 · 停摆的 lane · 集成闸退回')}</div></div><span className="selchip"><span className="v">{t('4 defaults', '默认 4 类')}</span><span className="car">▾</span></span></div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Quiet hours', '免打扰时段')}</div><div className="l2">{t('Webhook only; everything else lands in the morning briefing', '只留 webhook；其余进次日晨间简报')}</div></div><span className="selchip"><span className="v">23:00 – 08:00</span><span className="car">▾</span></span></div>
        </div>
      </div>
    );
  }

  function SecPrivacy() {
    const [tel, setTel] = useState(false);
    return (
      <div>
        <div className="gset-h"><h2>{t('Privacy & Telemetry', '隐私与遥测')}</h2></div>
        <p className="gset-sub">{t('Your code and transcripts never leave this machine except to the providers you configure. Telemetry is off by default.', '代码与转录不出本机，唯一的出口是你配置的 provider。遥测默认关闭。')}</p>
        <div className="card">
          <div className="chd">{t('Telemetry', '遥测')}</div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Anonymous usage metrics', '匿名使用统计')}</div><div className="l2">{t('Feature counts and crash reports — no code, paths or prompts', '功能计数与崩溃报告 —— 不含代码、路径或 prompt')}</div></div><Toggle on={tel} set={setTel}/></div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Keep session transcripts on disk', '会话转录留存本机')}</div><div className="l2">{t('Powers resume, evidence and the audit timeline · ', '支撑续会话、证据与审计时间线 · ')}<span className="cfg">session_home</span></div></div><Toggle on={true} set={() => {}}/></div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Redact secrets in transcripts', '转录写盘前脱敏')}</div><div className="l2">{t('Masks environment values and key-shaped strings before writing', '写盘前掩码环境变量值与形似密钥的字符串')}</div></div><Toggle on={true} set={() => {}}/></div>
        </div>
      </div>
    );
  }

  function SecWorkspace() {
    return (
      <div>
        <div className="gset-h"><h2>{t('Workspace', '工作区')}</h2></div>
        <p className="gset-sub">{t('Precedence: CLI flags → ', '优先级：CLI 参数 → ')}<span className="cfg">VIDEN_*</span>{t(' env → project ', ' 环境变量 → 项目 ')}<span className="cfg">.viden/config.toml</span>{t(' → global. Gate rules and ownership live in ', ' → 全局。闸规则与归属在仓库根的 ')}<span className="cfg">viden.toml</span>{t(' at the repo root, shared with your team.', '，随仓库与团队共享。')}</p>
        <div className="card">
          <div className="chd">{t('Config files', '配置文件')}<span className="sp"></span><span className="selchip">{t('Open in editor', '在编辑器打开')}</span></div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Project config', '项目配置')}</div><div className="l2"><span className="cfg">~/code/viden/.viden/config.toml</span> · {t('loaded ✓', '已加载 ✓')}</div></div><span className="keychip ok">2 keys</span></div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Global config', '全局配置')}</div><div className="l2"><span className="cfg">~/.config/viden/config.toml</span> · {t('loaded ✓', '已加载 ✓')}</div></div><span className="keychip ok">7 keys</span></div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Gate rules & ownership', '闸规则与归属')}</div><div className="l2"><span className="cfg">viden.toml</span> · {t('gates are reviewed in the Decision Center, not here', '闸的评审在决策中心进行，不在此处')}</div></div><span className="selchip"><span className="v">{t('View', '查看')}</span></span></div>
        </div>
        <div className="card">
          <div className="chd">{t('Lanes & sessions', 'Lane 与会话')}</div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Session home', '会话存储位置')}</div><div className="l2">{t('Transcripts, checkpoints and evidence · ', '转录、检查点与证据 · ')}<span className="cfg">session_home</span></div></div><span className="txtin">~/.viden/sessions</span></div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Lane branch prefix', 'Lane 分支前缀')}</div><div className="l2">{t('Worktree branches are named ', 'worktree 分支命名为 ')}<span className="cfg">vd/&lt;slug&gt;</span></div></div><span className="txtin">vd/</span></div>
          <div className="setrow"><div className="lb"><div className="l1">{t('Worktree root', 'Worktree 根目录')}</div><div className="l2">{t('Where isolated lane worktrees are created', '隔离 lane worktree 的创建位置')}</div></div><span className="txtin">../viden-worktrees</span></div>
        </div>
      </div>
    );
  }

  function SettingsView({ defaultSection }) {
    const SECTIONS = [
      { id: 'provider',   nm: t('Provider & Models', 'Provider 与模型'),  C: SecProvider },
      { id: 'permission', nm: t('Permissions', '权限'),                   C: SecPermission },
      { id: 'appearance', nm: t('Appearance', '外观'),                    C: SecAppearance },
      { id: 'keyboard',   nm: t('Keyboard', '快捷键'),                    C: SecKeyboard },
      { id: 'notify',     nm: t('Notifications', '通知'),                 C: SecNotify },
      { id: 'privacy',    nm: t('Privacy & Telemetry', '隐私与遥测'),     C: SecPrivacy },
      { id: 'workspace',  nm: t('Workspace', '工作区'),                   C: SecWorkspace },
    ];
    const [sec, setSec] = useState(defaultSection || 'provider');
    const Cur = (SECTIONS.find(s => s.id === sec) || SECTIONS[0]).C;
    return (
      <div className="gset-root center">
        <div className="cmdbar2">
          <span className="t">{t('Settings', '设置')}</span>
          <span className="src"><span className="p">.viden/config.toml</span> {t('+1 global', '+1 全局')}</span>
          <span className="sp"></span>
          <span className="srch">⌕ {t('search settings…', '搜索设置…')}</span>
        </div>
        <div className="gset-body">
          <div className="gset-nav">
            <div className="gset-navhd">{t('Preferences', '偏好')}</div>
            {SECTIONS.map(s => (
              <div key={s.id} className={'navrow ' + (sec === s.id ? 'on' : '')} onClick={() => setSec(s.id)}>
                {s.nm}
              </div>
            ))}
          </div>
          <div className="gset-main"><div className="gset-col"><Cur/></div></div>
        </div>
      </div>
    );
  }

  window.SettingsView = SettingsView;
})();
