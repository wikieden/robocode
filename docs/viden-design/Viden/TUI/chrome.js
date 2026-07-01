/* Viden chrome.js — 通用「皮肤 + 密度」切换器（drop-in · 纯 JS · 在 i18n.js 之后加载）
   - 与 Aurora 页共用 localStorage 键 rc-scheme / rc-density → 任一页切换,全站持久一致。
   - 暴露 window.RC（若已存在则复用,不覆盖）。
   - 全程 var(--*) 上色 → 随皮肤换肤;皮肤圆点用 [data-theme] 包裹,各自显示该皮肤强调色。
   用法: <script src="i18n.js"></script><script src="chrome.js"></script> 放在 app 脚本之前。 */
(function(){
  var root = document.documentElement;
  var SCHEMES = [['dark','Aurora'],['amber','Amber'],['phosphor','Phosphor'],['ice','Ice'],['mono','Mono'],['light','Light']];
  var DENS = [['compact','Compact','紧'],['regular','Regular','中'],['comfy','Comfy','松']];
  var t = window.t || function(en,zh){ return en; };

  function labelFor(v){ var s = SCHEMES.find(function(x){return x[0]===v;}); return s ? s[1] : 'Aurora'; }

  var RC = window.RC || {
    scheme: function(){ return root.getAttribute('data-theme') || 'dark'; },
    setScheme: function(v){
      root.setAttribute('data-theme', v);
      try{ localStorage.setItem('rc-scheme', v); }catch(e){}
      var tl = document.getElementById('themeLabel'); if(tl) tl.textContent = labelFor(v);
      window.dispatchEvent(new CustomEvent('rc-state', { detail:{ scheme:v } }));
      sync();
    },
    density: function(){ return root.getAttribute('data-density') || 'compact'; },
    setDensity: function(v){
      root.setAttribute('data-density', v);
      try{ localStorage.setItem('rc-density', v); }catch(e){}
      window.dispatchEvent(new CustomEvent('rc-state', { detail:{ density:v } }));
      sync();
    },
    cycle: function(){ var i = SCHEMES.findIndex(function(s){return s[0]===RC.scheme();}); RC.setScheme(SCHEMES[(i+1)%SCHEMES.length][0]); }
  };
  window.RC = RC;

  // 恢复持久化状态（若页面未显式设 data-density,默认 compact）
  try{
    var s = localStorage.getItem('rc-scheme'); if(s) RC.setScheme(s);
    var d = localStorage.getItem('rc-density'); if(d) RC.setDensity(d);
    else if(!root.getAttribute('data-density')) root.setAttribute('data-density','compact');
  }catch(e){}

  var box = null;
  function sync(){
    if(!box) return;
    box.querySelectorAll('[data-sk]').forEach(function(el){ el.classList.toggle('on', el.getAttribute('data-sk')===RC.scheme()); });
    box.querySelectorAll('[data-dn]').forEach(function(el){ el.classList.toggle('on', el.getAttribute('data-dn')===RC.density()); });
  }

  // 跟随外部切换（如 Aurora 页的 nav 按钮 / Tweaks 面板）保持高亮同步
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
      + '#vchrome .dn{font-size:10px;font-weight:600;color:var(--fg-muted);border:1px solid var(--border);'
      + 'background:transparent;border-radius:5px;padding:2px 8px;cursor:pointer;line-height:1.2;font-family:inherit}'
      + '#vchrome .dn:hover{color:var(--fg-secondary);border-color:var(--accent-dim)}'
      + '#vchrome .dn.on{color:var(--bg-void);background:var(--accent);border-color:var(--accent)}';
    document.head.appendChild(st);

    box = document.createElement('div');
    box.id = 'vchrome';
    var r1 = '<div class="row"><span class="lb">SKIN</span>';
    SCHEMES.forEach(function(s){ r1 += '<span data-theme="'+s[0]+'"><button class="sk" data-sk="'+s[0]+'" title="'+s[1]+'" aria-label="'+s[1]+'"></button></span>'; });
    r1 += '</div>';
    var r2 = '<div class="row"><span class="lb">DENS</span>';
    DENS.forEach(function(d){ r2 += '<button class="dn" data-dn="'+d[0]+'">'+t(d[1],d[2])+'</button>'; });
    r2 += '</div>';
    box.innerHTML = r1 + r2;
    document.body.appendChild(box);

    box.querySelectorAll('[data-sk]').forEach(function(el){ el.onclick = function(){ RC.setScheme(el.getAttribute('data-sk')); }; });
    box.querySelectorAll('[data-dn]').forEach(function(el){ el.onclick = function(){ RC.setDensity(el.getAttribute('data-dn')); }; });
    sync();
  }
  if(document.readyState==='loading') document.addEventListener('DOMContentLoaded', mount); else mount();
})();
