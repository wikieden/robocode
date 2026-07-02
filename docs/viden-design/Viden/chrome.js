/* Viden chrome.js — 通用「皮肤(skin) + 明暗(mode) + 密度(density)」切换器
   （drop-in · 纯 JS · 在 i18n.js 之后、app 脚本之前加载）
   - 两轴换肤：data-skin（性格/强调色）× data-mode（明暗极性），挂在 <html> 上。
   - localStorage 键 rc-skin / rc-mode / rc-density → 任一页切换,全站持久一致。
     （旧键 rc-scheme 会被一次性迁移为 rc-skin+rc-mode。）
   - 皮肤注册表单一真源 = window.RC.SCHEMES：[id, en, zh, modes[]]。加/删皮肤只改这里。
     amber/phosphor = 复古终端族 modes=['dark']（dark-only）；其余 ['dark','light']。
   - 全程 var(--*) 上色 → 随皮肤换肤;皮肤圆点用 [data-skin][data-mode] 包裹自显其色。
   用法: <script src="i18n.js"></script><script src="chrome.js"></script> */
(function(){
  var root = document.documentElement;
  var t = window.t || function(en,zh){ return en; };

  /* ★ 皮肤注册表 = 单一真源（id, 英文名, 中文名, 支持的 modes）。加/删皮肤只改这里;
     索引页 / D1 / Aurora 一律读 window.RC.SCHEMES,勿再各抄一份。 */
  var SCHEMES = [
    ['aurora','Aurora','Aurora 青',   ['dark','light']],
    ['ice',   'Ice',   'Ice 冰蓝',    ['dark','light']],
    ['mono',  'Mono',  'Mono 灰',     ['dark','light']],
    ['amber', 'Amber', 'Amber 琥珀',  ['dark']],
    ['phosphor','Phosphor','Phosphor 绿', ['dark']]
  ];
  var MODES = [['dark','Dark','深','☾'],['light','Light','浅','☀']];
  var DENS  = [['compact','Compact','紧'],['regular','Regular','中'],['comfy','Comfy','松']];

  function skinDef(id){ return SCHEMES.find(function(s){return s[0]===id;}) || SCHEMES[0]; }
  function modesFor(id){ return skinDef(id)[3]; }
  function supports(id, m){ return modesFor(id).indexOf(m) !== -1; }

  /* 旧 rc-scheme（单值）→ 新 rc-skin + rc-mode 一次性迁移 */
  var LEGACY = { dark:['aurora','dark'], light:['aurora','light'], amber:['amber','dark'],
                 phosphor:['phosphor','dark'], ice:['ice','dark'], mono:['mono','dark'] };

  var RC = window.RC || {
    skin: function(){ return root.getAttribute('data-skin') || 'aurora'; },
    mode: function(){ return root.getAttribute('data-mode') || 'dark'; },
    setSkin: function(v){
      root.setAttribute('data-skin', v);
      // 切到 dark-only 皮肤时,若当前 mode 不支持则回落到该皮肤首个支持的 mode
      if(!supports(v, RC.mode())) RC.setMode(modesFor(v)[0], true);
      try{ localStorage.setItem('rc-skin', v); }catch(e){}
      emit(); sync();
    },
    setMode: function(v, silent){
      if(!supports(RC.skin(), v)) return;   // 当前皮肤不支持该 mode → 忽略
      root.setAttribute('data-mode', v);
      try{ localStorage.setItem('rc-mode', v); }catch(e){}
      if(!silent){ emit(); sync(); }
    },
    setDensity: function(v){
      root.setAttribute('data-density', v);
      try{ localStorage.setItem('rc-density', v); }catch(e){}
      emit(); sync();
    },
    density: function(){ return root.getAttribute('data-density') || 'compact'; },
    cycleSkin: function(){ var i = SCHEMES.findIndex(function(s){return s[0]===RC.skin();}); RC.setSkin(SCHEMES[(i+1)%SCHEMES.length][0]); },
    toggleMode: function(){ RC.setMode(RC.mode()==='dark'?'light':'dark'); }
  };
  if(!RC.SCHEMES) RC.SCHEMES = SCHEMES;
  if(!RC.MODES)   RC.MODES = MODES;
  if(!RC.DENS)    RC.DENS = DENS;
  if(!RC.modesFor) RC.modesFor = modesFor;
  if(!RC.supports) RC.supports = supports;
  window.RC = RC;

  function emit(){ window.dispatchEvent(new CustomEvent('rc-state', { detail:{ skin:RC.skin(), mode:RC.mode(), density:RC.density() } })); }

  /* 恢复持久化状态（含旧 rc-scheme 迁移）*/
  try{
    var sk = localStorage.getItem('rc-skin');
    var md = localStorage.getItem('rc-mode');
    if(!sk){
      var legacy = localStorage.getItem('rc-scheme');
      if(legacy && LEGACY[legacy]){ sk = LEGACY[legacy][0]; md = md || LEGACY[legacy][1]; }
    }
    if(sk) RC.setSkin(sk);
    if(md) RC.setMode(md);
    else if(!root.getAttribute('data-mode')) root.setAttribute('data-mode','dark');
    if(!root.getAttribute('data-skin')) root.setAttribute('data-skin','aurora');
    var d = localStorage.getItem('rc-density'); if(d) RC.setDensity(d);
    else if(!root.getAttribute('data-density')) root.setAttribute('data-density','compact');
  }catch(e){}

  var box = null;
  function sync(){
    if(!box) return;
    box.querySelectorAll('[data-sk]').forEach(function(el){ el.classList.toggle('on', el.getAttribute('data-sk')===RC.skin()); });
    box.querySelectorAll('[data-md]').forEach(function(el){
      var m = el.getAttribute('data-md');
      var ok = supports(RC.skin(), m);
      el.classList.toggle('on', ok && m===RC.mode());
      el.classList.toggle('disabled', !ok);
      el.title = ok ? '' : t('Retro skin is dark-only','复古皮肤仅深色');
    });
    box.querySelectorAll('[data-dn]').forEach(function(el){ el.classList.toggle('on', el.getAttribute('data-dn')===RC.density()); });
  }
  window.addEventListener('rc-state', sync);

  function mount(){
    if(document.getElementById('vchrome')) return;
    var st = document.createElement('style');
    st.textContent =
      '#vchrome{position:fixed;bottom:16px;right:16px;z-index:99998;display:flex;flex-direction:column;gap:6px;'
      + 'background:color-mix(in srgb,var(--bg-elev) 90%,transparent);border:1px solid var(--border);'
      + 'border-radius:9px;padding:7px 9px;font-family:var(--font-mono,ui-monospace,monospace);'
      + '-webkit-backdrop-filter:blur(6px);backdrop-filter:blur(6px);box-shadow:var(--shadow-sm)}'
      + '#vchrome .row{display:flex;align-items:center;gap:6px}'
      + '#vchrome .lb{font-size:8.5px;font-weight:700;letter-spacing:1.5px;color:var(--fg-muted);width:30px;flex:none}'
      + '#vchrome .sk{width:15px;height:15px;border-radius:50%;cursor:pointer;display:block;padding:0;'
      + 'background:var(--accent);border:1.5px solid transparent;box-sizing:border-box;outline:none;transition:transform .12s}'
      + '#vchrome .sk:hover{transform:scale(1.18)}'
      + '#vchrome .sk.on{border-color:var(--fg-primary)}'
      + '#vchrome .md{font-size:11px;line-height:1;color:var(--fg-muted);border:1px solid var(--border);'
      + 'background:transparent;border-radius:5px;padding:3px 7px;cursor:pointer;font-family:inherit}'
      + '#vchrome .md:hover{color:var(--fg-secondary);border-color:var(--accent-dim)}'
      + '#vchrome .md.on{color:var(--on-accent);background:var(--accent);border-color:var(--accent)}'
      + '#vchrome .md.disabled{opacity:.3;cursor:not-allowed;pointer-events:none}'
      + '#vchrome .dn{font-size:10px;font-weight:600;color:var(--fg-muted);border:1px solid var(--border);'
      + 'background:transparent;border-radius:5px;padding:2px 8px;cursor:pointer;line-height:1.2;font-family:inherit}'
      + '#vchrome .dn:hover{color:var(--fg-secondary);border-color:var(--accent-dim)}'
      + '#vchrome .dn.on{color:var(--on-accent);background:var(--accent);border-color:var(--accent)}';
    document.head.appendChild(st);

    box = document.createElement('div');
    box.id = 'vchrome';
    var r1 = '<div class="row"><span class="lb">SKIN</span>';
    SCHEMES.forEach(function(s){ r1 += '<span data-skin="'+s[0]+'" data-mode="dark"><button class="sk" data-sk="'+s[0]+'" title="'+s[1]+'" aria-label="'+s[1]+'"></button></span>'; });
    r1 += '</div>';
    var r2 = '<div class="row"><span class="lb">MODE</span>';
    MODES.forEach(function(m){ r2 += '<button class="md" data-md="'+m[0]+'" aria-label="'+m[1]+'">'+m[3]+'</button>'; });
    r2 += '</div>';
    var r3 = '<div class="row"><span class="lb">DENS</span>';
    DENS.forEach(function(d){ r3 += '<button class="dn" data-dn="'+d[0]+'">'+t(d[1],d[2])+'</button>'; });
    r3 += '</div>';
    box.innerHTML = r1 + r2 + r3;
    document.body.appendChild(box);

    box.querySelectorAll('[data-sk]').forEach(function(el){ el.onclick = function(){ RC.setSkin(el.getAttribute('data-sk')); }; });
    box.querySelectorAll('[data-md]').forEach(function(el){ el.onclick = function(){ RC.setMode(el.getAttribute('data-md')); }; });
    box.querySelectorAll('[data-dn]').forEach(function(el){ el.onclick = function(){ RC.setDensity(el.getAttribute('data-dn')); }; });
    sync();
  }
  if(document.readyState==='loading') document.addEventListener('DOMContentLoaded', mount); else mount();
})();
