import { useCallback, useEffect, useRef, useState } from 'react';
import { AccountCard } from '../components/AccountCard';
import { Icon } from '../components/Icon';
import appIcon from '../assets/app-icon.png';
import { api, appError } from '../services/tauri';
import type { AccountAction, AppError, CodexAccount, RuntimeStatus } from '../types/account';

export function AccountsPage() {
  const [accounts, setAccounts] = useState<CodexAccount[]>([]);
  const [runtimes, setRuntimes] = useState<RuntimeStatus[]>([]);
  const [errors, setErrors] = useState<Record<string, AppError>>({});
  const [error, setError] = useState<AppError | null>(null);
  const [notice, setNotice] = useState('');
  const [busy, setBusy] = useState(false);
  const [login, setLogin] = useState(false);
  const [loading, setLoading] = useState(true);
  const [now, setNow] = useState(Date.now);
  const [query, setQuery] = useState('');
  const [deleteCandidate, setDeleteCandidate] = useState<CodexAccount | null>(null);
  const running = useRef(false);
  const generation = useRef(0);
  const reload = useCallback(async () => {
    const version = ++generation.current;
    const [a, r] = await Promise.all([api.list(), api.status()]);
    if (version === generation.current) { setAccounts(a); setRuntimes(r); }
  }, []);
  useEffect(() => {
    let alive = true;
    reload().catch(e => { if (alive) setError(appError(e)); }).finally(() => { if (alive) setLoading(false); });
    const timer = window.setInterval(() => setNow(Date.now()), 30_000);
    const focus = () => { if (!running.current) void reload().catch(e => setError(appError(e))); };
    window.addEventListener('focus', focus);
    return () => { alive = false; clearInterval(timer); window.removeEventListener('focus', focus); };
  }, [reload]);

  const execute = useCallback(async (task: () => Promise<void>) => {
    if (running.current) return;
    running.current = true; setBusy(true); setError(null); setNotice('');
    try { await task(); } catch (e) { setError(appError(e)); }
    finally {
      // Also reload on partial-switch errors: disk, not optimistic UI, is authoritative.
      try { await reload(); } catch (e) { setError(appError(e)); }
      running.current = false; setBusy(false);
    }
  }, [reload]);
  const doLogin = useCallback(async (id?: string) => {
    setLogin(true);
    try {
      const result = await api.login(id);
      if (result.quotaError) setErrors(prev => ({ ...prev, [result.account.id]: result.quotaError! }));
      setNotice(id ? '重新授权完成。运行环境认证文件未自动更改，请按需切换。' : '账号已安全保存。');
    } finally { setLogin(false); }
  }, []);
  const action = useCallback((kind: AccountAction, account: CodexAccount) => {
    if (kind === 'delete') { setDeleteCandidate(account); return; }
    void execute(async () => {
      setErrors(prev => { const next = { ...prev }; delete next[account.id]; return next; });
      try {
        switch (kind) {
          case 'refresh': await api.refresh(account.id); break;
          case 'codex': await api.switchCodex(account.id); setNotice('Codex 认证文件已切换；已运行的 CLI 可能需要重启。'); break;
          case 'pi': await api.switchPi(account.id); setNotice('Pi Agent 认证文件已切换；已有会话请重启。'); break;
          case 'both': await api.switchBoth(account.id); setNotice('Codex 和 Pi Agent 认证文件均已切换。'); break;
          case 'reauthorize': await doLogin(account.id); break;
        }
      } catch (e) { setErrors(prev => ({ ...prev, [account.id]: appError(e) })); throw e; }
    });
  }, [execute, doLogin]);
  const refreshAll = () => void execute(async () => {
    const results = await api.refreshAll(); const next: Record<string, AppError> = {};
    for (const r of results) if (r.error) next[r.accountId] = r.error;
    setErrors(next); setNotice(`已刷新 ${results.length - Object.keys(next).length} / ${results.length} 个账号额度。`);
  });
  const visible = accounts.filter(a => `${a.email ?? ''} ${a.accountId} ${a.planType ?? ''}`.toLowerCase().includes(query.toLowerCase()));
  return <div className="app-shell">
    <aside className="sidebar">
      <a className="brand" href="#" aria-label="AI Pool 首页"><img src={appIcon} alt="" width="48" height="48"/><span>AI Pool<small>你的本地账号工作台</small></span></a>
      <div className="nav-caption">工作空间</div>
      <div className="nav-active" aria-current="page"><Icon name="grid" size={18}/>账号管理 <small>{accounts.length}</small></div>
      <div className="sidebar-bottom"><div className="security-note"><Icon name="shield" size={20}/><strong>本地明文保存</strong><p>Token 保存在本机文件<br/>请勿分享或上传凭据库</p></div><div className="sidebar-version"><span>AI Pool <small>v0.1.0</small></span><Icon name="sun"/></div></div>
    </aside>
    <main>
      <div className="topbar"><div className="eyebrow">工作空间 <span>/</span> <strong>账号管理</strong></div><span className="local-indicator"><span className="status-dot"/>本地工作台</span></div>
      <header className="page-header"><div><h1>Codex Accounts</h1><p>管理你的所有账号，让额度和工作状态一目了然。</p></div><button className="primary" disabled={busy} onClick={() => void execute(() => doLogin())}><Icon name="plus"/>添加账号</button></header>
      <div className="section-heading"><h2>运行环境</h2><span>独立切换，互不影响</span></div>
      <section className="runtime-grid" aria-label="当前运行环境">
        {runtimes.map((r, index) => <div className="runtime" key={r.runtime}><div className="runtime-title"><span className="runtime-icon"><Icon name={index === 0 ? 'terminal' : 'agent'} size={20}/></span>{r.runtime}<small className={r.managedId ? 'connected' : ''}>{r.managedId ? '已连接' : '当前环境'}</small></div><strong>{r.error ? '无法读取认证文件' : r.managedId ? accounts.find(a => a.id === r.managedId)?.email ?? r.accountId : r.accountId ? '检测到未管理的账号' : '未检测到 OAuth 账号'}</strong><div className="runtime-path" title={r.path}>{r.path}</div>{r.error ? <p className="card-error">{r.error.message}</p> : r.accountId && !r.managedId ? <button disabled={busy} onClick={() => void execute(async () => { const a = await (index === 0 ? api.importCodex() : api.importPi()); try { await api.refresh(a.id); } catch (e) { setErrors(prev => ({ ...prev, [a.id]: appError(e) })); } setNotice('当前账号已导入。'); })}>导入当前账号 ↗</button> : null}</div>)}
      </section>
      {login ? <div className="banner login" role="status"><div><strong>请在系统浏览器中完成 OpenAI 登录</strong><p>等待本地 OAuth 回调 · 3 分钟后自动超时</p></div><button onClick={() => void api.cancel().catch(e => setError(appError(e)))}>取消登录</button></div> : null}
      {error ? <div className="banner error" role="alert"><div><strong>{error.message}</strong><p><code>{error.code}</code>{error.detail ? ` · ${error.detail}` : ''}</p></div><button aria-label="关闭错误" onClick={() => setError(null)}>×</button></div> : null}
      {notice ? <div className="notice" role="status">✓ {notice}</div> : null}
      <div className="toolbar"><div className="section-title">所有账号 <span>{accounts.length}</span></div><div className="toolbar-actions"><label className="search-field"><Icon name="search"/><input aria-label="搜索账号" placeholder="搜索账号…" value={query} onChange={e => setQuery(e.target.value)} /></label><button className="quiet" disabled={busy} onClick={() => void execute(async () => { await reload(); })}>识别状态</button><button disabled={busy || !accounts.length} onClick={refreshAll}><Icon name="refresh" className={busy ? 'spinning' : undefined}/>{busy ? '处理中…' : '刷新全部'}</button></div></div>
      {loading ? <div className="empty">正在连接本地凭据库…</div> : visible.length ? <div className="accounts-grid">{visible.map(a => <AccountCard key={a.id} account={a} codex={runtimes[0]?.managedId === a.id} pi={runtimes[1]?.managedId === a.id} busy={busy} error={errors[a.id]} now={now} onAction={action} />)}</div> : <div className="empty"><div className="empty-art"><span/><div className="empty-icon"><Icon name={query ? 'search' : 'grid'} size={30}/></div><span/></div><h2>{query ? '没有匹配的账号' : '从你的第一个账号开始'}</h2><p>{query ? '尝试搜索其他 Email 或账号 ID。' : '添加 Codex OAuth 账号，或导入上方检测到的本地账号。'}</p>{!query ? <button className="primary" disabled={busy} onClick={() => void execute(() => doLogin())}><Icon name="plus"/>添加 Codex 账号</button> : null}</div>}
      <footer className="page-footer"><span><Icon name="shield" size={14}/>凭据仅在本机处理</span><span>额度为缓存快照 · 切换前请暂停运行中的 Agent</span></footer>
    </main>
    {deleteCandidate ? <div className="modal-overlay"><section className="modal" role="dialog" aria-modal="true" aria-labelledby="delete-title" onKeyDown={e => {
      if (e.key === 'Escape') setDeleteCandidate(null);
      if (e.key === 'Tab') { const buttons = e.currentTarget.querySelectorAll('button'); const first = buttons[0]; const last = buttons[buttons.length - 1]; if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); } else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); } }
    }}><h2 id="delete-title">删除这个账号？</h2><strong>{deleteCandidate.email ?? deleteCandidate.accountId}</strong><p>管理器中的账号和凭据将清除。Codex / Pi 的认证文件不会删除，也不会远程撤销授权；当前账号将变为未管理状态。</p><div><button autoFocus onClick={() => setDeleteCandidate(null)}>取消</button><button className="danger" onClick={() => { const id = deleteCandidate.id; setDeleteCandidate(null); void execute(async () => { await api.remove(id); setNotice('账号已从本地凭据库删除。'); }); }}>确认删除</button></div></section></div> : null}
  </div>;
}
