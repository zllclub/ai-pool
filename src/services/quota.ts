import type { AccountQuota, QuotaWindow } from '../types/account';

// Display normalized backend windows. Only old caches need the fixed-field fallback.
export function quotaWindows(quota: AccountQuota | null): QuotaWindow[] {
  if (!quota) return [];
  if (quota.windows != null) return quota.windows;
  const windows: QuotaWindow[] = [];
  if (quota.fiveHourRemaining != null || quota.fiveHourResetAt != null) windows.push({
    id: 'legacy-5h', label: '5H', limitWindowSeconds: 18000,
    remaining: quota.fiveHourRemaining, resetAt: quota.fiveHourResetAt,
  });
  if (quota.weeklyRemaining != null || quota.weeklyResetAt != null) windows.push({
    id: 'legacy-weekly', label: 'Weekly', limitWindowSeconds: 604800,
    remaining: quota.weeklyRemaining, resetAt: quota.weeklyResetAt,
  });
  return windows;
}
