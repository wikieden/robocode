/* Viden TUI · 借自 hermes 的两个交互,做成真可交互组件
   - CommandGate: 4 档命令闸 + 键盘选 + 倒计时自动拒 + 拒绝兜底
   - StreamTimeline: 流式工具/技能时间线(preparing→result,逐行展开)
   导出到 window 供 T1 主文件使用。字形/颜色遵循 T4 §08。*/
const {useState:useStateGT, useEffect:useEffectGT, useRef:useRefGT} = React;
const tGT = window.t || ((en,zh)=>en);

/* ───────────────────────── 4 档命令闸 ───────────────────────── */
const GATE_OPTS = [
  {key:'once',   n:'1', label:tGT('Allow once','本次允许'),        sub:tGT('this one run','仅此一次')},
  {key:'session',n:'2', label:tGT('Allow for this session','本会话允许'), sub:tGT('until you quit','退出前有效')},
  {key:'allow',  n:'3', label:tGT('Add to allowlist','加入白名单'),  sub:tGT('permanent · this repo','永久 · 本仓库')},
  {key:'deny',   n:'4', label:tGT('Deny','拒绝'),                  sub:tGT('safe default','安全默认'), deny:true},
];

function CommandGate({cmd, consequence, risk, total=50, onResolve}){
  const [sel,setSel]=useStateGT(0);
  const [left,setLeft]=useStateGT(total);
  const doneRef=useRefGT(false);

  const finish=(decision)=>{ if(doneRef.current)return; doneRef.current=true; onResolve(decision); };

  // 倒计时:超时自动「拒」—— 绝不自动放行
  useEffectGT(()=>{
    const id=setInterval(()=>{
      setLeft(s=>{ if(s<=1){clearInterval(id); finish({decision:'deny',reason:'timeout'}); return 0;} return s-1; });
    },1000);
    return ()=>clearInterval(id);
  },[]);

  // 键盘:↑↓/jk 选 · 1-4 直选 · Enter 确认 · esc = 拒
  useEffectGT(()=>{
    const h=(e)=>{
      if(e.key==='Escape'){finish({decision:'deny',reason:'esc'});e.preventDefault();return;}
      if(e.key==='ArrowDown'||e.key==='j'){setSel(s=>Math.min(GATE_OPTS.length-1,s+1));e.preventDefault();}
      if(e.key==='ArrowUp'||e.key==='k'){setSel(s=>Math.max(0,s-1));e.preventDefault();}
      if(/^[1-4]$/.test(e.key)){const i=+e.key-1;setSel(i);finish({decision:GATE_OPTS[i].key});e.preventDefault();}
      if(e.key==='Enter'){finish({decision:GATE_OPTS[sel].key});e.preventDefault();}
    };
    window.addEventListener('keydown',h,true);return()=>window.removeEventListener('keydown',h,true);
  },[sel]);

  const pct=Math.max(0,left/total*100);
  // canonical 闸:消费 tui-kit 的 .vgate / .vscrim（与「统一原型」GateCard 同构，单一真源）
  return (
    <div className="vscrim" onClick={()=>finish({decision:'deny',reason:'click-out'})}>
      <div className="vgate" onClick={e=>e.stopPropagation()} style={{width:'min(540px,88%)'}}>
        <div className="vgate-head"><span className="badge">GATE</span>{tGT('Dangerous command · approval','危险命令 · 审批')}
          <span className="risk">{risk||tGT('ELEVATED','高危')}</span></div>
        <div className="vgate-body">
          <div className="vgate-cmd"><span className="vk-accent">$ </span>{cmd}</div>
          <div style={{fontSize:'11px',color:'var(--fg-muted)',margin:'-4px 0 9px'}}>↳ {consequence}</div>
          <div className="vgate-opts">
            {GATE_OPTS.map((o,i)=>(
              <div key={o.key} className={"vgate-opt "+(o.deny?'deny ':'')+(sel===i?'on':'')}
                   onMouseEnter={()=>setSel(i)} onClick={()=>finish({decision:o.key})}>
                <span className="n">{o.n}</span><span>{o.label}</span><span className="sub">{o.sub}</span>
              </div>
            ))}
          </div>
          <div className="vgate-foot"><span><b>↑↓</b> {tGT('select','选')}</span><span><b>1–4</b> {tGT('direct','直选')}</span><span><b>⏎</b> {tGT('confirm','确认')}</span>
            <span className="cd">{tGT('auto-deny','超时拒')} {left}s<span className="bar"><i style={{width:pct+'%'}}></i></span></span></div>
        </div>
      </div>
    </div>
  );
}

/* ─────────────────── 流式工具/技能时间线 ─────────────────── */
/* 每步:{cls,ic,lb,ar,dur,gut} —— 颜色=状态(T4 §08) */
const STREAM_STEPS=[
  {cls:'prep', ic:'◌', lb:tGT('preparing','准备'), ar:'clarify…',                              dur:'',     gut:'│', delay:600},
  {cls:'ask',  ic:'◆', lb:'clarify',               ar:tGT('“Bump to 0.24 or 0.22?” → 0.24','“提到 0.24 还是 0.22?” → 0.24'), dur:'2.4s', gut:'├', delay:1500},
  {cls:'done', ic:'▣', lb:'skill',                 ar:'viden-agent · replay-regression',       dur:'0.0s', gut:'├', delay:700},
  {cls:'ok',   ic:'✓', lb:'read',                  ar:'contracts/L1.toml · stagger.gd +2',     dur:'0.3s', gut:'├', delay:900},
  {cls:'ok',   ic:'✓', lb:'edit',                  ar:'src/enemy/stagger.gd · +1 −1',          dur:'0.2s', gut:'├', delay:800},
  {cls:'run',  ic:'▶', lb:'run',                   ar:'godot --headless replay/stagger-combo.vseq', dur:'…', gut:'╰', delay:1600, running:true},
  {cls:'ok',   ic:'✓', lb:'run',                   ar:tGT('combos clear ✓ · counter 7f ✓ · stunlock 28f ⚠','连段清 ✓ · 反击 7f ✓ · stunlock 28f ⚠'), dur:'1.9s', gut:'╰', delay:0, replaceRun:true},
];

function StreamTimeline(){
  const [n,setN]=useStateGT(0);          // 已显示步数
  const [spin,setSpin]=useStateGT(0);
  const [playing,setPlaying]=useStateGT(true);
  const SP=['⠋','⠙','⠹','⠸','⠼','⠴','⠦','⠧','⠇','⠏'];

  useEffectGT(()=>{const id=setInterval(()=>setSpin(s=>(s+1)%SP.length),90);return()=>clearInterval(id);},[]);
  useEffectGT(()=>{
    if(!playing) return;
    if(n>=STREAM_STEPS.length){setPlaying(false);return;}
    const step=STREAM_STEPS[n];
    const id=setTimeout(()=>setN(x=>x+1), step.delay||700);
    return ()=>clearTimeout(id);
  },[n,playing]);

  const replay=()=>{setN(0);setPlaying(true);};
  // 渲染:run 步骤若后面已出现 replaceRun,则被结果行替换
  const hasResult = n>=STREAM_STEPS.length;
  const rows = STREAM_STEPS.slice(0,n).filter(s=>!(s.replaceRun)||true);
  // 展示逻辑:running 的 run 行在结果行出现后切成 ✓
  const visible = [];
  STREAM_STEPS.slice(0,n).forEach((s)=>{
    if(s.replaceRun){ // 把上一条 run 行替换为结果
      const idx=visible.findIndex(v=>v.running);
      if(idx>=0){visible[idx]=s;return;}
    }
    visible.push(s);
  });

  const done = n>=STREAM_STEPS.length;
  return (
    <div className="stl">
      <div className="stlhdr">
        <span className="dot" style={{background:done?'var(--success)':'var(--progress)'}}></span>
        <span className="ttl">{done?tGT('stream complete','流结束'):tGT('agent working…','agent 干活中…')}</span>
        <span className="meta">{Math.min(n,STREAM_STEPS.length)}/{STREAM_STEPS.length} {tGT('steps','步')}</span>
        <span className="sp"></span>
        <span className="replay" onClick={replay}>↻ {tGT('replay','重播')}</span>
      </div>
      <div className="stlbody">
        {visible.map((s,i)=>(
          <div key={i} className={"stlr "+s.cls}>
            <span className="gut">{s.gut}</span>
            <span className="ic">{s.running?SP[spin]:s.ic}</span>
            <span className="lb">{s.lb}</span>
            <span className="ar">{s.ar}</span>
            <span className="du">{s.running?tGT('running','运行中'):s.dur}</span>
          </div>
        ))}
        {done && (
          <div className="stlgate">⏸ {tGT('Change touched ','改动触及 ')}<b>src/enemy/**</b>{tGT(' → replay-regression gate formed with evidence. ',' → 已成回放回归闸并附证据。')}<span className="lk">/gate</span> {tGT('to review.','查看。')}</div>
        )}
      </div>
    </div>
  );
}

Object.assign(window, { CommandGate, StreamTimeline });
