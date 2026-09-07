use crate::{
    account::{model::now, service::AccountService},
    error::{AppError, Result},
    runtime::Snapshot,
    storage::atomic,
};
use std::path::PathBuf;

fn conflict() -> AppError {
    AppError::new(
        "RUNTIME_DESYNC",
        "认证文件被外部进程修改；请暂停 Codex/Pi 后重试",
    )
}
struct Prepared {
    path: PathBuf,
    old: Option<Vec<u8>>,
    new: Vec<u8>,
    temp: Option<tempfile::NamedTempFile>,
}
// Injectable commit boundary allows deterministic failure/rollback tests without touching real auth files.
fn commit_prepared(
    prepared: &mut [Prepared],
    mut commit: impl FnMut(usize, &std::path::Path, tempfile::NamedTempFile) -> Result<()>,
) -> Result<()> {
    for p in prepared.iter() {
        if atomic::read_optional(&p.path)? != p.old {
            return Err(conflict());
        }
    }
    for index in 0..prepared.len() {
        let result = (|| {
            let p = &mut prepared[index];
            if atomic::read_optional(&p.path)? != p.old {
                return Err(conflict());
            }
            commit(index, &p.path, p.temp.take().ok_or_else(conflict)?)
        })();
        if let Err(e) = result {
            // Two files cannot be committed atomically. Roll back OUR bytes only.
            for p in prepared[..index].iter().rev() {
                let rollback = (|| {
                    if atomic::read_optional(&p.path)? != Some(p.new.clone()) {
                        return Err(conflict());
                    }
                    match &p.old {
                        Some(b) => atomic::write(&p.path, b),
                        None => std::fs::remove_file(&p.path).map_err(|e| AppError::io(&p.path, e)),
                    }
                })();
                if rollback.is_err() {
                    return Err(AppError::new(
                        "SWITCH_PARTIAL",
                        "双环境切换部分完成且无法安全回滚，请检查顶部实际账号状态",
                    ));
                }
            }
            return Err(e);
        }
    }
    Ok(())
}

impl AccountService {
    pub async fn switch(&self, id: &str, targets: &[usize]) -> Result<()> {
        // Lock order everywhere: Switch -> Account Token -> Repository. Never reverse it.
        let _switch = self.switch_lock.lock().await;
        let mut snapshots: Vec<(usize, Snapshot)> = Vec::new();
        for &i in targets {
            snapshots.push((i, self.runtimes[i].read()?));
        }
        // Capture CLI-rotated credentials of outgoing managed accounts BEFORE preparing target.
        for (_, s) in &snapshots {
            if let Some(local) = &s.account {
                if let Some(managed) = self
                    .repo
                    .all()
                    .await
                    .into_iter()
                    .find(|a| a.view.account_id == local.view.account_id)
                {
                    let _token = self.token_lock(&managed.view.id).await;
                    self.reconcile_locked(managed).await?;
                }
            }
        }
        let _token = self.token_lock(id).await;
        let mut target = self.fresh_locked(id, false).await?;
        // Pi does not store id_token, but Codex expects one. Try the isolated token endpoint first.
        if targets.contains(&0) && target.credentials.id_token.is_none() {
            target = self.fresh_locked(id, true).await?;
        }
        let mut prepared = Vec::new();
        // Prepare ALL JSON and tempfiles first. Any preparation failure leaves both originals intact.
        for (i, s) in snapshots {
            let adapter = &self.runtimes[i];
            let value = adapter.apply_account(&s.value, &target)?;
            let bytes = serde_json::to_vec_pretty(&value)
                .map_err(|_| AppError::new("SERIALIZE", "认证文件序列化失败"))?;
            let check = atomic::json(&bytes)?;
            let detected = adapter
                .detect_current_account(&check)?
                .ok_or_else(|| AppError::new("RUNTIME_SCHEMA", "写入验证失败"))?;
            if detected.view.account_id != target.view.account_id {
                return Err(conflict());
            }
            let path = adapter.path().to_owned();
            let temp = atomic::stage(&path, &bytes)?;
            prepared.push(Prepared {
                path,
                old: s.bytes,
                new: bytes,
                temp: Some(temp),
            });
        }
        commit_prepared(&mut prepared, |_, path, temp| atomic::commit(path, temp))?;
        target.view.last_used_at = Some(now());
        // Current account is always derived from disk; no stale currentAccountId cache.
        self.repo.put(target).await.map_err(|_| {
            AppError::new(
                "SWITCH_METADATA",
                "认证文件已切换，但最后使用时间保存失败；请刷新状态",
            )
        })?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
