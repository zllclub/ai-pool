import { useCallback, useEffect, useRef, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';
import { api } from '../services/tauri';
import { quotaWindows } from '../services/quota';
import { accountAvatarStyle } from '../services/avatar';
import { accountRefreshMinutes, isLowQuotaRefreshActive, recordRefreshResult, REFRESH_POLICY_EVENT, REFRESH_POLICY_KEY } from '../services/refreshPolicy';
import type { CodexAccount, QuotaRequestReason } from '../types/account';
import { Icon } from './Icon';

function compactLabel(label: string) {
  if (/weekly|week|周/i.test(label)) return 'W';
  if (/5\s*h|five/i.test(label)) return '5H';
  return label.length > 4 ? label.slice(0, 4) : label;
}
function quotaTone(value: number | null) {
  if (value == null) return '';
  if (value <= 20) return ' critical';
  if (value <= 50) return ' warning';
  return '';
}
function storedMinutes() {
  const value = Number(window.localStorage.getItem('quota-refresh-minutes'));
  return Number.isInteger(value) && value >= 1 && value <= 999 ? value : 5;
}
function countdownLabel(milliseconds: number) {
  const totalSeconds = Math.max(0, Math.ceil(milliseconds / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  return hours > 0 ? `${hours} 小时 ${minutes} 分` : `${minutes}:${String(seconds).padStart(2, '0')}`;
}

export function AccountWidget() {
  const [accountId] = useState(() => new URLSearchParams(window.location.search).get('account') ?? '');
  const [account, setAccount] = useState<CodexAccount | null>(null);
  const [minutes, setMinutes] = useState(storedMinutes);
  const [active, setActive] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [refreshPolicyVersion, setRefreshPolicyVersion] = useState(0);
  const [expanded, setExpanded] = useState(false);
  const [nextRefreshAt, setNextRefreshAt] = useState<number | null>(null);
  const [countdownNow, setCountdownNow] = useState(Date.now);
  const [error, setError] = useState('');
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const refreshingRef = useRef(false);
  const expandedRef = useRef(false);
  const accountRef = useRef<CodexAccount | null>(null);
  const lastRefreshAttempt = useRef(0);

  const load = useCallback(async () => {
    if (!accountId) return;
    try {
      const [items, runtimes] = await Promise.all([api.list(), api.status()]);
      const found = items.find(item => item.id === accountId) ?? null;
      accountRef.current = found;
      setAccount(found);
      setActive(runtimes.some(runtime => runtime.managedId === accountId));
      setError(found ? '' : '账号不存在');
    } catch { setError('读取失败'); }
  }, [accountId]);
  const changeMode = useCallback(async (next: boolean) => {
    expandedRef.current = next;
    setExpanded(next);
    if (!next) setContextMenu(null);
    try { await api.setWidgetExpanded(accountId, next); } catch { /* Keep the widget usable if resizing fails. */ }
  }, [accountId]);
  const refresh = useCallback(async (reason: QuotaRequestReason = 'widgetManual') => {
    if (!accountId || refreshingRef.current) return;
    refreshingRef.current = true;
    lastRefreshAttempt.current = Date.now();
    setRefreshing(true);
    try {
      const before = accountRef.current;
      const updated = await api.refresh(accountId, reason);
      recordRefreshResult(before, updated);
      accountRef.current = updated;
      setAccount(updated);
      setError('');
      if (expandedRef.current) await changeMode(false);
    } catch { setError('刷新失败'); }
    finally { refreshingRef.current = false; setRefreshing(false); }
  }, [accountId, changeMode]);
  const close = () => void api.closeWidget(accountId);
  const effectiveRefreshMinutes = account ? accountRefreshMinutes(account, active, minutes) : minutes;

  useEffect(() => { void load(); }, [load]);
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<string | null>('account-data-changed', event => {
      if (event.payload !== null && event.payload !== accountId) return;
      const previousUpdatedAt = accountRef.current?.quota?.updatedAt ?? 0;
      void load().then(() => {
        const refreshedAt = accountRef.current?.quota?.updatedAt ?? 0;
        if (expandedRef.current && refreshedAt > previousUpdatedAt) void changeMode(false);
      });
    }).then(dispose => { unlisten = dispose; });
    return () => unlisten?.();
  }, [accountId, changeMode, load]);
  useEffect(() => {
    const policyChanged = () => setRefreshPolicyVersion(version => version + 1);
    const storage = (event: StorageEvent) => {
      if (event.key === 'quota-refresh-minutes') setMinutes(storedMinutes());
      if (event.key === REFRESH_POLICY_KEY) policyChanged();
    };
    window.addEventListener('storage', storage);
    window.addEventListener(REFRESH_POLICY_EVENT, policyChanged);
    return () => { window.removeEventListener('storage', storage); window.removeEventListener(REFRESH_POLICY_EVENT, policyChanged); };
  }, []);
  useEffect(() => {
    if (!account) {
      setNextRefreshAt(null);
      return;
    }

    let alive = true;
    let timer: number | undefined;
    const interval = effectiveRefreshMinutes * 60_000;
    const schedule = () => {
      if (!alive) return;
      window.clearTimeout(timer);
      const baseline = Math.max(account.quota?.updatedAt ?? 0, lastRefreshAttempt.current);
      const target = baseline + interval;
      setNextRefreshAt(target);
      const delay = Math.max(0, target - Date.now());
      timer = window.setTimeout(async () => {
        await refresh(isLowQuotaRefreshActive(account, active) ? 'lowQuotaAutomatic' : 'automatic');
        schedule();
      }, delay);
    };
    const catchUp = () => schedule();
    const visibility = () => { if (document.visibilityState === 'visible') catchUp(); };
    schedule();
    window.addEventListener('focus', catchUp);
    document.addEventListener('visibilitychange', visibility);
    return () => {
      alive = false;
      window.clearTimeout(timer);
      window.removeEventListener('focus', catchUp);
      document.removeEventListener('visibilitychange', visibility);
    };
  }, [account, active, effectiveRefreshMinutes, refresh, refreshPolicyVersion]);
  useEffect(() => {
    if (nextRefreshAt === null) return;
    const tick = () => setCountdownNow(Date.now());
    tick();
    const timer = window.setInterval(tick, 1000);
    return () => window.clearInterval(timer);
  }, [nextRefreshAt]);
  const name = account?.email ?? account?.accountId ?? 'AI Pool';
  const shortName = name.includes('@') ? name.slice(0, name.indexOf('@')) : name;
  const windows = quotaWindows(account?.quota ?? null).slice(0, 2);
  const updated = account?.quota ? new Date(account.quota.updatedAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) : '—';
  const refreshInterval = effectiveRefreshMinutes * 60_000;
  const countdownRemaining = Math.max(0, (nextRefreshAt ?? countdownNow) - countdownNow);
  const countdownProgress = Math.min(1, countdownRemaining / refreshInterval);
  const countdownText = countdownLabel(countdownRemaining);
  const startDrag = async (target: EventTarget | null, button: number) => {
    if (button !== 0 || (target as HTMLElement).closest('button, .widget-context-menu')) return;
    try { await getCurrentWindow().startDragging(); await api.snapWidget(accountId); } catch { /* Native drag can be cancelled. */ }
  };
  const openMenu = async (event: React.MouseEvent) => {
    event.preventDefault();
    if (!expanded) await changeMode(true);
    setContextMenu({ x: Math.max(6, Math.min(event.clientX, window.innerWidth - 88)), y: Math.max(6, Math.min(event.clientY, window.innerHeight - 34)) });
  };

  return <div className={`widget-page ${expanded ? 'expanded' : 'collapsed'}${refreshing ? ' is-refreshing' : ''}`}  title="拖动吸附边缘 · 右键打开菜单"
    onMouseDown={event => void startDrag(event.target, event.button)}
    onClick={event => { setContextMenu(null); if (!expanded && !(event.target as HTMLElement).closest('button')) void changeMode(true); }}
    onContextMenu={event => void openMenu(event)}>
    {!expanded ? <div className="widget-capsule" data-tauri-drag-region>
      <div className="widget-mini-avatar" style={accountAvatarStyle(name)}>{name.slice(0, 2).toUpperCase()}</div><strong>{shortName.slice(0, 7)}</strong>
      <div className="widget-capsule-quotas">{windows.map(item => <span className={quotaTone(item.remaining)} key={item.id}><b>{compactLabel(item.label)}</b><em>{item.remaining == null ? '—' : Math.round(item.remaining)}%</em></span>)}</div>
      {refreshing ? <Icon name="refresh" size={11} className="widget-spinning"/> : null}
    </div> : <>
      <header className="widget-header" data-tauri-drag-region>
        <div className="widget-avatar" style={accountAvatarStyle(name)}>{name.slice(0, 2).toUpperCase()}</div>
        <div className="widget-identity" data-tauri-drag-region><strong>{name}</strong><span>{account?.planType ?? '账号'}</span></div>
        {refreshing ? <div className="widget-auto" title="正在更新额度"><Icon name="refresh" size={10} className="widget-spinning"/>更新中</div> : null}
        <button type="button" className="widget-collapse" title="折叠" aria-label="折叠悬浮窗" onClick={event => { event.stopPropagation(); void changeMode(false); }}><Icon name="collapse" size={11}/></button>
      </header>
      {error ? <div className="widget-error">{error}</div> : windows.length ? <div className={`widget-quotas${windows.length === 1 ? ' single' : ''}`}>
        {windows.map(item => <div className={`widget-quota${quotaTone(item.remaining)}`} key={item.id}><div><span>{compactLabel(item.label)}</span><strong>{item.remaining == null ? '—' : Math.round(item.remaining)}<small>%</small></strong></div><div className="widget-track"><i style={{ width: `${Math.max(0, Math.min(100, item.remaining ?? 0))}%` }}/></div></div>)}
      </div> : <div className="widget-error">暂无额度</div>}
      <footer className="widget-footer" data-tauri-drag-region>
        <span>{updated} 更新</span>
        {nextRefreshAt !== null ? <span className="widget-next-refresh" title="下次自动刷新"><Icon name="clock" size={9}/>{countdownText}</span> : null}
        <button type="button" title="刷新" aria-label="刷新额度" disabled={refreshing || !account} onClick={event => { event.stopPropagation(); void refresh('widgetManual'); }}><Icon name="refresh" size={12} className={refreshing ? 'widget-spinning' : undefined}/></button>
      </footer>
    </>}
    {nextRefreshAt !== null ? <div className="widget-refresh-countdown" role="progressbar" aria-label="下次自动刷新倒计时" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(countdownProgress * 100)} aria-valuetext={countdownText} title={`距下次刷新 ${countdownText}`}><span className="widget-countdown-fill" style={{ transform: `scaleX(${countdownProgress})` }}/><span className="widget-countdown-marker" style={{ left: `${countdownProgress * 100}%` }}/></div> : null}
    {contextMenu ? <div className="widget-context-menu" style={{ left: contextMenu.x, top: contextMenu.y }} onClick={event => event.stopPropagation()}>
      <button type="button" className="danger" onClick={close}><Icon name="close" size={12}/>关闭悬浮窗</button>
    </div> : null}
  </div>;
}
