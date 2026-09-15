import { useCallback, useEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react';
import { listen } from '@tauri-apps/api/event';
import Button from '@arco-design/web-react/es/Button';
import Card from '@arco-design/web-react/es/Card';
import InputNumber from '@arco-design/web-react/es/InputNumber';
import Popover from '@arco-design/web-react/es/Popover';
import Switch from '@arco-design/web-react/es/Switch';
import Modal from '@arco-design/web-react/es/Modal';
import Alert from '@arco-design/web-react/es/Alert';
import Spin from '@arco-design/web-react/es/Spin';
import Empty from '@arco-design/web-react/es/Empty';
import { AccountCard } from '../components/AccountCard';
import { Icon } from '../components/Icon';
import { RequestLogsPage } from './RequestLogsPage';
import appIcon from '../assets/app-icon.png';
import { api, appError } from '../services/tauri';
import { accountRefreshMinutes, fiveHourRemaining, isEligiblePiSuccessor, isLowQuotaRefreshActive, recordRefreshResult, shouldAutoSwitchPi, weeklyRemaining, REFRESH_POLICY_EVENT, REFRESH_POLICY_KEY } from '../services/refreshPolicy';
import type { AccountAction, AppError, CodexAccount, QuotaRequestReason, RuntimeStatus } from '../types/account';

const ACCOUNT_ORDER_KEY = 'account-card-order.v1';
const AUTO_SWITCH_PI_KEY = 'auto-switch-pi.v1';
const AUTO_SWITCH_QUOTA_MAX_AGE = 2 * 60_000;

function storedAutoSwitchPi() {
  return window.localStorage.getItem(AUTO_SWITCH_PI_KEY) === 'true';
}

function storedAccountOrder() {
  try {
    const value: unknown = JSON.parse(window.localStorage.getItem(ACCOUNT_ORDER_KEY) ?? '[]');
    return Array.isArray(value) ? [...new Set(value.filter((id): id is string => typeof id === 'string'))] : [];
  } catch { return []; }
}

function orderAccounts(accounts: CodexAccount[], order: string[]) {
  const byId = new Map(accounts.map(account => [account.id, account]));
  const result: CodexAccount[] = [];
  for (const id of order) {
    const account = byId.get(id);
    if (account) { result.push(account); byId.delete(id); }
  }
  result.push(...byId.values());
  return result;
}

export function AccountsPage() {
  const [section, setSection] = useState<'accounts' | 'logs'>('accounts');
  const [accounts, setAccounts] = useState<CodexAccount[]>([]);
  const [runtimes, setRuntimes] = useState<RuntimeStatus[]>([]);
  const [errors, setErrors] = useState<Record<string, AppError>>({});
  const [error, setError] = useState<AppError | null>(null);
  const [busy, setBusy] = useState(false);
  const [refreshing, setRefreshing] = useState<string | 'all' | null>(null);
  const [refreshPolicyVersion, setRefreshPolicyVersion] = useState(0);
  const [autoSwitchPi, setAutoSwitchPi] = useState(storedAutoSwitchPi);
  const [autoSwitching, setAutoSwitching] = useState(false);
  const [toast, setToast] = useState('');
  const [pinnedAccounts, setPinnedAccounts] = useState<Set<string>>(() => new Set());
  const [accountOrder, setAccountOrder] = useState<string[]>(storedAccountOrder);
  const [draggingAccount, setDraggingAccount] = useState<string | null>(null);
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
  const widgetStateVersion = useRef(0);
  const pointerDragCleanupRef = useRef<(() => void) | null>(null);
  const autoSwitchRunningRef = useRef(false);
  const autoSwitchEnabledRef = useRef(autoSwitchPi);
  const autoSwitchEvaluatedRef = useRef('');
  const noSuccessorNoticeAtRef = useRef(0);
  const accountsRef = useRef(accounts);
  accountsRef.current = accounts;
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
    running.current = true; setBusy(true); setError(null); setToast('');
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
      setToast(id ? '重新授权完成，运行环境认证文件未自动更改' : '账号已安全保存');
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
            try {
              const updated = await api.refresh(account.id, 'manual');
              recordRefreshResult(account, updated);
              setToast(`${account.email ?? account.accountId} 的额度已刷新`);
            } finally { setRefreshing(null); }
            break;
          case 'codex': await api.switchCodex(account.id); setToast('Codex 认证文件已切换，已运行的 CLI 可能需要重启'); break;
          case 'pi': await api.switchPi(account.id); setToast('Pi Agent 认证文件已切换'); break;
          case 'reauthorize': await doLogin(account.id); break;
        }
      } catch (e) { setErrors(prev => ({ ...prev, [account.id]: appError(e) })); throw e; }
    });
  }, [execute, doLogin]);
  const refreshAll = useCallback((showNotice = true) => {
    return execute(async () => {
      setRefreshing('all');
      try {
        const reason: QuotaRequestReason = showNotice ? 'manualAll' : 'appStartup';
        const targets = accountsRef.current;
        const results = await Promise.all(targets.map(async account => {
          try {
            const updated = await api.refresh(account.id, reason);
            recordRefreshResult(account, updated);
            return { accountId: account.id, error: null };
          } catch (error) { return { accountId: account.id, error: appError(error) }; }
        }));
        const next: Record<string, AppError> = {};
        for (const result of results) if (result.error) next[result.accountId] = result.error;
        setErrors(next);
        if (showNotice) setToast(`已刷新 ${results.length - Object.keys(next).length} / ${results.length} 个账号额度`);
      } finally { setRefreshing(null); }
    });
  }, [execute]);
  useEffect(() => {
    window.localStorage.setItem('quota-refresh-minutes', String(refreshMinutes));
  }, [refreshMinutes]);
  useEffect(() => {
    autoSwitchEnabledRef.current = autoSwitchPi;
    window.localStorage.setItem(AUTO_SWITCH_PI_KEY, String(autoSwitchPi));
  }, [autoSwitchPi]);
  useEffect(() => {
    const changed = () => setRefreshPolicyVersion(version => version + 1);
    const storage = (event: StorageEvent) => { if (event.key === REFRESH_POLICY_KEY) changed(); };
    window.addEventListener(REFRESH_POLICY_EVENT, changed);
    window.addEventListener('storage', storage);
    return () => {
      window.removeEventListener(REFRESH_POLICY_EVENT, changed);
      window.removeEventListener('storage', storage);
    };
  }, []);
  useEffect(() => {
    if (loading || startupRefreshStarted.current) return;
    startupRefreshStarted.current = true;
    if (accounts.length === 0) { setStartupRefreshed(true); return; }
    void refreshAll(false).finally(() => setStartupRefreshed(true));
  }, [accounts.length, loading, refreshAll]);
  const runScheduledRefresh = useCallback(async (requests: { id: string; reason: QuotaRequestReason }[]) => {
    if (running.current || requests.length === 0) return;
    running.current = true;
    try {
      const results = await Promise.all(requests.map(async ({ id, reason }) => {
        try {
          const before = accountsRef.current.find(account => account.id === id);
          const updated = await api.refresh(id, reason);
          recordRefreshResult(before, updated);
          return { id, error: null };
        } catch (error) { return { id, error: appError(error) }; }
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
  // Include quota snapshots so a low-quota schedule is rebuilt immediately after a refresh
  // changes the 5H value. Depending on IDs alone leaves the old one-minute plan running.
  const refreshPlanKey = accounts
    .map(account => `${account.id}:${account.quota?.updatedAt ?? 0}:${fiveHourRemaining(account) ?? 'none'}`)
    .sort()
    .join(',');
  const activeIds = [...new Set(runtimes.map(runtime => runtime.managedId).filter((id): id is string => Boolean(id)))].sort().join(',');
  useEffect(() => {
    if (!startupRefreshed || !accountIds) return;
    const active = new Set(activeIds ? activeIds.split(',') : []);
    const plan = accounts.map(account => ({
      id: account.id,
      interval: accountRefreshMinutes(account, active.has(account.id), refreshMinutes) * 60_000,
      reason: (isLowQuotaRefreshActive(account, active.has(account.id)) ? 'lowQuotaAutomatic' : 'automatic') as QuotaRequestReason,
    }));
    const nextRefresh = new Map(plan.map(item => [item.id, Date.now() + item.interval]));
    const tick = () => {
      if (running.current) return;
      const now = Date.now();
      const due: { id: string; reason: QuotaRequestReason }[] = [];
      for (const item of plan) {
        if (now < (nextRefresh.get(item.id) ?? 0)) continue;
        due.push({ id: item.id, reason: item.reason });
        nextRefresh.set(item.id, now + item.interval);
      }
      if (due.length) void runScheduledRefresh(due);
    };
    const shortestInterval = Math.min(...plan.map(item => item.interval));
    const timer = window.setInterval(tick, Math.min(30_000, shortestInterval));
    window.addEventListener('focus', tick);
    document.addEventListener('visibilitychange', tick);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener('focus', tick);
      document.removeEventListener('visibilitychange', tick);
    };
  }, [accountIds, activeIds, refreshMinutes, refreshPlanKey, refreshPolicyVersion, runScheduledRefresh, startupRefreshed]);
  useEffect(() => {
    let alive = true;
    let unlistenClosed: (() => void) | undefined;
    let unlistenData: (() => void) | undefined;
    const initialWidgetStateVersion = widgetStateVersion.current;
    void api.listOpenWidgets()
      .then(ids => {
        if (alive && initialWidgetStateVersion === widgetStateVersion.current) setPinnedAccounts(new Set(ids));
      })
      .catch(e => { if (alive) setError(appError(e)); });
    void listen<string>('widget-closed', event => {
      widgetStateVersion.current += 1;
      setPinnedAccounts(current => { const next = new Set(current); next.delete(event.payload); return next; });
    }).then(dispose => { unlistenClosed = dispose; });
    void listen<string | null>('account-data-changed', () => {
      void reload().catch(e => setError(appError(e)));
    }).then(dispose => { unlistenData = dispose; });
    return () => { alive = false; unlistenClosed?.(); unlistenData?.(); };
  }, [reload]);
  const showWidget = useCallback((account: CodexAccount) => {
    const version = ++widgetStateVersion.current;
    void api.toggleWidget(account.id)
      .then(pinned => {
        if (version !== widgetStateVersion.current) return;
        setPinnedAccounts(current => { const next = new Set(current); if (pinned) next.add(account.id); else next.delete(account.id); return next; });
        setToast(`${account.email ?? account.accountId} 已${pinned ? '开启' : '关闭'}悬浮显示`);
      })
      .catch(e => setError(appError(e)));
  }, []);
  const orderedAccounts = useMemo(() => orderAccounts(accounts, accountOrder), [accountOrder, accounts]);
  useEffect(() => {
    if (!autoSwitchPi || !startupRefreshed || autoSwitchRunningRef.current) return;
    const currentId = runtimes.find(runtime => runtime.runtime === 'Pi Agent')?.managedId;
    const currentIndex = orderedAccounts.findIndex(account => account.id === currentId);
    if (!currentId || currentIndex < 0) return;
    const current = orderedAccounts[currentIndex];
    if (!current.quota || Date.now() - current.quota.updatedAt > AUTO_SWITCH_QUOTA_MAX_AGE || !shouldAutoSwitchPi(current)) return;
    const evaluation = `${current.id}:${current.quota.updatedAt}`;
    if (autoSwitchEvaluatedRef.current === evaluation) return;
    autoSwitchEvaluatedRef.current = evaluation;
    autoSwitchRunningRef.current = true;
    setAutoSwitching(true);

    void (async () => {
      let successor: CodexAccount | null = null;
      for (const candidate of orderedAccounts.slice(currentIndex + 1)) {
        if (!autoSwitchEnabledRef.current) return;
        let checked = candidate;
        if (!candidate.quota || Date.now() - candidate.quota.updatedAt > AUTO_SWITCH_QUOTA_MAX_AGE) {
          try {
            checked = await api.refresh(candidate.id, 'autoSwitchCheck');
            recordRefreshResult(candidate, checked);
            setErrors(previous => { const next = { ...previous }; delete next[candidate.id]; return next; });
          } catch (error) {
            setErrors(previous => ({ ...previous, [candidate.id]: appError(error) }));
            continue;
          }
        }
        if (isEligiblePiSuccessor(checked)) { successor = checked; break; }
      }
      if (!autoSwitchEnabledRef.current) return;
      if (!successor) {
        const now = Date.now();
        if (now - noSuccessorNoticeAtRef.current > 30 * 60_000) {
          noSuccessorNoticeAtRef.current = now;
          setToast('当前账号之后没有满足额度条件的 Pi 候选账号');
        }
        return;
      }

      const latestPi = (await api.status()).find(runtime => runtime.runtime === 'Pi Agent')?.managedId;
      if (latestPi !== current.id || !autoSwitchEnabledRef.current) return;
      await api.switchPi(successor.id);
      const weekly = weeklyRemaining(successor);
      const fiveHour = fiveHourRemaining(successor);
      const quotaSummary = [weekly == null ? null : `周 ${Math.round(weekly)}%`, fiveHour == null ? null : `5H ${Math.round(fiveHour)}%`].filter(Boolean).join(' · ');
      setToast(`Pi 已自动切换至 ${successor.email ?? successor.accountId}${quotaSummary ? ` · ${quotaSummary}` : ''}`);
      await reload();
    })().catch(error => setError(appError(error))).finally(() => {
      autoSwitchRunningRef.current = false;
      setAutoSwitching(false);
    });
  }, [autoSwitchPi, orderedAccounts, reload, runtimes, startupRefreshed]);
  const startAccountDrag = useCallback((accountId: string, event: ReactPointerEvent<HTMLDivElement>) => {
    pointerDragCleanupRef.current?.();
    const source = event.currentTarget;
    const pointerId = event.pointerId;
    const startX = event.clientX;
    const startY = event.clientY;
    const sourceRect = source.getBoundingClientRect();
    const order = orderedAccounts.map(account => account.id);
    const slots = Array.from(document.querySelectorAll<HTMLElement>('.accounts-grid .account-card-sortable')).map(element => {
      const rect = element.getBoundingClientRect();
      return { id: element.dataset.accountId ?? '', x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
    });
    let currentOrder = order;
    let active = false;
    let ghost: HTMLElement | null = null;
    try { source.setPointerCapture(pointerId); } catch { /* Pointer capture is optional in older webviews. */ }

    const cleanup = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', end);
      window.removeEventListener('pointercancel', cancel);
      source.style.removeProperty('pointer-events');
      try { if (source.hasPointerCapture(pointerId)) source.releasePointerCapture(pointerId); } catch { /* The pointer may already be released. */ }
      ghost?.remove();
      document.body.classList.remove('is-sorting-accounts');
      pointerDragCleanupRef.current = null;
      setDraggingAccount(null);
    };
    const move = (pointerEvent: globalThis.PointerEvent) => {
      if (pointerEvent.pointerId !== pointerId) return;
      const x = pointerEvent.clientX - startX;
      const y = pointerEvent.clientY - startY;
      if (!active) {
        if (Math.hypot(x, y) < 5) return;
        active = true;
        ghost = source.cloneNode(true) as HTMLElement;
        ghost.className = 'account-drag-ghost';
        ghost.removeAttribute('data-account-id');
        ghost.setAttribute('aria-hidden', 'true');
        ghost.style.left = `${sourceRect.left}px`;
        ghost.style.top = `${sourceRect.top}px`;
        ghost.style.width = `${sourceRect.width}px`;
        ghost.querySelectorAll<HTMLElement>('button, [tabindex]').forEach(element => element.tabIndex = -1);
        document.body.appendChild(ghost);
        source.style.pointerEvents = 'none';
        document.body.classList.add('is-sorting-accounts');
        setDraggingAccount(accountId);
      }
      pointerEvent.preventDefault();
      if (ghost) ghost.style.transform = `translate3d(${x}px, ${y}px, 0)`;

      let nearestIndex = -1;
      let nearestDistance = Number.POSITIVE_INFINITY;
      for (let index = 0; index < slots.length; index += 1) {
        const distance = Math.hypot(pointerEvent.clientX - slots[index].x, pointerEvent.clientY - slots[index].y);
        if (distance < nearestDistance) { nearestDistance = distance; nearestIndex = index; }
      }
      if (nearestIndex < 0) return;
      const next = order.filter(id => id !== accountId);
      next.splice(Math.min(nearestIndex, next.length), 0, accountId);
      if (next.some((id, index) => id !== currentOrder[index])) {
        currentOrder = next;
        setAccountOrder(next);
      }
    };
    const end = (pointerEvent: globalThis.PointerEvent) => {
      if (pointerEvent.pointerId !== pointerId) return;
      if (active) window.localStorage.setItem(ACCOUNT_ORDER_KEY, JSON.stringify(currentOrder));
      cleanup();
    };
    const cancel = (pointerEvent: globalThis.PointerEvent) => {
      if (pointerEvent.pointerId !== pointerId) return;
      if (active) setAccountOrder(order);
      cleanup();
    };

    pointerDragCleanupRef.current = cleanup;
    window.addEventListener('pointermove', move, { passive: false });
    window.addEventListener('pointerup', end);
    window.addEventListener('pointercancel', cancel);
  }, [orderedAccounts]);
  useEffect(() => () => pointerDragCleanupRef.current?.(), []);
  return <div className="app-shell">
    <aside className="sidebar">
      <a className="brand" href="#" aria-label="AI Pool 首页"><img src={appIcon} alt="" width="48" height="48"/><span>AIPool</span></a>
      <div className="nav-caption">工作空间</div>
      <nav className="sidebar-nav" aria-label="工作空间">
        <button type="button" className={`nav-item${section === 'accounts' ? ' nav-active' : ''}`} aria-current={section === 'accounts' ? 'page' : undefined} onClick={() => setSection('accounts')}><Icon name="grid" size={18}/>账号管理</button>
        <button type="button" className={`nav-item${section === 'logs' ? ' nav-active' : ''}`} aria-current={section === 'logs' ? 'page' : undefined} onClick={() => setSection('logs')}><Icon name="logs" size={18}/>请求日志</button>
      </nav>
      <div className="sidebar-bottom"><div className="security-note"><Icon name="shield" size={20}/><strong>本地管理</strong></div><div className="sidebar-version"><span>AIPool <small>v0.1.0</small></span></div></div>
    </aside>
    <main>
      {section === 'accounts' ? <>
      <header className="page-header"><div><h1>账号管理</h1><p>Codex Accounts</p></div><Button type="primary" disabled={busy} onClick={() => void execute(() => doLogin())}><Icon name="plus"/>添加账号</Button></header>
      <section className="runtime-grid" aria-label="当前运行环境">
        {runtimes.map((r, index) => <Card className="runtime" key={r.runtime}><div className="runtime-title"><span className="runtime-icon"><Icon name={index === 0 ? 'terminal' : 'agent'} size={20}/></span>{r.runtime}<small className={r.managedId ? 'connected' : ''}>{r.managedId ? '已连接' : '当前环境'}</small></div><strong>{r.error ? '无法读取认证文件' : r.managedId ? accounts.find(a => a.id === r.managedId)?.email ?? r.accountId : r.accountId ? '检测到未管理的账号' : '未检测到 OAuth 账号'}</strong><div className="runtime-path" title={r.path}>{r.path}</div>{r.error ? <p className="card-error">{r.error.message}</p> : r.accountId && !r.managedId ? <Button disabled={busy} onClick={() => void execute(async () => { const a = await (index === 0 ? api.importCodex() : api.importPi()); try { await api.refresh(a.id, 'import'); } catch (e) { setErrors(prev => ({ ...prev, [a.id]: appError(e) })); } setToast('当前账号已导入'); })}>导入当前账号 <Icon name="external" size={13}/></Button> : null}</Card>)}
      </section>
      {login ? <Alert className="page-alert" type="info" title="请在系统浏览器中完成 OpenAI 登录" content="等待本地 OAuth 回调 · 3 分钟后自动超时" action={<Button size="small" onClick={() => void api.cancel().catch(e => setError(appError(e)))}>取消登录</Button>}/> : null}
      {error ? <Alert className="page-alert" type="error" title={error.message} content={<><code>{error.code}</code>{error.detail ? ` · ${error.detail}` : ''}</>} closable onClose={() => setError(null)}/> : null}
      <div className="toolbar"><div className="section-title">所有账号 <span>{accounts.length}</span></div><div className="toolbar-actions">
        <Popover trigger="hover" position="br" title="Pi Agent 自动切换" content={<div className="auto-switch-help"><p>周额度低于 3% 或 5H 额度低于 5% 时，按卡片顺序向后寻找可用账号。</p><small>只向后查找，不循环到顶部 · 仅判断接口提供的额度窗口</small></div>}>
          <div className={`auto-switch-control${autoSwitchPi ? ' enabled' : ''}`}><span><Icon name="agent" size={13}/>Pi 自动切换</span><Switch size="small" checked={autoSwitchPi} loading={autoSwitching} aria-label="Pi Agent 额度不足时自动切换账号" onChange={checked => { autoSwitchEnabledRef.current = checked; setAutoSwitchPi(checked); setToast(`Pi 自动切换已${checked ? '开启' : '关闭'}`); }}/></div>
        </Popover>
        <Popover trigger="click" position="br" title="自动刷新额度" content={<div className="refresh-frequency-popover"><span>刷新间隔</span><InputNumber aria-label="额度自动刷新间隔（分钟）" min={1} max={999} precision={0} value={refreshMinutes} onChange={value => setRefreshMinutes(Math.min(999, Math.max(1, value || 1)))}/><span>分钟</span><small>允许设置 1–999 分钟</small></div>}><Button className="refresh-settings" type="default" aria-label={`自动刷新设置，当前 ${refreshMinutes} 分钟`} title="修改自动刷新频率" icon={<Icon name="settings" size={14}/>}><span>自动刷新</span><strong>{refreshMinutes} 分钟</strong></Button></Popover>
        <Button type="default" loading={refreshing === 'all'} disabled={busy || !accounts.length} onClick={() => void refreshAll()} icon={<Icon name="refresh"/>}>刷新全部</Button>
      </div></div>
      {loading ? <div className="empty"><Spin tip="正在连接本地凭据库…"/></div> : accounts.length ? <div className="accounts-grid">{orderedAccounts.map(a => <AccountCard key={a.id} account={a} codex={runtimes[0]?.managedId === a.id} pi={runtimes[1]?.managedId === a.id} busy={busy && (refreshing === null || refreshing === 'all' || refreshing === a.id)} refreshing={refreshing === a.id} pinned={pinnedAccounts.has(a.id)} dragging={draggingAccount === a.id} error={errors[a.id]} now={now} onAction={action} onPin={showWidget} onPointerDragStart={startAccountDrag} />)}</div> : <div className="empty"><Empty icon={<Icon name="grid" size={42}/>} description={<><h2>从你的第一个账号开始</h2><p>添加 Codex OAuth 账号，或导入上方检测到的本地账号。</p></>}/><Button type="primary" disabled={busy} onClick={() => void execute(() => doLogin())} icon={<Icon name="plus"/>}>添加 Codex 账号</Button></div>}
      <footer className="page-footer"><span><Icon name="shield" size={14}/>凭据仅在本机处理</span><span>额度为缓存快照 · 切换前请暂停运行中的 Agent</span></footer>
      </> : <RequestLogsPage/>}
    </main>
    {toast ? <div className="light-toast" role="status"><Icon name="check" size={15}/><span>{toast}</span></div> : null}
    <Modal title="删除这个账号？" visible={deleteCandidate !== null} onCancel={() => setDeleteCandidate(null)}
      maskClosable={false} focusLock autoFocus escToExit okText="确认删除" cancelText="取消" okButtonProps={{ status: 'danger' }}
      onOk={() => { if (!deleteCandidate) return; const id = deleteCandidate.id; setDeleteCandidate(null); void execute(async () => { await api.remove(id); setToast('账号已从本地凭据库删除'); }); }}>
      <div className="delete-confirmation"><strong>{deleteCandidate?.email ?? deleteCandidate?.accountId}</strong><p>管理器中的账号和凭据将清除。Codex / Pi 的认证文件不会删除，也不会远程撤销授权；当前账号将变为未管理状态。</p></div>
    </Modal>
  </div>;
}
