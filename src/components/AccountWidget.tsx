import { useCallback, useEffect, useRef, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { api } from '../services/tauri';
import { quotaWindows } from '../services/quota';
import { accountAvatarStyle } from '../services/avatar';
import type { CodexAccount } from '../types/account';
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

export function AccountWidget() {
  const [accountId] = useState(() => new URLSearchParams(window.location.search).get('account') ?? '');
  const [account, setAccount] = useState<CodexAccount | null>(null);
  const [minutes, setMinutes] = useState(storedMinutes);
  const [active, setActive] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [activity, setActivity] = useState(0);
  const [error, setError] = useState('');
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const refreshingRef = useRef(false);
  const lastRefreshAttempt = useRef(0);

  const load = useCallback(async () => {
    if (!accountId) return;
    try {
      const [items, runtimes] = await Promise.all([api.list(), api.status()]);
      const found = items.find(item => item.id === accountId) ?? null;
      setAccount(found);
      setActive(runtimes.some(runtime => runtime.managedId === accountId));
      setError(found ? '' : '账号不存在');
    } catch { setError('读取失败'); }
  }, [accountId]);
  const refresh = useCallback(async () => {
    if (!accountId || refreshingRef.current) return;
    refreshingRef.current = true;
    lastRefreshAttempt.current = Date.now();
    setRefreshing(true);
    try { setAccount(await api.refresh(accountId)); setError(''); }
    catch { setError('刷新失败'); }
    finally { refreshingRef.current = false; setRefreshing(false); }
  }, [accountId]);
  const changeMode = useCallback(async (next: boolean) => {
    setExpanded(next);
    if (!next) setContextMenu(null);
    try { await api.setWidgetExpanded(next); } catch { /* Keep the widget usable if resizing fails. */ }
  }, []);
  const close = () => void api.closeWidget(accountId);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => {
    const storage = (event: StorageEvent) => {
      if (event.key === 'quota-refresh-minutes') setMinutes(storedMinutes());
    };
    window.addEventListener('storage', storage);
    let unlisten: (() => void) | undefined;
    void getCurrentWindow().onFocusChanged(({ payload }) => { if (!payload && expanded) void changeMode(false); }).then(fn => { unlisten = fn; });
    return () => { window.removeEventListener('storage', storage); unlisten?.(); };
  }, [changeMode, expanded]);
  useEffect(() => {
    if (!account) return;

    let alive = true;
    let timer: number | undefined;
    const interval = (active ? minutes : 60) * 60_000;
    const schedule = () => {
      if (!alive) return;
      window.clearTimeout(timer);
      const baseline = Math.max(account.quota?.updatedAt ?? 0, lastRefreshAttempt.current);
      const delay = Math.max(0, baseline + interval - Date.now());
      timer = window.setTimeout(async () => {
        await refresh();
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
  }, [account, active, minutes, refresh]);
  useEffect(() => {
    if (!expanded || contextMenu) return;
    const timer = window.setTimeout(() => void changeMode(false), 3000);
    return () => window.clearTimeout(timer);
  }, [activity, changeMode, contextMenu, expanded]);

  const name = account?.email ?? account?.accountId ?? 'AI Pool';
  const shortName = name.includes('@') ? name.slice(0, name.indexOf('@')) : name;
  const windows = quotaWindows(account?.quota ?? null).slice(0, 2);
  const updated = account?.quota ? new Date(account.quota.updatedAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) : '—';
  const startDrag = async (target: EventTarget | null, button: number) => {
    if (button !== 0 || (target as HTMLElement).closest('button, .widget-context-menu')) return;
    try { await getCurrentWindow().startDragging(); await api.snapWidget(); } catch { /* Native drag can be cancelled. */ }
  };
  const openMenu = async (event: React.MouseEvent) => {
    event.preventDefault();
    setActivity(value => value + 1);
    if (!expanded) await changeMode(true);
    setContextMenu({ x: Math.max(6, Math.min(event.clientX, window.innerWidth - 88)), y: Math.max(6, Math.min(event.clientY, window.innerHeight - 34)) });
  };

  return <div className={`widget-page ${expanded ? 'expanded' : 'collapsed'}${refreshing ? ' is-refreshing' : ''}`}  title="拖动吸附边缘 · 右键打开菜单"
    onMouseDown={event => void startDrag(event.target, event.button)}
    onClick={event => { setActivity(value => value + 1); setContextMenu(null); if (!expanded && !(event.target as HTMLElement).closest('button')) { void changeMode(true); void refresh(); } }}
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
      </header>
      {error ? <div className="widget-error">{error}</div> : windows.length ? <div className={`widget-quotas${windows.length === 1 ? ' single' : ''}`}>
        {windows.map(item => <div className={`widget-quota${quotaTone(item.remaining)}`} key={item.id}><div><span>{compactLabel(item.label)}</span><strong>{item.remaining == null ? '—' : Math.round(item.remaining)}<small>%</small></strong></div><div className="widget-track"><i style={{ width: `${Math.max(0, Math.min(100, item.remaining ?? 0))}%` }}/></div></div>)}
      </div> : <div className="widget-error">暂无额度</div>}
      <footer className="widget-footer" data-tauri-drag-region><span>{updated} 更新</span><button type="button" title="刷新" aria-label="刷新额度" disabled={refreshing || !account} onClick={event => { event.stopPropagation(); void refresh(); }}><Icon name="refresh" size={12} className={refreshing ? 'widget-spinning' : undefined}/></button></footer>
    </>}
    {contextMenu ? <div className="widget-context-menu" style={{ left: contextMenu.x, top: contextMenu.y }} onClick={event => event.stopPropagation()}>
      <button type="button" className="danger" onClick={close}><Icon name="close" size={12}/>关闭悬浮窗</button>
    </div> : null}
  </div>;
}
