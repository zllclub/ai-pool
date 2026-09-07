// Deliberately excludes ALL credentials. Rust alone owns the token lifecycle.
export interface AccountQuota {
  fiveHourRemaining: number | null; weeklyRemaining: number | null;
  fiveHourResetAt: number | null; weeklyResetAt: number | null; updatedAt: number;
}
export interface CodexAccount {
  id: string; accountId: string; email: string | null; planType: string | null;
  quota: AccountQuota | null; createdAt: number; lastUsedAt: number | null;
}
export interface AppError { code: string; message: string; detail?: string }
export interface RuntimeStatus { runtime: string; path: string; accountId: string | null; managedId: string | null; error: AppError | null }
export interface QuotaResult { accountId: string; error: AppError | null }
export interface LoginResult { account: CodexAccount; quotaError: AppError | null }
export type AccountAction = 'refresh' | 'codex' | 'pi' | 'both' | 'reauthorize' | 'delete';
