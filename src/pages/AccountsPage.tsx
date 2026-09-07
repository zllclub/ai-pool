import { useCallback, useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import Button from '@arco-design/web-react/es/Button';
import Card from '@arco-design/web-react/es/Card';
import InputNumber from '@arco-design/web-react/es/InputNumber';
import Popover from '@arco-design/web-react/es/Popover';
import Modal from '@arco-design/web-react/es/Modal';
import Alert from '@arco-design/web-react/es/Alert';
import Spin from '@arco-design/web-react/es/Spin';
import Empty from '@arco-design/web-react/es/Empty';
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
  const [refreshing, setRefreshing] = useState<string | 'all' | null>(null);
  const [toast, setToast] = useState('');
  const [pinnedAccounts, setPinnedAccounts] = useState<Set<string>>(() => new Set());
  const [login, setLogin] = useState(false);
  const [loading, setLoading] = useState(true);
  const [startupRefreshed, setStartupRefreshed] = useState(false);
  const [now, setNow] = useState(Date.now);
  const [refreshMinutes, setRefreshMinutes] = useState(() => {
    const saved = Number(window.localStorage.getItem('quota-refresh-minutes'));
    return Number.isInteger(saved) && saved >= 1 && saved <= 999 ? saved : 5;
  });
  const [deleteCandidate, setDeleteCandidate] = useState<CodexAccount | null>(null);
  const running = useRef(false);
  const startupRefreshStarted = useRef(false);
  const generation = useRef(0);
  const reload = useCallback(async () => {
    const version = ++generation.current;
    const [a, r] = await Promise.all([api.list(), api.status()]);
    if (version === generation.current) { setAccounts(a); setRuntimes(r); }
  }, []);
  useEffect(() => {
    if (!toast) return;
    const timer = window.setTimeout(() => setToast(''), 2200);
    return () => window.clearTimeout(timer);
  }, [toast]);
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
          case 'refresh':
            setRefreshing(account.id);
            try { await api.refresh(account.id); setToast(`${account.email ?? account.accountId} 的额度已刷新`); }
            finally { setRefreshing(null); }
            break;
          case 'codex': await api.switchCodex(account.id); setNotice('Codex 认证文件已切换；已运行的 CLI 可能需要重启。'); break;
          case 'pi': await api.switchPi(account.id); setNotice('Pi Agent 认证文件已切换；已有会话请重启。'); break;
          case 'reauthorize': await doLogin(account.id); break;
        }
      } catch (e) { setErrors(prev => ({ ...prev, [account.id]: appError(e) })); throw e; }
    });
  }, [execute, doLogin]);
  const refreshAll = useCallback((showNotice = true) => {
    return execute(async () => {
      setRefreshing('all');
      try {
        const results = await api.refreshAll(); const next: Record<string, AppError> = {};
        for (const r of results) if (r.error) next[r.accountId] = r.error;
        setErrors(next);
        if (showNotice) setToast(`已刷新 ${results.length - Object.keys(next).length} / ${results.length} 个账号额度`);
      } finally { setRefreshing(null); }
    });
  }, [execute]);
  useEffect(() => {
    window.localStorage.setItem('quota-refresh-minutes', String(refreshMinutes));
  }, [refreshMinutes]);
  useEffect(() => {
    if (loading || startupRefreshStarted.current) return;
    startupRefreshStarted.current = true;
    if (accounts.length === 0) { setStartupRefreshed(true); return; }
    void refreshAll(false).finally(() => setStartupRefreshed(true));
  }, [accounts.length, loading, refreshAll]);
  const runScheduledRefresh = useCallback(async (ids: string[]) => {
    if (running.current || ids.length === 0) return;
    running.current = true;
    try {
      const results = await Promise.all(ids.map(async id => {
        try { await api.refresh(id); return { id, error: null }; }
        catch (error) { return { id, error: appError(error) }; }
      }));
      setErrors(previous => {
        const next = { ...previous };
        for (const result of results) result.error ? next[result.id] = result.error : delete next[result.id];
        return next;
      });
      await reload();
    } finally { running.current = false; }
  }, [reload]);
  const accountIds = accounts.map(account => account.id).sort().join(',');
  const activeIds = [...new Set(runtimes.map(runtime => runtime.managedId).filter((id): id is string => Boolean(id)))].sort().join(',');
  useEffect(() => {
    if (!startupRefreshed || !accountIds) return;
    const all = accountIds.split(',');
    const active = new Set(activeIds ? activeIds.split(',') : []);
    const current = all.filter(id => active.has(id));
    const standby = all.filter(id => !active.has(id));
    const activeInterval = refreshMinutes * 60_000;
    const standbyInterval = 60 * 60_000;
    let nextActive = Date.now() + activeInterval;
    let nextStandby = Date.now() + standbyInterval;
    const tick = () => {
      const now = Date.now();
      const due = new Set<string>();
      if (now >= nextActive) { current.forEach(id => due.add(id)); nextActive = now + activeInterval; }
      if (now >= nextStandby) { standby.forEach(id => due.add(id)); nextStandby = now + standbyInterval; }
      if (due.size) void runScheduledRefresh([...due]);
    };
    const timer = window.setInterval(tick, Math.min(30_000, activeInterval));
    window.addEventListener('focus', tick);
    document.addEventListener('visibilitychange', tick);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener('focus', tick);
      document.removeEventListener('visibilitychange', tick);
    };
  }, [accountIds, activeIds, refreshMinutes, runScheduledRefresh, startupRefreshed]);
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<string>('widget-closed', event => {
      setPinnedAccounts(current => { const next = new Set(current); next.delete(event.payload); return next; });
    }).then(dispose => { unlisten = dispose; });
    return () => unlisten?.();
  }, []);
  const showWidget = useCallback((account: CodexAccount) => {
    void api.toggleWidget(account.id)
      .then(pinned => {
        setPinnedAccounts(current => { const next = new Set(current); if (pinned) next.add(account.id); else next.delete(account.id); return next; });
        setToast(`${account.email ?? account.accountId} 已${pinned ? '开启' : '关闭'}悬浮显示`);
      })
      .catch(e => setError(appError(e)));
  }, []);
  return <div className="app-shell">
    <aside className="sidebar">
      <a className="brand" href="#" aria-label="AI Pool 首页"><img src={appIcon} alt="" width="48" height="48"/><span>AIPool</span></a>
      <div className="nav-caption">工作空间</div>
      <div className="nav-active" aria-current="page"><Icon name="grid" size={18}/>账号管理</div>
      <div className="sidebar-bottom"><div className="security-note"><Icon name="shield" size={20}/><strong>本地管理</strong></div><div className="sidebar-version"><span>AIPool <small>v0.1.0</small></span></div></div>
    </aside>
    <main>
      <header className="page-header"><div><h1>账号管理</h1><p>Codex Accounts</p></div><Button type="primary" disabled={busy} onClick={() => void execute(() => doLogin())}><Icon name="plus"/>添加账号</Button></header>
      <section className="runtime-grid" aria-label="当前运行环境">
        {runtimes.map((r, index) => <Card className="runtime" key={r.runtime}><div className="runtime-title"><span className="runtime-icon"><Icon name={index === 0 ? 'terminal' : 'agent'} size={20}/></span>{r.runtime}<small className={r.managedId ? 'connected' : ''}>{r.managedId ? '已连接' : '当前环境'}</small></div><strong>{r.error ? '无法读取认证文件' : r.managedId ? accounts.find(a => a.id === r.managedId)?.email ?? r.accountId : r.accountId ? '检测到未管理的账号' : '未检测到 OAuth 账号'}</strong><div className="runtime-path" title={r.path}>{r.path}</div>{r.error ? <p className="card-error">{r.error.message}</p> : r.accountId && !r.managedId ? <Button disabled={busy} onClick={() => void execute(async () => { const a = await (index === 0 ? api.importCodex() : api.importPi()); try { await api.refresh(a.id); } catch (e) { setErrors(prev => ({ ...prev, [a.id]: appError(e) })); } setNotice('当前账号已导入。'); })}>导入当前账号 <Icon name="external" size={13}/></Button> : null}</Card>)}
      </section>
      {login ? <Alert className="page-alert" type="info" title="请在系统浏览器中完成 OpenAI 登录" content="等待本地 OAuth 回调 · 3 分钟后自动超时" action={<Button size="small" onClick={() => void api.cancel().catch(e => setError(appError(e)))}>取消登录</Button>}/> : null}
      {error ? <Alert className="page-alert" type="error" title={error.message} content={<><code>{error.code}</code>{error.detail ? ` · ${error.detail}` : ''}</>} closable onClose={() => setError(null)}/> : null}
      {notice ? <Alert className="page-alert" type="success" content={notice} closable onClose={() => setNotice('')}/> : null}
      <div className="toolbar"><div className="section-title">所有账号 <span>{accounts.length}</span></div><div className="toolbar-actions"><Popover trigger="click" position="br" title="自动刷新额度" content={<div className="refresh-frequency-popover"><span>刷新间隔</span><InputNumber aria-label="额度自动刷新间隔（分钟）" min={1} max={999} precision={0} value={refreshMinutes} onChange={value => setRefreshMinutes(Math.min(999, Math.max(1, value || 1)))}/><span>分钟</span><small>允许设置 1–999 分钟</small></div>}><Button className="refresh-settings" type="default" aria-label={`自动刷新设置，当前 ${refreshMinutes} 分钟`} title="修改自动刷新频率" icon={<Icon name="settings" size={14}/>}><span>自动刷新</span><strong>{refreshMinutes} 分钟</strong></Button></Popover><Button type="default" loading={refreshing === 'all'} disabled={busy || !accounts.length} onClick={() => void refreshAll()} icon={<Icon name="refresh"/>}>刷新全部</Button></div></div>
      {loading ? <div className="empty"><Spin tip="正在连接本地凭据库…"/></div> : accounts.length ? <div className="accounts-grid">{accounts.map(a => <AccountCard key={a.id} account={a} codex={runtimes[0]?.managedId === a.id} pi={runtimes[1]?.managedId === a.id} busy={busy && (refreshing === null || refreshing === 'all' || refreshing === a.id)} refreshing={refreshing === a.id} pinned={pinnedAccounts.has(a.id)} error={errors[a.id]} now={now} onAction={action} onPin={showWidget} />)}</div> : <div className="empty"><Empty icon={<Icon name="grid" size={42}/>} description={<><h2>从你的第一个账号开始</h2><p>添加 Codex OAuth 账号，或导入上方检测到的本地账号。</p></>}/><Button type="primary" disabled={busy} onClick={() => void execute(() => doLogin())} icon={<Icon name="plus"/>}>添加 Codex 账号</Button></div>}
      <footer className="page-footer"><span><Icon name="shield" size={14}/>凭据仅在本机处理</span><span>额度为缓存快照 · 切换前请暂停运行中的 Agent</span></footer>
    </main>
    {toast ? <div className="light-toast" role="status"><Icon name="check" size={15}/><span>{toast}</span></div> : null}
    <Modal title="删除这个账号？" visible={deleteCandidate !== null} onCancel={() => setDeleteCandidate(null)}
      maskClosable={false} focusLock autoFocus escToExit okText="确认删除" cancelText="取消" okButtonProps={{ status: 'danger' }}
      onOk={() => { if (!deleteCandidate) return; const id = deleteCandidate.id; setDeleteCandidate(null); void execute(async () => { await api.remove(id); setNotice('账号已从本地凭据库删除。'); }); }}>
      <div className="delete-confirmation"><strong>{deleteCandidate?.email ?? deleteCandidate?.accountId}</strong><p>管理器中的账号和凭据将清除。Codex / Pi 的认证文件不会删除，也不会远程撤销授权；当前账号将变为未管理状态。</p></div>
    </Modal>
  </div>;
}
