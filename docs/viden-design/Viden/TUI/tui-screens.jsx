/* Viden TUI — 屏幕组件(真终端保真基线)· 导出到 window 供总览主文件使用 */
const {useState:useStateS} = React;
const t = window.t || ((en,zh)=>en);

/* ── seeded rng + minimap ── */
function rng(seed){let a=seed>>>0;return function(){a|=0;a=a+0x6D2B79F5|0;let t=Math.imul(a^a>>>15,1|a);t=t+Math.imul(t^t>>>7,61|t)^t;return((t^t>>>14)>>>0)/4294967296;};}
const TERR={snow:{nm:t('Snowfield','雪原'),c:'#74a8c9',p:'#3e6a85'},cave:{nm:t('Cavern','溶洞'),c:'#9b87d4',p:'#4a3f78'},volcano:{nm:t('Volcano','火山'),c:'#e0884f',p:'#7a4424'},isle:{nm:t('Floating Isle','浮岛'),c:'#54c0a0',p:'#2c6856'}};
function mapData(seed){
  const C=14,R=8,r=rng(seed),cells=[];
  for(let x=0;x<C;x++) if(r()>0.16) cells.push({x,y:R-1});
  const nP=3+Math.floor(r()*4);
  for(let p=0;p<nP;p++){const y=1+Math.floor(r()*(R-3)),x0=Math.floor(r()*(C-4)),len=2+Math.floor(r()*3);for(let x=x0;x<Math.min(C,x0+len);x++)cells.push({x,y});}
  const en=[],nE=4+Math.floor(r()*6);
  for(let e=0;e<nE;e++)en.push({x:Math.floor(r()*C),y:Math.floor(r()*(R-1))});
  return {C,R,cells,en,exitX:C-1-Math.floor(r()*2),enemies:nE};
}
function SvgMap({seed,terr}){
  const d=mapData(seed),cell=10,W=d.C*cell,H=d.R*cell,T=TERR[terr];
  return (
    <svg viewBox={`0 0 ${W} ${H}`} width="100%" style={{display:'block',aspectRatio:`${W}/${H}`}}>
      <rect width={W} height={H} style={{fill:'var(--bg-void)'}}/>
      {d.cells.map((q,i)=><rect key={i} x={q.x*cell} y={q.y*cell} width={cell-1} height={cell-1} fill={T.p}/>)}
      {d.en.map((q,i)=><circle key={'e'+i} cx={q.x*cell+cell/2} cy={q.y*cell+cell/2} r={1.8} style={{fill:'var(--error)'}}/>)}
      <rect x={1} y={(d.R-2)*cell+1} width={cell-2} height={cell-2} fill="none" style={{stroke:'var(--success)'}} strokeWidth="1.4"/>
      <rect x={d.exitX*cell+1} y={2} width={cell-2} height={cell-2} fill="none" style={{stroke:'var(--gold)'}} strokeWidth="1.4"/>
    </svg>
  );
}
function AsciiMap({seed}){
  const d=mapData(seed);
  const g=Array.from({length:d.R},()=>Array(d.C).fill('·'));
  d.cells.forEach(q=>g[q.y][q.x]='█');
  d.en.forEach(q=>{if(g[q.y][q.x]==='·')g[q.y][q.x]='●';});
  g[d.R-2][0]='◣'; g[0][d.exitX]='◆';
  return <div className="ascii">{g.map((row,y)=><div key={y}>{row.map((ch,x)=>{
    let cls=ch==='█'?'pf':ch==='●'?'en':ch==='◣'?'st':ch==='◆'?'ex':'';
    return <span key={x} className={cls}>{ch}</span>;
  })}</div>)}</div>;
}

const SPIN=['⠋','⠙','⠹','⠸','⠼','⠴','⠦','⠧','⠇','⠏'];
const STT={
  gate:{mk:'⏸',lbl:'GATE',cls:'t-gold'},
  run:{mk:null,lbl:'RUN',cls:'t-prog'},
  wait:{mk:'·',lbl:'WAIT',cls:'t-mut'},
  done:{mk:'✓',lbl:'DONE',cls:'t-suc'},
};

/* ── box-drawing frame (flex 保证 CJK 双宽下边框对齐) ── */
const DASH='──────────────────────────────────────────────────────────────';
function Frame({title,lid,tag,tagCls,foc,corners=['╭','╮','╰','╯'],brdOnly,style,children}){
  const [tl,tr,bl,br]=corners;
  const lines=React.Children.toArray(children);
  return (
    <div className={"tbox "+(foc?'foc ':'')+(brdOnly?'brdonly ':'')} style={style}>
      <div className="tl">
        <span className="bd">{tl}─</span>
        {title!=null && <span className="ttl">&nbsp;{lid&&<span className="lid">{lid}</span>}{lid?' · ':''}{title}&nbsp;</span>}
        <span className="fillc">{DASH}</span>
        {tag && <span className={"ttag "+(tagCls||'')}>&nbsp;{tag}&nbsp;</span>}
        <span className="bd">─{tr}</span>
      </div>
      {lines.map((ln,i)=>(
        <div className="tl" key={i}><span className="bd">│</span><span className="tc">{ln}</span><span className="bd">│</span></div>
      ))}
      <div className="tl"><span className="bd">{bl}</span><span className="fillc">{DASH+DASH}</span><span className="bd">{br}</span></div>
    </div>
  );
}

/* ───────────── LANE BOARD ───────────── */
const LANES=[
  {id:'L1',role:t('Design','策划'),agent:'claude',st:'gate',gtag:t('contract gate','契约闸'),sess:t('S1 · feel params v1.1','S1 · 手感参数 v1.1'),note:t('contracts/design-spec.md pending','contracts/design-spec.md 待批'),spark:'⡀⣄⣦⣶⣷⣿'},
  {id:'L2',role:'Gameplay',agent:'codex',st:'gate',gtag:t('replay-regression gate','回放回归闸'),sess:t('S1 · hitstun 0.18→0.24s','S1 · 受击硬直 0.18→0.24s'),note:t('replay 240f · asserts 2✓1⚠','回放 240f · 断言 2✓1⚠'),spark:'⣀⣠⣤⣶⣶⣦'},
  {id:'L3',role:'Gameplay',agent:'codex',st:'run',sess:t('S2 · dash-cancel window','S2 · 冲刺取消窗口'),note:t('editing dash.gd · 38s','编辑 dash.gd · 38s'),spark:'⣀⣦⣷⣿⣷⣦'},
  {id:'L4',role:t('Content gen','内容生成'),agent:t('built-in','内置'),st:'gate',gtag:t('gallery gate','画廊闸'),sess:t('S2 · level variants ×20','S2 · 关卡变体 ×20'),note:t('20 keep/kill to review','20 keep/kill 待评审'),spark:'⣿⣷⣦⣤⣄⣀'},
  {id:'L6',role:t('Render','渲染'),agent:'kiro',st:'wait',sess:t('S1 · cavern volumetric fog','S1 · 溶洞体积雾'),note:t('QH offline · golden-frame gate backlog','QH 离线 · 金样闸积压'),spark:'⣄⣄⣠⣀⣀⣀'},
  {id:'L0',role:t('Scribe','书记官'),agent:t('built-in','内置'),st:'done',sess:t('daily briefing generated','每日简报已生成'),note:t('09:00 distributed · read-only','09:00 已分发 · 只读'),spark:'⣶⣶⣶⣶⣶⣶'},
];
function Board({foc,corners,brdOnly,spin}){
  return (
    <div className="tboard">
      {LANES.map((l,i)=>{const s=STT[l.st];return (
        <Frame key={l.id} lid={l.id} title={l.role} tag={(l.st==='run'?spin:s.mk)+' '+s.lbl} tagCls={s.cls} foc={foc===i} corners={corners} brdOnly={brdOnly}>
          <span><span className="t-fnt">agent</span> <span className="t-br">{l.agent}</span>{l.gtag&&<span className="t-gold"> · {l.gtag}</span>}</span>
          <span className="t-sec">{l.sess}</span>
          <span><span className={l.st==='run'?'t-prog':'t-fnt'}>{l.spark}</span>  <span className={l.st==='gate'?'t-gold':'t-mut'}>{l.note}</span></span>
        </Frame>
      );})}
    </div>
  );
}

/* ───────────── DECISION CENTER · 文本档 ───────────── */
const QUEUE=[
  {id:'q1',ic:'§',icc:'t-gold',lane:'L1',t:t('Contract change: feel params v1.0→v1.1','契约变更:手感参数 v1.0→v1.1'),tag:t('contract','契约'),sub:'claude · contracts/**'},
  {id:'q2',ic:'⏸',icc:'t-gold',lane:'L2',t:t('Replay-regression gate: hitstun 0.18→0.24s','回放回归闸:受击硬直 0.18→0.24s'),tag:t('gate','闸'),sub:'codex · src/enemy/**'},
  {id:'q3',ic:'?',icc:'t-warn',lane:'L4',t:t('Ask: content-gen budget 82%, keep running?','问询:内容生成预算 82%,是否续跑'),tag:t('ask','问询'),sub:t('built-in · budget','内置 · budget')},
];
function DecisionCenter({sel,setSel,corners,brdOnly,openBrowser}){
  return (
    <div className="dcgrid">
      <div className="dccol">
        <Frame title={t('Decision queue · gate/ask/contract together','决策队列 · 闸/问询/契约同列')} corners={corners}>
          {QUEUE.map(q=>(
            <span key={q.id} className={sel===q.id?'qrow rev':'qrow'} onClick={()=>setSel(q.id)} style={{cursor:'pointer'}}>
              <span className={sel===q.id?'':q.icc}>{q.ic}</span> <b>{q.lane}</b> <span className={sel===q.id?'':'t-fnt'}>[{q.tag}]</span> {q.t}
            </span>
          ))}
          <span className="t-fnt">{t('─ Cleared · today ──────────────','─ 已清 · 今日 ──────────────')}</span>
          <span className="t-mut">{t('✓ L2 jump feel 16:32 approved · ↩ L6 volumetric fog 14:02 returned','✓ L2 跳跃手感 16:32 批 · ↩ L6 体积雾 14:02 退')}</span>
        </Frame>
        <div className="t-fnt" style={{padding:'7px 2px 0',fontSize:'11.5px',lineHeight:1.6}}>{t('Queue aggregates all lanes · ','队列聚合全部 lane · ')}<span className="t-br">j/k</span>{t(' move up/down · ',' 上下移 · ')}<span className="t-br">↵</span>{t(' expand evidence',' 展开证据')}</div>
      </div>

      <div className="dccol">
        <div className="dchdr">
          <span className="t-gold">⏸</span>{t(' replay-regression gate · ',' 回放回归闸 · ')}<span className="t-pri bld">L2</span> codex {t('· hitstun ','· 受击硬直 ')}<span className="t-mut">0.18 →</span> <span className="t-br">0.24s</span>
          <div className="t-fnt" style={{marginTop:'3px',fontSize:'11.5px'}}>{t('Evidence written back ','证据回写 ')}<span className="t-acc">docs/evidence/v0.6/stagger/</span>{t(' · runner Godot headless 4.3 · policy src/enemy/** = auto-validate + human approve',' · runner Godot headless 4.3 · 策略 src/enemy/** = 自动验证 + 人批')}</div>
        </div>

        <Frame title={t('① Code diff','① 代码 diff')} tag={t('text-native','文本原生')} tagCls="t-suc" corners={corners}>
          <span className="t-acc">src/enemy/stagger.gd · +1 −1</span>
          <span className="t-acc">{'@@ -12,6 +12,6 @@ func apply_hit(dmg):'}</span>
          <span><span className="t-fnt">12 </span><span className="t-sec">{'    var kb = _knockback(dmg)'}</span></span>
          <span className="difdel"><span className="t-fnt">13 </span>{t('- const HITSTUN := 0.18  # old: combos stick easily','- const HITSTUN := 0.18  # 旧:连段易卡')}</span>
          <span className="difadd"><span className="t-fnt">13 </span>{t('+ const HITSTUN := 0.24  # align contract §4','+ const HITSTUN := 0.24  # 对齐契约 §4')}</span>
          <span><span className="t-fnt">14 </span><span className="t-sec">{'    stun_timer.start(HITSTUN)'}</span></span>
        </Frame>

        <Frame title={t('② Deterministic replay + assertions','② 确定性回放 + 断言')} tag={t('text-native · Q3 input sequence','文本原生 · Q3 输入序列')} tagCls="t-suc" corners={corners}>
          <span className="t-mut">{t('seq ','序列 ')}<span className="t-acc">replay/stagger-combo.vseq</span>{t(' · 240f · deterministic ',' · 240f · 确定性 ')}<span className="t-suc">{t('✓ replayable','✓ 可重演')}</span> · 1.2s</span>
          <span><span className="t-suc">✓</span> <span className="t-pri">{t('Combos no longer stick at two','连段不再卡两段')}</span>{t(' — input buffer consumed normally ',' — 输入缓冲正常消费 ')}<span className="t-fnt">@38f</span></span>
          <span><span className="t-suc">✓</span> <span className="t-pri">{t('Counter window ≥ 6 frames','反击窗口 ≥ 6 帧')}</span>{t(' — measured 7 frames, recoverable ',' — 实测 7 帧,可救 ')}<span className="t-fnt">@38–45f</span></span>
          <span><span className="t-warn">⚠</span> <span className="t-pri">{t('Heavy stunlock 28 frames','重击 stunlock 28 帧')}</span>{t(' — threshold 30, close but not crossed ',' — 阈值 30,接近未越线 ')}<span className="t-fnt">@96f</span></span>
        </Frame>

        <Frame title={t('③ Behavior metrics','③ 行为指标')} tag={t('braille · swappable to sixel','braille · 可换 sixel 图')} tagCls="t-acc" corners={corners}>
          <span><span className="t-mut" style={{display:'inline-block',width:'9ch'}}>{t('Combo break','连段中断率')}</span> <span className="t-err" style={{fontSize:'15px'}}>⣿⣷⣶⣦⣤⣄⣀⣀</span>  {t('before','改前')} <span className="t-err">31%</span> → <span className="t-suc">12%</span></span>
          <span><span className="t-mut" style={{display:'inline-block',width:'9ch'}}>{t('Recovery','救场成功率')}</span> <span className="t-suc" style={{fontSize:'15px'}}>⣀⣀⣄⣤⣦⣶⣷⣿</span>  {t('before','改前')} <span className="t-mut">44%</span> → <span className="t-suc">68%</span></span>
        </Frame>

        <div className="vopen">
          <span className="t-gold bld">$ viden open L2</span> <span className="t-sec">{t('Watch actual replay video / pixel diff → open browser','看实际回放录像 / 像素 diff → 弹浏览器')}</span>
          <button className="vopenbtn" onClick={openBrowser}>{t('↗ open browser','↗ 弹浏览器')}</button>
        </div>
        <div className="dcact">
          <span className="kk ap">{t('[a] approve + merge evidence','[a] 批准并入证据')}</span>
          <span className="kk rj">{t('[r] reject','[r] 驳回')}</span>
          <span className="kk rt">{t('[b] return to lane','[b] 退回原 lane')}</span>
          <span className="t-fnt" style={{marginLeft:'auto',fontSize:'11.5px'}}>{t('reason persisted to gate log with decision','理由随决策落盘闸日志')}<span className="curblink">▏</span></span>
        </div>
      </div>
    </div>
  );
}

/* ───────────── GALLERY · 键盘流 ───────────── */
const VARIANTS=Array.from({length:8},(_,i)=>{
  const terr=['snow','cave','volcano','isle'][i%4];const r=rng(2200+i*41);
  return {id:i+1,terr,seed:7000+i*17,rooms:3+Math.floor(r()*4),enemies:4+Math.floor(r()*8),dur:(52+r()*46).toFixed(0),soft:(i===5)};
});
function Gallery({gfx,cur,setCur,verdict,submitted}){
  const nP=20-Object.values(verdict).filter(v=>v).length;
  const nK=Object.values(verdict).filter(v=>v==='keep').length;
  const nX=Object.values(verdict).filter(v=>v==='kill').length;
  return (
    <div className="glwrap">
      <div className="gltop">
        <span><span className="t-gold">⏸</span> <span className="t-pri bld">{t('Gallery gate','画廊闸')}</span>{t(' · L4 built-in · level-variants',' · L4 内置 · level-variants')} <span className="t-fnt">{t('· page 1/3 · showing 8/20','· 页 1/3 · 显示 8/20')}</span></span>
        <span className="sp"></span>
        <span className="t-fnt">{t('render ','渲染 ')}{gfx?<span className="t-acc">{t('◉ kitty/sixel inline','◉ kitty/sixel 内联')}</span>:<span className="t-mut">{t('▦ ASCII degraded','▦ ASCII 降级')}</span>} · keep <span className="t-suc">{nK}</span> kill <span className="t-err">{nX}</span> {t('pending','待定')} <span className="t-fnt">{nP}</span></span>
      </div>
      <div className="glgrid">
        {VARIANTS.map((v,i)=>{const T=TERR[v.terr],vd=verdict[v.id];return (
          <div key={v.id} className={"gc "+(cur===i?'foc ':'')+(vd==='kill'?'kill ':'')+(vd==='keep'?'keep':'')} onClick={()=>setCur(i)}>
            <div className={"gctop "+(cur===i?'rev':'')}>#{String(v.id).padStart(2,'0')} <span style={cur===i?null:{color:T.c}}>{T.nm}</span>{vd&&<span className={"gcvd "+(vd==='keep'?'t-suc':'t-err')}> {vd==='keep'?'KEEP':'KILL'}</span>}</div>
            <div className="gcbody">{gfx?<SvgMap seed={v.seed} terr={v.terr}/>:<AsciiMap seed={v.seed}/>}</div>
            <div className="gcstat">{v.soft?<span className="t-warn">{t('⚠ softlock','⚠软锁')}</span>:<span className="t-suc">{t('✓ reachable','✓可达')}</span>} <span className="t-mut">{v.rooms}{t(' rm ',' 房 ')}{v.enemies}{t(' en ',' 敌 ')}≈{v.dur}s</span></div>
          </div>
        );})}
      </div>
      <div className="glfoot">
        <span className="t-fnt">{t('current ','当前 ')}<span className="t-pri">#{String(VARIANTS[cur].id).padStart(2,'0')} {TERR[VARIANTS[cur].terr].nm}</span>{t(' · seed ',' · seed ')}{VARIANTS[cur].seed}{t(' · kill→levels/.archive recoverable · writes ',' · kill→levels/.archive 可复活 · 写 ')}<span className="t-acc">gallery.md</span></span>
        <span className="sp"></span>
        <span className={"glsub "+((nP>0||submitted)?'dis':'')}>{submitted?t('Submitted ✓','已提交 ✓'):nP>0?t(`${nP} still pending`,`还有 ${nP} 待定`):t('⌘↵ submit gallery gate','⌘↵ 提交画廊闸')}</span>
      </div>
    </div>
  );
}

/* ── browser handoff overlay ── */
function BrowserOverlay({close}){
  return (
    <div className="ovl" onClick={close}>
      <div className="browser" onClick={e=>e.stopPropagation()}>
        <div className="bchrome"><div className="bdots"><i style={{background:'var(--error)'}}></i><i style={{background:'var(--warning)'}}></i><i style={{background:'var(--success)'}}></i></div><div className="burl"><span className="t-acc">viden://</span>evidence/L2/stagger/replay · localhost:7420</div><span className="bx" onClick={close}>esc ✕</span></div>
        <div className="bbody">
          <div className="bbt"><span className="t-gold">↗ viden open L2</span>{t(' replay-regression gate · rich evidence view',' 回放回归闸 · 富证据视图')}</div>
          <div className="t-mut" style={{fontSize:'11.5px',marginTop:'4px'}}>{t('Popped from the TUI · complex review done in browser/GUI · decision still writes back to the terminal queue','从 TUI 弹出 · 复杂评审在浏览器/GUI 完成 · 决策仍回写终端队列')}</div>
          <div className="player"><span className="pab">{t('replay/stagger-combo.vseq · live replay','replay/stagger-combo.vseq · 现场重演')}</span><div className="pply">▶</div><div className="pscrub"><span>00:38</span><span className="pbar"><i></i></span><span className="t-mut">240f · 1×</span></div></div>
          <div className="t-fnt" style={{fontSize:'11px',marginTop:'10px',lineHeight:1.7}}>{t('Same evidence, two views · watch video/pixel diff here, back in ','同一份证据两种视图 · 此处看录像/像素 diff,回 ')}<span className="t-acc">TUI</span>{t(' press ',' 按 ')}<b>a</b>{t(' to approve · braille metrics/asserts/diff already readable in place in the terminal',' 批准 · braille 指标/断言/diff 已在终端就地可读')}</div>
        </div>
      </div>
    </div>
  );
}

/* ───────────── SESSION · 会话主窗口(干活侧) ───────────── */
const SLASH=[
  {c:'/gate', d:t('View gates this session triggered','查看本会话触发的闸'), go:'review'},
  {c:'/review', d:t('Request review from another lane','对另一条 lane 发起评审请求'), go:'review'},
  {c:'/lanes', d:t('Back to lane board overview','回 lane board 总览'), go:'board'},
  {c:'/gallery', d:t('Open L4 gallery gate','打开 L4 画廊闸'), go:'gallery'},
  {c:'/handoff', d:t('Hand this lane to a teammate (with session context)','把本 lane 移交同事(含会话上下文)'), go:null},
  {c:'/target', d:t('Switch execution target local / ssh','切换执行目标 local / ssh'), go:null},
  {c:'/model', d:t('Switch model / configure routing','换模型 / 配路由策略'), go:null},
  {c:'/clear', d:t('Clear session context','清空会话上下文'), go:null},
];
function Session({onCommand}){
  const [val,setVal]=useStateS('');
  const [pi,setPi]=useStateS(0);
  const slash=val.startsWith('/');
  const matches=slash?SLASH.filter(s=>s.c.startsWith(val.split(' ')[0])):[];
  const run=(cmd)=>{const m=SLASH.find(s=>s.c===cmd);setVal('');setPi(0);if(m&&m.go)onCommand(m.go);};
  const onKey=(e)=>{
    if(slash&&matches.length){
      if(e.key==='ArrowDown'){setPi(p=>Math.min(matches.length-1,p+1));e.preventDefault();return;}
      if(e.key==='ArrowUp'){setPi(p=>Math.max(0,p-1));e.preventDefault();return;}
      if(e.key==='Tab'){setVal(matches[pi].c+' ');e.preventDefault();return;}
      if(e.key==='Enter'){run(matches[pi].c);e.preventDefault();return;}
    }
    if(e.key==='Enter'){setVal('');}
  };
  return (
    <div className="sess">
      <div className="sessctx">
        <span><span className="t-acc bld">L2</span> <span className="t-pri">Gameplay</span> <span className="t-fnt">·</span> <span className="t-br">codex</span> <span className="t-mut">gpt-5-codex</span></span>
        <span className="t-fnt">·</span><span className="t-mut">⊨ local · godot 4.3</span>
        <span className="t-fnt">·</span><span className="t-mut">⌥ vd/stagger @ 9f3ac21</span>
        <span className="sp"></span>
        <span className="t-mut">ctx <span className="t-sec">24%</span> · budget <span className="t-sec">31k</span>/50k</span>
      </div>
      <div className="sesslog">
        <div className="umsg"><span className="p">❯</span><span className="x">{t('Bump hitstun from 0.18 to 0.24, run replay-regression to confirm combos no longer stick and the counter window lets players recover.','把受击硬直从 0.18 提到 0.24,跑回放回归确认连段不再卡、反击窗口够玩家救场。')}</span></div>
        <div className="amsg">
          <div className="ahdr">⡾ codex <span className="m">{t('· reads contracts/design-spec.md §4 · stagger.gd','· 读 contracts/design-spec.md §4 · stagger.gd')}</span></div>
          <div className="athink">{t('Thinking · contract §4 hitstun range 0.22–0.26s, current 0.18 too low → change the constant then verify with deterministic-replay assertions','思考 · 契约 §4 受击区间 0.22–0.26s,当前 0.18 偏低 → 改常数后用确定性回放断言验证')}</div>
          <div className="atext">{t('Plan: ① change ','计划:① 改 ')}<b>HITSTUN</b>{t(' constant → ② run replay sequence ',' 常数 → ② 跑回放序列 ')}<b>stagger-combo</b>{t(' → ③ check combo/counter/stunlock assertions.',' → ③ 看连段/反击/stunlock 断言。')}</div>
          <div className="tool">
            <div className="th"><span className="g">⚙ edit</span> · src/enemy/stagger.gd <span className="t-fnt">+1 −1</span></div>
            <div className="to del">{t('- const HITSTUN := 0.18  # old: combos stick easily','- const HITSTUN := 0.18  # 旧:连段易卡')}</div>
            <div className="to add">{t('+ const HITSTUN := 0.24  # align contract §4','+ const HITSTUN := 0.24  # 对齐契约 §4')}</div>
          </div>
          <div className="tool">
            <div className="th"><span className="g">⚙ run</span> · godot --headless replay/stagger-combo.vseq <span className="t-fnt">· 240f</span></div>
            <div className="to"><span className="t-mut">{t('Replayed 240 frames · deterministic ✓ · 1.2s','重演 240 帧 · 确定性 ✓ · 1.2s')}</span></div>
            <div className="to"><span className="ok">✓</span>{t(' combos no longer stick ',' 连段不再卡两段 ')}<span className="t-fnt">@38f</span>  <span className="ok">✓</span>{t(' counter window 7 frames ',' 反击窗口 7 帧 ')}<span className="t-fnt">@38–45f</span></div>
            <div className="to"><span className="wn">⚠</span>{t(' heavy stunlock 28 frames ',' 重击 stunlock 28 帧 ')}<span className="t-fnt">{t('threshold 30, close but not crossed','阈值 30,接近未越线')}</span></div>
          </div>
          <div className="gatemsg">
            <div style={{display:'flex',alignItems:'center',gap:'8px',marginBottom:'6px'}}>⏸ <b>{t('Replay-regression gate','回放回归闸')}</b> <span className="t-fnt" style={{fontWeight:400}}>src/enemy/** · {t('evidence above','证据见上')}</span></div>
            <div style={{lineHeight:1.55,marginBottom:'8px'}}>{t('Approve the hitstun change and merge its evidence? Combos fixed · counter window 7f · heavy stunlock 28f stays under the 30f threshold.','批准受击硬直改动并并入证据?连段已修 · 反击窗口 7f · 重击 stunlock 28f 仍在 30f 阈值内。')}</div>
            <div className="dcact">
              <span className="kk ap">{t('[a] approve + merge','[a] 批准并入')}</span>
              <span className="kk rj">{t('[r] reject','[r] 驳回')}</span>
              <span className="kk rt">{t('[b] return','[b] 退回')}</span>
              <span className="t-fnt" style={{marginLeft:'auto',fontSize:'11px'}}>{t('decide here · also queued in ','就地决定 · 也进')}<span className="lk">{t('Decision Center','决策中心')}</span></span>
            </div>
          </div>
          <div className="atext">{t('Done. stunlock 28f is near the threshold but not crossed; want me to clamp the cap to 26f to be safer?','完成。stunlock 28f 接近阈值但未越线;要我顺带把上限钳到 26f 更稳吗?')}</div>
        </div>
      </div>
      <div className="composer">
        {slash&&matches.length>0 && (
          <div className="palette">
            {matches.map((s,i)=>(
              <div key={s.c} className={"pcmd "+(i===pi?'on':'')} onMouseEnter={()=>setPi(i)} onClick={()=>run(s.c)}>
                <span className="pc">{s.c}</span><span className="pd">{s.d}</span>{s.go&&<span className="pg">{t('↗ switch','↗ 切屏')}</span>}
              </div>
            ))}
          </div>
        )}
        <div className="cline">
          <span className="cprompt">❯</span>
          <input className="cin" value={val} placeholder={t('Ask codex, or type / for commands (try /gate)…','问 codex,或敲 / 调命令(试试 /gate)…')} onChange={e=>{setVal(e.target.value);setPi(0);}} onKeyDown={onKey}/>
          <span className="cmodes"><span className="cm">gpt-5-codex</span><span className="cm">⊨ local</span><span className="cm">ctx 24%</span></span>
        </div>
        <div className="chint"><span className="t-fnt">{t('/ commands','/ 命令')}</span> <span className="t-mut">/gate · /review · /handoff · /target · /model</span><span className="sp"></span><span className="t-fnt">{t('⏎ send · ⌃C interrupt · ⇥ complete · ↑↓ history/select','⏎ 发送 · ⌃C 打断 · ⇥ 补全 · ↑↓ 历史/选命令')}</span></div>
      </div>
    </div>
  );
}

Object.assign(window,{rng,TERR,mapData,SvgMap,AsciiMap,SPIN,STT,Frame,Board,LANES,DecisionCenter,QUEUE,Gallery,VARIANTS,BrowserOverlay,Session});