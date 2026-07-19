/* Viden i18n — drop-in. EN default; floating EN/中 switch.
   Usage: <script src="i18n-dict.js"></script>  (optional, before this)
          <script src="i18n.js"></script>        BEFORE your app script.

   Two ways to get a string:
   · window.t('English', '中文')          inline pair — page-unique prose (legacy, still works)
   · window.tk('key')                     central dictionary lookup (i18n-dict.js · shared chrome)
   HTML auto-bind: <span data-i18n-key="open"></span> is filled from the dict on mount.

   切换模式（v2 · 热切换）：
   · 页面 <html data-i18n-live> 且为 React 页 → 切语言零刷新：t()/tk() 动态读
     window.vLang，切换时重渲已 hook 的 React 根 + 重绑 data-i18n-key + 派发
     'v-lang' 事件（React 状态/窗口位置全保留）。
     ⚠ live 页纪律：t() 必须在组件 render 内调用；模块级 const 里的 t() 结果会被
     冻在首个语言 —— 数据表写成函数（如 TOOLDEF()）或搬进组件。
   · 未标 data-i18n-live 的页面 → 照旧 location.reload()（模块级冻结也安全）。
   API：window.vSetLang('en'|'zh') —— 设置屏 Language 项与悬浮开关同走此入口。 */
(function(){
  var KEY='viden-lang';
  window.vLang=(localStorage.getItem(KEY)||'en');
  // t/tk 动态读 window.vLang（热切换的前提：不再闭包捕获）
  window.t=function(en,zh){return (window.vLang==='zh'&&zh!=null)?zh:en;};
  window.tk=function(key){
    var d=window.VIDEN_DICT&&window.VIDEN_DICT[key];
    if(!d) return key;
    return (window.vLang==='zh'&&d.zh!=null)?d.zh:d.en;
  };
  function bindKeys(){
    Array.prototype.forEach.call(document.querySelectorAll('[data-i18n-key]'),function(el){
      el.textContent=window.tk(el.getAttribute('data-i18n-key'));
    });
  }
  window.vBindI18nKeys=bindKeys;

  // ── React 根 hook（热切换用）：包一层 createRoot/render，记住每个根的最后元素 ──
  var roots=[];
  function hookReact(){
    var RD=window.ReactDOM;
    if(!RD||!RD.createRoot||RD.__vLangHooked)return;
    RD.__vLangHooked=true;
    var orig=RD.createRoot.bind(RD);
    RD.createRoot=function(container,opts){
      var root=orig(container,opts);
      var origRender=root.render.bind(root);
      root.render=function(el){root.__vEl=el;return origRender(el);};
      roots.push(root);
      return root;
    };
  }

  function syncButtons(){
    var d=document.getElementById('vlang');
    if(!d)return;
    Array.prototype.forEach.call(d.querySelectorAll('button'),function(b){
      b.className=(b.getAttribute('data-l')===window.vLang)?'on':'';
    });
  }

  window.vSetLang=function(v){
    if(v===window.vLang)return;
    try{localStorage.setItem(KEY,v);}catch(e){}
    var live=document.documentElement.hasAttribute('data-i18n-live');
    if(!live){location.reload();return;}
    window.vLang=v;
    syncButtons();
    bindKeys();
    roots.forEach(function(r){
      if(r.__vEl==null)return;
      try{
        // React 18 对同引用 element 会 bail out —— clone 出新引用（同 type/key/props）强制整树重跑
        var el=(window.React&&window.React.cloneElement)?window.React.cloneElement(r.__vEl):r.__vEl;
        r.render(el);
      }catch(e){}
    });
    try{window.dispatchEvent(new CustomEvent('v-lang',{detail:{lang:v}}));}catch(e){}
  };

  function mount(){
    hookReact();
    bindKeys();
    if(document.getElementById('vlang'))return;
    var s=document.createElement('style');
    s.textContent='#vlang{position:fixed;top:12px;right:12px;z-index:99999;display:flex;gap:2px;background:color-mix(in srgb,var(--bg-base) 92%,transparent);border:1px solid var(--border);border-radius:9px;padding:3px;font-family:ui-monospace,SFMono-Regular,monospace;backdrop-filter:blur(5px);box-shadow:var(--shadow-sm)}#vlang button{border:0;background:transparent;color:var(--fg-muted);font-size:12px;font-weight:700;padding:4px 11px;border-radius:6px;cursor:pointer;line-height:1}#vlang button.on{background:var(--bg-sel);color:var(--accent-bright)}#vlang button:hover{color:var(--fg-secondary)}';
    document.head.appendChild(s);
    var d=document.createElement('div');d.id='vlang';
    d.innerHTML='<button data-l="en">EN</button><button data-l="zh">\u4e2d</button>';
    document.body.appendChild(d);
    Array.prototype.forEach.call(d.querySelectorAll('button'),function(b){
      b.onclick=function(){window.vSetLang(b.getAttribute('data-l'));};
    });
    syncButtons();
  }
  if(document.readyState==='loading')document.addEventListener('DOMContentLoaded',mount);else mount();
})();
