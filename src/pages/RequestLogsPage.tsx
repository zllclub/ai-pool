import { useCallback, useEffect, useMemo, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import Button from '@arco-design/web-react/es/Button';
import Spin from '@arco-design/web-react/es/Spin';
import { Icon } from '../components/Icon';
import { api, appError } from '../services/tauri';
import type { QuotaRequestLog, QuotaRequestReason } from '../types/account';

const REASON_LABELS: Record<QuotaRequestReason, string> = {
  manual: '手动刷新',
  manualAll: '手动刷新全部',
  automatic: '自动刷新',
  lowQuotaAutomatic: '低额度自动刷新',
  appStartup: '打开应用刷新',
  widgetManual: '悬浮窗刷新',
  autoSwitchCheck: '自动切换检查',
  import: '导入账号刷新',
  oauthLogin: '登录后刷新',
};

export function RequestLogsPage() {
  const [logs, setLogs] = useState<QuotaRequestLog[]>([]);
  const [loading, setLoading] = useState(true);
  const [accountFilter, setAccountFilter] = useState('all');
  const [error, setError] = useState('');

  const load = useCallback(async () => {
    try {
      setLogs(await api.requestLogs());
      setError('');
    } catch (value) { setError(appError(value).message); }
    finally { setLoading(false); }
  }, []);

  useEffect(() => {
    void load();
    let unlisten: (() => void) | undefined;
    void listen('quota-request-logged', () => void load()).then(dispose => { unlisten = dispose; });
    return () => unlisten?.();
  }, [load]);

  const accountOptions = useMemo(() => {
    const options = new Map<string, string>();
    for (const log of logs) if (!options.has(log.accountId)) options.set(log.accountId, log.accountLabel);
    return [...options].map(([id, label]) => ({ id, label })).sort((a, b) => a.label.localeCompare(b.label));
  }, [logs]);
  const filteredLogs = accountFilter === 'all' ? logs : logs.filter(log => log.accountId === accountFilter);
  const failures = filteredLogs.filter(log => !log.success).length;
  return <>
    <header className="page-header logs-header">
      <div><h1>请求日志</h1><p>最近 24 小时的额度请求记录</p></div>
      <div className="logs-header-actions">
        <label className="log-account-filter"><span>账号</span><select aria-label="根据账号筛选请求日志" value={accountFilter} onChange={event => setAccountFilter(event.target.value)}><option value="all">全部账号</option>{accountOptions.map(option => <option key={option.id} value={option.id}>{option.label}</option>)}</select></label>
        <Button type="default" loading={loading} onClick={() => void load()} icon={<Icon name="refresh"/>}>刷新日志</Button>
      </div>
    </header>
    <div className="log-summary" aria-label="日志统计">
      <span><b>{filteredLogs.length}</b> 次请求</span>
      <span className={failures ? 'has-failures' : ''}><b>{failures}</b> 次失败</span>
      <small>日志仅保存在本机，超过 24 小时自动清理</small>
    </div>
    {error ? <div className="logs-error"><Icon name="close" size={13}/>{error}</div> : null}
    {loading && logs.length === 0 ? <div className="logs-empty"><Spin tip="正在读取请求日志…"/></div> : filteredLogs.length ? <div className="request-log-list" role="list">
      <div className="request-log-columns" aria-hidden="true"><span>时间</span><span>账号</span><span>请求原因</span><span>结果</span></div>
      {filteredLogs.map((log, index) => <div className="request-log-row" role="listitem" key={`${log.timestamp}-${log.accountId}-${index}`}>
        <time dateTime={new Date(log.timestamp).toISOString()}>{new Date(log.timestamp).toLocaleString([], { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit' })}</time>
        <strong title={log.accountLabel}>{log.accountLabel}</strong>
        <span><em className={`log-reason ${log.reason}`}>{REASON_LABELS[log.reason]}</em></span>
        <span className={`log-result ${log.success ? 'success' : 'failure'}`}>{log.success ? '成功' : log.errorCode ?? '失败'}</span>
      </div>)}
    </div> : <div className="logs-empty"><Icon name="logs" size={32}/><strong>{accountFilter === 'all' ? '近 24 小时暂无请求' : '该账号暂无请求记录'}</strong><span>{accountFilter === 'all' ? '额度刷新后会在这里记录账号、时间和触发原因' : '可切换到其他账号或查看全部日志'}</span></div>}
  </>;
}
