import type { CodexAccount } from '../types/account';
import { quotaWindows } from './quota';

export const REFRESH_POLICY_KEY = 'adaptive-refresh-policy.v1';
export const REFRESH_POLICY_EVENT = 'adaptive-refresh-policy-changed';

interface AccountRefreshPolicy {
  lastFiveHourRemaining: number | null;
  lastWeeklyRemaining: number | null;
  // Kept optional so existing 5H-only v1 records can migrate without a reset.
  lastRemaining?: number;
  unchangedCount: number;
  throttled: boolean;
}

type RefreshPolicies = Record<string, AccountRefreshPolicy>;

function readPolicies(): RefreshPolicies {
  try {
    const value: unknown = JSON.parse(window.localStorage.getItem(REFRESH_POLICY_KEY) ?? '{}');
    return value && typeof value === 'object' && !Array.isArray(value) ? value as RefreshPolicies : {};
  } catch { return {}; }
}

function writePolicies(policies: RefreshPolicies) {
  window.localStorage.setItem(REFRESH_POLICY_KEY, JSON.stringify(policies));
  window.dispatchEvent(new Event(REFRESH_POLICY_EVENT));
}

export function fiveHourRemaining(account: CodexAccount | null | undefined) {
  if (!account?.quota) return null;
  const window = quotaWindows(account.quota).find(item =>
    item.limitWindowSeconds === 5 * 60 * 60 || /(^|\D)5\s*h|five\s*hour|5\s*小时/i.test(item.label),
  );
  return window?.remaining ?? account.quota.fiveHourRemaining;
}

export function weeklyRemaining(account: CodexAccount | null | undefined) {
  if (!account?.quota) return null;
  const window = quotaWindows(account.quota).find(item =>
    item.limitWindowSeconds === 7 * 24 * 60 * 60 || /week|weekly|周/i.test(item.label),
  );
  return window?.remaining ?? account.quota.weeklyRemaining;
}

export function shouldAutoSwitchPi(account: CodexAccount) {
  const weekly = weeklyRemaining(account);
  const fiveHour = fiveHourRemaining(account);
  return (weekly != null && weekly < 3) || (fiveHour != null && fiveHour < 5);
}

export function isEligiblePiSuccessor(account: CodexAccount) {
  const weekly = weeklyRemaining(account);
  const fiveHour = fiveHourRemaining(account);
  // Some plans only expose one quota window. Judge every available window and
  // do not reject a candidate merely because the API does not provide 5H.
  return (weekly != null || fiveHour != null)
    && (weekly == null || weekly > 3)
    && (fiveHour == null || fiveHour > 5);
}

function hasLowRefreshQuota(account: CodexAccount) {
  const fiveHour = fiveHourRemaining(account);
  const weekly = weeklyRemaining(account);
  return (fiveHour != null && fiveHour < 20) || (weekly != null && weekly < 8);
}

export function isLowQuotaRefreshActive(account: CodexAccount, active: boolean) {
  return active && hasLowRefreshQuota(account) && !readPolicies()[account.id]?.throttled;
}

export function accountRefreshMinutes(account: CodexAccount, active: boolean, configuredMinutes: number) {
  if (!active) return 60;
  return isLowQuotaRefreshActive(account, active) ? 1 : configuredMinutes;
}

export function recordRefreshResult(before: CodexAccount | null | undefined, after: CodexAccount) {
  const fiveHour = fiveHourRemaining(after);
  const weekly = weeklyRemaining(after);
  const policies = readPolicies();
  if (!hasLowRefreshQuota(after)) {
    if (policies[after.id]) { delete policies[after.id]; writePolicies(policies); }
    return;
  }

  const current = policies[after.id];
  const previousFiveHour = current
    ? (current.lastFiveHourRemaining ?? current.lastRemaining ?? null)
    : fiveHourRemaining(before);
  const previousWeekly = current ? (current.lastWeeklyRemaining ?? null) : weeklyRemaining(before);
  const same = (left: number | null, right: number | null) =>
    left == null ? right == null : right != null && Math.abs(left - right) < 0.001;
  const hasPrevious = previousFiveHour != null || previousWeekly != null;
  const unchanged = hasPrevious && same(previousFiveHour, fiveHour) && same(previousWeekly, weekly);
  const unchangedCount = unchanged ? Math.min(5, (current?.unchangedCount ?? 0) + 1) : 0;
  policies[after.id] = {
    lastFiveHourRemaining: fiveHour,
    lastWeeklyRemaining: weekly,
    unchangedCount,
    throttled: unchangedCount >= 5,
  };
  writePolicies(policies);
}
