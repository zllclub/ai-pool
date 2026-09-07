import { memo } from 'react';
import Card from '@arco-design/web-react/es/Card';
import Button from '@arco-design/web-react/es/Button';
import Avatar from '@arco-design/web-react/es/Avatar';
import Dropdown from '@arco-design/web-react/es/Dropdown';
import Menu from '@arco-design/web-react/es/Menu';
import Tag from '@arco-design/web-react/es/Tag';
import Alert from '@arco-design/web-react/es/Alert';
import Tooltip from '@arco-design/web-react/es/Tooltip';
import type { AccountAction, AppError, CodexAccount } from '../types/account';
import { CurrentAccountBadge } from './CurrentAccountBadge';
import { QuotaBar } from './QuotaBar';
import { Icon } from './Icon';
import { quotaWindows } from '../services/quota';
import { accountAvatarStyle } from '../services/avatar';
interface Props { account: CodexAccount; codex: boolean; pi: boolean; busy: boolean; refreshing: boolean; pinned: boolean; error?: AppError; now: number; onAction: (action: AccountAction, account: CodexAccount) => void; onPin: (account: CodexAccount) => void }

export const AccountCard = memo(function AccountCard({ account: a, codex, pi, busy, refreshing, pinned, error, now, onAction, onPin }: Props) {
  const accountName = a.email ?? a.accountId;
  const windows = quotaWindows(a.quota);
  const menu = <Menu onClickMenuItem={key => { if (!busy) onAction(key as AccountAction, a); }}>
    <Menu.Item key="reauthorize" disabled={busy}><span className="account-menu-action"><Icon name="shield"/><span>重新授权</span></span></Menu.Item>
    <Menu.Item key="delete" disabled={busy} className="delete-menu-item"><span className="account-menu-action"><Icon name="trash"/><span>删除账号</span></span></Menu.Item>
  </Menu>;
  return <Card className={`account-card${codex || pi ? ' is-active' : ''}`} bordered>
    <header className="card-header">
      <Avatar size={32} shape="square" className="account-avatar" style={accountAvatarStyle(accountName)}>{accountName.slice(0, 2).toUpperCase()}</Avatar>
      <div className="identity">
        <div className="identity-title"><Tooltip content={a.email ?? a.accountId}><h2>{a.email ?? a.accountId}</h2></Tooltip><Tag size="small" bordered={false}>{a.planType ?? '未知计划'}</Tag></div>
        <div className="identity-meta"><span className="account-state">{codex ? <CurrentAccountBadge name="Codex" /> : null}{pi ? <CurrentAccountBadge name="Pi Agent" /> : null}{!codex && !pi ? <Tag size="small" bordered={false}>备用账号</Tag> : null}</span><span className="header-updated">{a.quota ? `更新于 ${new Date(a.quota.updatedAt).toLocaleTimeString()} · ${Math.max(0, Math.floor((now - a.quota.updatedAt) / 60_000))} 分钟前` : '尚未查询额度'}{busy ? ' · 处理中…' : ''}</span></div>
      </div>
      <div className="card-header-actions"><Button className={`pin-button${pinned ? ' is-pinned' : ''}`} type="text" size="small" shape="circle" aria-pressed={pinned} aria-label={pinned ? '关闭此账号的悬浮显示' : '悬浮显示此账号'} title={pinned ? '关闭悬浮显示' : '悬浮显示'} disabled={busy} onClick={() => onPin(a)} icon={<Icon name="desktop" size={16}/>}/><Dropdown droplist={menu} trigger={['hover', 'click']} position="br" disabled={busy}>
        <Button type="text" size="small" shape="circle" aria-label="账号操作" disabled={busy} icon={<Icon name="more" size={19}/>}/>
      </Dropdown></div>
    </header>
    {windows.length ? <div className={`quota-grid${windows.length === 1 ? ' quota-single' : ''}`}>
      {windows.map(w => <QuotaBar key={w.id} label={w.label} value={w.remaining} reset={w.resetAt} now={now}/>)}
    </div> : <div className="quota-empty"><Icon name="clock" size={20}/><span>{a.quota ? '接口未提供额度窗口' : '尚未获取额度'}</span><small>{a.quota ? '不代表额度已耗尽或无限使用' : '刷新额度后查看可用窗口'}</small></div>}
    {error ? <Alert className="card-error" type="error" content={<>{error.message} <code>{error.code}</code></>}/> : null}
    <footer className="card-actions"><Button type="default" size="small" disabled={busy} onClick={() => onAction('codex', a)} icon={<Icon name="terminal" size={14}/>}>设为 Codex</Button><Button type="default" size="small" disabled={busy} onClick={() => onAction('pi', a)} icon={<Icon name="agent" size={14}/>}>设为 Pi</Button><Button className="card-refresh" size="small" type="secondary" loading={refreshing} disabled={busy && !refreshing} onClick={() => onAction('refresh', a)} icon={<Icon name="refresh" size={14}/>}>刷新</Button></footer>
  </Card>;
});
