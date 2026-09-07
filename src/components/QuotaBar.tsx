import { Icon } from './Icon';
import Progress from '@arco-design/web-react/es/Progress';
import Tooltip from '@arco-design/web-react/es/Tooltip';

function resetLabel(reset: number | null | undefined, now: number) {
  if (reset == null) return '重置时间未知';
  const minutes = Math.ceil((reset - now) / 60_000);
  if (minutes <= 0) return '窗口已到期 · 请刷新';
  const hours = Math.floor(minutes / 60);
  return hours >= 24 ? `${Math.floor(hours / 24)}d ${hours % 24}h 后重置` : `${hours}h ${minutes % 60}m 后重置`;
}
export function QuotaBar({ label, value, reset, now }: { label: string; value: number | null | undefined; reset: number | null | undefined; now: number }) {
  const remaining = value == null ? null : Math.max(0, Math.min(100, value));
  return <div className="quota">
    <div className="quota-label">{label}<span>剩余</span></div>
    <div className="quota-value">{remaining == null ? '—' : Math.round(remaining)}{remaining == null ? null : <small>%</small>}</div>
    <div role="progressbar" aria-label={`${label} 剩余`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={remaining ?? undefined} aria-valuetext={remaining == null ? '暂无数据' : `${remaining}%`}>
      <div aria-hidden="true"><Progress percent={remaining ?? 0} showText={false} strokeWidth={5} color={remaining != null && remaining < 10 ? '#d4a269' : '#6894f5'}/></div>
    </div>
    <Tooltip content={reset == null ? '接口未提供重置时间' : new Date(reset).toLocaleString()}><div className="reset"><Icon name="clock" size={12}/>{resetLabel(reset, now)}</div></Tooltip>
  </div>;
}
