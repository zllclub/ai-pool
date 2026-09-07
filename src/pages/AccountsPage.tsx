import { useCallback, useEffect, useRef, useState } from 'react';
import { AccountCard } from '../components/AccountCard';
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
    if (kind === 'delete' && !window.confirm(`删除 ${account.email ?? account.accountId}？\n账号库中的凭据将清除。Codex / Pi 已写入的认证不删除、不远程撤销；当前账号会变为未管理状态。`)) return;
    void execute(async () => {
      setErrors(prev => { const next = { ...prev }; delete next[account.id]; return next; });
      try {
        switch (kind) {
          case 'refresh': await api.refresh(account.id); break;
          case 'codex': await api.switchCodex(account.id); setNotice('Codex 认证文件已切换；已运行的 CLI 可能需要重启。'); break;
          case 'pi': await api.switchPi(account.id); setNotice('Pi Agent 认证文件已切换；已有会话请重启。'); break;
          case 'both': await api.switchBoth(account.id); setNotice('Codex 和 Pi Agent 认证文件均已切换。'); break;
          case 'reauthorize': await doLogin(account.id); break;
          case 'delete': await api.remove(account.id); setNotice('账号已从加密凭据库删除。'); break;
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
    <aside className="sidebar"><a className="brand" href="#"><span className="brand-icon">c_</span><span>CODEX<br/><small>ACCOUNT POOL</small></span></a><div className="nav-caption">WORKSPACE</div><div className="nav-active"><span>▦</span> 账号管理 <small>{accounts.length}</small></div><div className="sidebar-bottom"><span className="status-dot" /> LOCAL FIRST<p>凭据由系统密钥保护<br/>不经过第三方服务器</p><span className="version">DESKTOP / v0.1.0</span></div></aside>
    <main>
      <div className="eyebrow">WORKSPACE <span>/</span> ACCOUNTS</div>
      <header className="page-header"><div><h1>Codex Accounts<span className="count">{accounts.length}</span></h1><p>多账号，一个工作台。额度独立查询，环境自由切换。</p></div><button className="primary" disabled={busy} onClick={() => void execute(() => doLogin())}>＋ 添加账号</button></header>
      <section className="runtime-grid" aria-label="当前运行环境">
        {runtimes.map((r, index) => <div className="runtime" key={r.runtime}><div className="runtime-title"><span>{index === 0 ? '›_' : 'π'}</span>{r.runtime}<small>CURRENT</small></div><strong>{r.error ? '无法读取认证文件' : r.managedId ? accounts.find(a => a.id === r.managedId)?.email ?? r.accountId : r.accountId ? '检测到未管理的账号' : '未检测到 OAuth 账号'}</strong><div className="runtime-path" title={r.path}>{r.path}</div>{r.error ? <p className="card-error">{r.error.message}</p> : r.accountId && !r.managedId ? <button disabled={busy} onClick={() => void execute(async () => { const a = await (index === 0 ? api.importCodex() : api.importPi()); try { await api.refresh(a.id); } catch (e) { setErrors(prev => ({ ...prev, [a.id]: appError(e) })); } setNotice('当前账号已导入。'); })}>导入当前账号 ↗</button> : null}</div>)}
      </section>
      {login ? <div className="banner login" role="status"><div><strong>请在系统浏览器中完成 OpenAI 登录</strong><p>等待本地 OAuth 回调 · 3 分钟后自动超时</p></div><button onClick={() => void api.cancel().catch(e => setError(appError(e)))}>取消登录</button></div> : null}
      {error ? <div className="banner error" role="alert"><div><strong>{error.message}</strong><p><code>{error.code}</code>{error.detail ? ` · ${error.detail}` : ''}</p></div><button aria-label="关闭错误" onClick={() => setError(null)}>×</button></div> : null}
      {notice ? <div className="notice" role="status">✓ {notice}</div> : null}
      <div className="toolbar"><div className="section-title">所有账号 <span>{accounts.length.toString().padStart(2, '0')}</span></div><div className="toolbar-actions"><input aria-label="搜索账号" placeholder="搜索账号…" value={query} onChange={e => setQuery(e.target.value)} /><button disabled={busy} onClick={() => void execute(async () => { await reload(); })}>识别状态</button><button disabled={busy || !accounts.length} onClick={refreshAll}>↻ {busy ? '处理中…' : '刷新全部额度'}</button></div></div>
      {loading ? <div className="empty">正在连接本地凭据库…</div> : visible.length ? <div className="accounts-grid">{visible.map(a => <AccountCard key={a.id} account={a} codex={runtimes[0]?.managedId === a.id} pi={runtimes[1]?.managedId === a.id} busy={busy} error={errors[a.id]} now={now} onAction={action} />)}</div> : <div className="empty"><div className="empty-icon">＋</div><h2>{query ? '没有匹配的账号' : '让每个账号，各就其位。'}</h2><p>{query ? '尝试搜索其他 Email 或账号 ID。' : '添加 Codex OAuth 账号，或导入上方检测到的本地账号。'}</p>{!query ? <button disabled={busy} onClick={() => void execute(() => doLogin())}>添加第一个账号 ↗</button> : null}</div>}
      <footer className="page-footer"><span>⌁ Token 仅在 Rust 后端处理</span><span>额度为缓存快照 · 切换前建议暂停运行中的 Agent</span></footer>
    </main>
  </div>;
}
