/* ════════════════════════════════════════════════════════════════
 * gui-titlebar.jsx · Viden GUI 窗口级标题栏（canonical · 单一真源）
 *
 * 所有 GUI 桌面窗口共用的顶栏：46px · 红黄绿灯 + wordmark + projsel +
 * gitops(sync / worktrees) + 工具组(palette / focus / popout / ▤索引 / settings)。
 * 视觉母版 = D1 驾驶舱 .titlebar；抽成组件后各页零重造、改一处全改。
 * 样式真源 = gui-kit.css(.titlebar/.tbtools/.gitops/.projsel…)，本组件不注样式。
 *
 * 用法（页面已加载 React + Babel + gui-kit.css）：
 *   <script type="text/babel" src="../gui-titlebar.jsx"></script>   // 主脚本之前
 *   渲染为 .frame / .win 的第一个子节点：
 *     <GuiTitleBar project="viden" branch="main" worktrees={4} />
 *
 * props（都可省，给的是 Aurora 默认占位）：
 *   project   : 项目名（projsel 左字）           默认 'viden'
 *
 * 另导出 <GuiPopoutBar> —— popout 独立窗口（D10 监视墙/竖条等）的轻量顶栏：
 *   .winbar 度量同源 gui-kit；dots + 字标 + 标题 + 右侧插槽。
 *   <GuiPopoutBar title={<b>Lane Monitor</b>}> <span className="pin on">…</span> </GuiPopoutBar>
 *   字标/dots 不再各页手抄（popout chrome 去重 · 评审反馈 2026-07-02 第5项）。
 *   branch    : 分支名（projsel ⎇ 右字）          默认 'main'
 *   git       : 是否显示 gitops 块                默认 true
 *   sync      : {up,down} 同步计数 | false 隐藏   默认 {up:2,down:0}
 *   worktrees : worktree 数 | null 隐藏该 chip    默认 4
 *   tools     : 工具组按钮 key 顺序数组           默认见下 DEFAULT_TOOLS
 *               可用 key: palette focus popout term panel index settings | (分隔)
 *   active    : 当前高亮工具 key                  默认 null
 *   onTool    : (key)=>void 点击回调（可省，纯展示页无需）
 *   indexHref : ▤ 设计稿索引链接                  默认 '../Viden - 设计稿索引 (GUI).html'
 *   dim       : projsel 半透明（如 no-project）   默认 false
 *   children  : projsel 之后、工具组之前插入的额外节点（如 crumb 面包屑）
 * ════════════════════════════════════════════════════════════════ */
(function () {
  // 图标真源 = gui-icons.jsx（window.ICONS）。本组件不再内嵌画法——
  // 渲染时按 key 取 ICONS（必须先于本文件加载 gui-icons.jsx）。小号 15px = "ic sm"。
  const icon = (k) => window.GuiIcon ? <window.GuiIcon name={k} className="ic sm"/> : null;
  const TITLES = {
    palette: 'Command palette ⌘K',
    focus: 'Focus mode · 隐藏侧栏 ⌘.',
    popout: 'Pop out thread',
    term: '召唤坞 · 下边栏 ⌘J',
    panel: '上下文面板 ⌥⌘B',
    index: '设计稿索引 · 全部 GUI 设计稿 ↗',
    settings: 'Settings',
  };
  const DEFAULT_TOOLS = ['palette', 'focus', 'popout', '|', 'index', 'settings'];

  function GuiTitleBar(props) {
    const project = props.project != null ? props.project : 'viden';
    const branch = props.branch != null ? props.branch : 'main';
    const git = props.git !== false;
    const sync = props.sync === undefined ? { up: 2, down: 0 } : props.sync;
    const worktrees = props.worktrees === undefined ? 4 : props.worktrees;
    const tools = props.tools || DEFAULT_TOOLS;
    const active = props.active || null;
    const onTool = props.onTool || null;
    const indexHref = props.indexHref || '../Viden - 设计稿索引 (GUI).html';

    const wm = (
      <span className="wm">
        <span style={{ color: 'var(--accent)' }}>[</span>
        <span style={{ color: 'var(--accent-bright)' }}>◉</span>
        <span style={{ color: 'var(--accent)' }}>]</span>{' '}
        <span className="r">v</span><span className="c">iden</span>
      </span>
    );

    const renderTool = (k, i) => {
      if (k === '|') return <span className="tbdiv" key={'d' + i}></span>;
      if (k === 'index') {
        return (
          <a className="tbtbtn" key="index" href={indexHref} target="_blank" rel="noopener"
             title={TITLES.index} style={{ textDecoration: 'none', fontSize: '15px' }}>▤</a>
        );
      }
      const cls = 'tbtbtn' + (active === k ? ' on' : '');
      return (
        <div className={cls} key={k} title={TITLES[k] || k}
             onClick={onTool ? () => onTool(k) : undefined}>
          {icon(k)}
        </div>
      );
    };

    return (
      <div className="titlebar">
        <div className="tl"><i className="a"></i><i className="b"></i><i className="c"></i></div>
        <div className="tbtitle">{wm}</div>
        <div className="projsel" style={props.dim ? { opacity: 0.6 } : undefined}>
          {project} {branch ? <span className="br">⎇ {branch}</span> : null} ▾
        </div>
        {git && (
          <div className="gitops">
            {sync && <span className="gitchip"><span className="up">↑{sync.up}</span><span className="down">↓{sync.down}</span> sync</span>}
            {worktrees != null && <span className="gitchip">⎇ {worktrees} worktrees</span>}
          </div>
        )}
        {props.children}
        <div className="tbtools">
          {tools.map(renderTool)}
        </div>
      </div>
    );
  }

  window.GuiTitleBar = GuiTitleBar;

  /* popout 独立窗口顶栏（.winbar · 度量与 .titlebar 同源 gui-kit）
     props: title(可省 · 窗口题字 .tt) · children(右侧插槽：pin/pop 等页专属控件) */
  function GuiPopoutBar(props) {
    return (
      <div className="winbar">
        <span className="dots"><i></i><i></i><i></i></span>
        <span className="wm">
          <span style={{ color: 'var(--accent)' }}>[</span>
          <span style={{ color: 'var(--accent-bright)' }}>◉</span>
          <span style={{ color: 'var(--accent)' }}>]</span>{' '}
          <span className="r">v</span><span className="c">iden</span>
        </span>
        {props.title != null && <span className="tt">{props.title}</span>}
        <span className="sp"></span>
        {props.children}
      </div>
    );
  }
  window.GuiPopoutBar = GuiPopoutBar;
})();
