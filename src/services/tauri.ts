import { invoke } from '@tauri-apps/api/core';
import type { AppError, CodexAccount, LoginResult, QuotaResult, RuntimeStatus } from '../types/account';
// Single IPC boundary. No fetch, filesystem plugins, token storage or OAuth URL handling in JS.
export const api = {
  list: () => invoke<CodexAccount[]>('list_accounts'),
  status: () => invoke<RuntimeStatus[]>('get_runtime_status'),
  login: (accountId?: string) => invoke<LoginResult>('start_oauth_login', { accountId: accountId ?? null }),
  cancel: () => invoke<void>('cancel_oauth_login'),
  remove: (accountId: string) => invoke<void>('delete_account', { accountId }),
  refresh: (accountId: string) => invoke<CodexAccount>('refresh_account', { accountId }),
  refreshAll: () => invoke<QuotaResult[]>('refresh_all_quotas'),
  switchCodex: (accountId: string) => invoke<void>('switch_codex_account', { accountId }),
  switchPi: (accountId: string) => invoke<void>('switch_pi_account', { accountId }),
  switchBoth: (accountId: string) => invoke<void>('switch_both', { accountId }),
  importCodex: () => invoke<CodexAccount>('import_current_codex_account'),
  importPi: () => invoke<CodexAccount>('import_current_pi_account'),
};
export function appError(value: unknown): AppError {
  if (value && typeof value === 'object' && 'code' in value && 'message' in value) return value as AppError;
  return { code: 'IPC_ERROR', message: '无法连接本地后端，请使用 npm run tauri dev 启动应用。' };
}
