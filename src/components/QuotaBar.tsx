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
    <div className="progress" role="progressbar" aria-label={`${label} 剩余`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={remaining ?? undefined} aria-valuetext={remaining == null ? '暂无数据' : `${remaining}%`}>
      <div style={{ width: `${remaining ?? 0}%` }} className={remaining != null && remaining < 10 ? 'low' : ''} />
    </div>
    <div className="reset" title={reset == null ? undefined : new Date(reset).toLocaleString()}>{resetLabel(reset, now)}</div>
  </div>;
}
