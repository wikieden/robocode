/* Viden i18n — drop-in. EN default; floating EN/中 switch; reload-based.
   Usage: <script src="i18n.js"></script> BEFORE your app script.
   Then call window.t('English', '中文') anywhere. */
(function(){
  var KEY='viden-lang';
  var lang=(localStorage.getItem(KEY)||'en');
  window.vLang=lang;
  window.t=function(en,zh){return (lang==='zh'&&zh!=null)?zh:en;};
  function mount(){
    if(document.getElementById('vlang'))return;
    var s=document.createElement('style');
    s.textContent='#vlang{position:fixed;top:12px;right:12px;z-index:99999;display:flex;gap:2px;background:color-mix(in srgb,var(--bg-base) 92%,transparent);border:1px solid var(--border);border-radius:9px;padding:3px;font-family:ui-monospace,SFMono-Regular,monospace;backdrop-filter:blur(5px);box-shadow:0 6px 20px rgba(0,0,0,.4)}#vlang button{border:0;background:transparent;color:var(--fg-muted);font-size:12px;font-weight:700;padding:4px 11px;border-radius:6px;cursor:pointer;line-height:1}#vlang button.on{background:var(--bg-sel);color:var(--accent-bright)}#vlang button:hover{color:var(--fg-secondary)}';
    document.head.appendChild(s);
    var d=document.createElement('div');d.id='vlang';
    d.innerHTML='<button data-l="en">EN</button><button data-l="zh">\u4e2d</button>';
    document.body.appendChild(d);
    Array.prototype.forEach.call(d.querySelectorAll('button'),function(b){
      if(b.getAttribute('data-l')===lang)b.className='on';
      b.onclick=function(){localStorage.setItem(KEY,b.getAttribute('data-l'));location.reload();};
    });
  }
  if(document.readyState==='loading')document.addEventListener('DOMContentLoaded',mount);else mount();
})();
