import { memo } from 'react';
import type { AccountAction, AppError, CodexAccount } from '../types/account';
import { CurrentAccountBadge } from './CurrentAccountBadge';
import { QuotaBar } from './QuotaBar';
import { Icon } from './Icon';
import { quotaWindows } from '../services/quota';
interface Props { account: CodexAccount; codex: boolean; pi: boolean; busy: boolean; error?: AppError; now: number; onAction: (action: AccountAction, account: CodexAccount) => void }
export const AccountCard = memo(function AccountCard({ account: a, codex, pi, busy, error, now, onAction }: Props) {
  const stale = a.quota && now - a.quota.updatedAt > 300_000;
  const windows = quotaWindows(a.quota);
  return <article className={`account-card${codex || pi ? ' is-active' : ''}`}>
    <header className="card-header">
      <div className="avatar">{(a.email ?? a.accountId).slice(0, 2).toUpperCase()}</div>
      <div className="identity"><h2 title={a.email ?? a.accountId}>{a.email ?? a.accountId}</h2><div className="identity-meta"><span>{a.planType ?? '未知计划'}</span><span title={a.accountId}>{a.accountId.slice(0, 12)}…</span></div></div>
      <details className="menu"><summary aria-label="账号操作"><Icon name="more" size={19}/></summary><div>
        <button disabled={busy} onClick={() => onAction('refresh', a)}>刷新额度</button>
        <button disabled={busy} onClick={() => onAction('reauthorize', a)}>重新授权</button>
        <button className="danger" disabled={busy} onClick={() => onAction('delete', a)}>删除账号</button>
      </div></details>
    </header>
    <div className="badges">{codex ? <CurrentAccountBadge name="Codex" /> : null}{pi ? <CurrentAccountBadge name="Pi Agent" /> : null}{!codex && !pi ? <span className="standby">备用账号</span> : null}</div>
    {windows.length ? <div className={`quota-grid${windows.length === 1 ? ' quota-single' : ''}`}>
      {windows.map(w => <QuotaBar key={w.id} label={w.label} value={w.remaining} reset={w.resetAt} now={now}/>)}
    </div> : <div className="quota-empty"><Icon name="clock" size={20}/><span>{a.quota ? '接口未提供额度窗口' : '尚未获取额度'}</span><small>{a.quota ? '不代表额度已耗尽或无限使用' : '刷新额度后查看可用窗口'}</small></div>}
    {error ? <p className="card-error" role="alert">{error.message} <code>{error.code}</code></p> : null}
    <div className="updated">{a.quota ? `${stale ? '缓存数据 · ' : ''}更新于 ${new Date(a.quota.updatedAt).toLocaleTimeString()}` : '尚未查询额度'}{busy ? <span>处理中…</span> : null}</div>
    <footer className="card-actions"><button disabled={busy} onClick={() => onAction('codex', a)}><Icon name="terminal" size={14}/>设为 Codex</button><button disabled={busy} onClick={() => onAction('pi', a)}><Icon name="agent" size={14}/>设为 Pi</button><button disabled={busy} onClick={() => onAction('both', a)}>同时切换<Icon name="arrow" size={14}/></button></footer>
  </article>;
});
