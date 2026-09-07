# Codex Accounts

本地 Codex OAuth 多账号工作台。Tauri 2 + Rust + React 19 / TypeScript + Tailwind CSS 4 + Vite，不使用 Electron。

## 运行

环境：Node.js 20.19+（建议 22/24）、Rust stable、对应系统的 [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/)。

```bash
npm install
npm run tauri dev
```

- macOS：安装 Xcode Command Line Tools（`xcode-select --install`），首次启动允许访问应用自己的 Keychain 条目。
- Windows：安装 Microsoft C++ Build Tools、WebView2，使用 Credential Manager。
- Linux：安装 WebKitGTK 4.1 等 Tauri 系统依赖，以及运行中的 Secret Service（GNOME Keyring / KWallet 的 Secret Service 接口）、D-Bus。系统凭据服务不可用时明确报错，**不会退回明文凭据库**。
- `npm run dev` 仅启动 Vite，浏览器预览不具有后端能力；必须通过 Tauri 启动执行真实操作。

```bash
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
# 编译桌面可执行文件（第一版未启用安装包签名/分发）
npm run tauri build
```

## 使用

1. 点击「添加账号」，在系统浏览器登录 OpenAI。回调使用 `http://localhost:1455/auth/callback`，等待最多 3 分钟；应用内可以取消。
2. 登录完成后保存账号、查询额度；额度失败不会丢失已授权账号。相同 `accountId` 不重复导入，请使用「重新授权」。重新授权必须登录同一账号。
3. 启动及窗口重新获得焦点时识别当前环境，也可点击「识别状态」。未知账号可一键导入。
4. 「刷新全部额度」最多并行查询 4 个账号，单账号失败不影响其他账号。额度不足/接口异常不会偷偷切换账号。
5. 分别「设为 Codex」「设为 Pi Agent」，或「同时切换」。切换前建议暂停所有使用这些凭据的 Agent；现有进程可能缓存凭据，需要重启。
6. 删除只清除管理器账号库内的记录和凭据，不替用户退出正在使用的 CLI，不远程撤销授权。运行环境仍持有其认证副本，界面会显示为「未管理账号」。要完全退出，请另外使用对应 CLI 的 logout。

使用的路径已按更正统一：

```text
~/.codex/auth.json
~/.pi/agent/auth.json
```

第一版使用上述标准路径，不处理自定义 `CODEX_HOME`、`PI_CODING_AGENT_DIR` 或 Codex 的系统 Keyring 认证存储模式。若 Codex 设置了 `cli_auth_credentials_store = "keyring"`，请先改为文件认证存储（`"file"`），否则修改 auth.json 不能代表其实际认证来源。

## 架构与数据边界

```text
src/
  types/account.ts                无凭据 DTO / AppError
  services/tauri.ts                唯一 IPC 边界
  components/                     AccountCard / QuotaBar / CurrentAccountBadge
  pages/AccountsPage.tsx           列表、操作状态、错误、导入与登录取消
src-tauri/src/
  account/model.rs                AccountView 与 Credentials 分离；所有时间戳为毫秒
  account/repository.rs           持久化成功后才更新内存，删除同样具备失败回退
  account/service.rs              生命周期、每账号锁、导入/删除/重新授权
  oauth/                         独立的 PKCE / callback / refresh / protocol
  quota/provider.rs              QuotaProvider 扩展边界
  quota/codex.rs                 URL / headers / 额度结构适配
  runtime/mod.rs                RuntimeAdapter 接口与实时状态
  runtime/codex.rs               Codex auth.json 格式
  runtime/pi_agent.rs            仅合并 openai-codex 节点
  switch/service.rs              全局切换锁、全部准备、提交、条件回滚
  storage/secure_store.rs         SecureStore + 系统密钥 + 加密凭据库
  storage/atomic.rs               同目录临时文件、fsync、原子替换
  storage/config.rs               标准路径 / 后续普通 UI 配置边界
  commands/                      Tauri command 层
  tests.rs                       离线集成测试，仅使用临时目录
```

内部 `Account` 包含无凭据 `AccountView` 与 `Credentials`，额外保留 Codex 所需的 `id_token`。前端永远只能获得 `AccountView`；不返回 access/refresh/id token，也不返回 OAuth URL 或 authorization code。前端没有 HTTP、文件插件或 localStorage 凭据存储。

未来自动选择账号可独立添加策略模块：先过滤所有窗口均超过阈值且额度未过时的账号，再按最低剩余额度、平均剩余额度、最久未使用排序，最后复用 `AccountService::switch`。本版不执行任何自动账号切换，不将策略耦合到 Provider/Adapter。

## 凭据安全

- 应用数据目录由 Tauri 提供，标识为 `dev.local.codex-accounts`。macOS 通常为 `~/Library/Application Support/dev.local.codex-accounts/`。
- 系统 Keychain / Credential Manager / Secret Service 保存随机 256-bit 主密钥（service `dev.local.codex-accounts`，entry `vault-key-v1`）。
- `credentials.v1.enc` 使用 XChaCha20-Poly1305 加密；每次写入使用独立随机 192-bit nonce。账号库不与普通 UI 配置混合，无明文 fallback。
- Unix 下应用私有目录为 `0700`，临时文件/替换后的认证文件为 `0600`。Windows 使用用户目录继承的 ACL，请勿将用户目录开放给其他用户。
- 敏感 Credentials 不实现 Debug，释放时 zeroize；序列化明文和主密钥使用 Zeroizing。未接入普通 Token/HTTP body 日志，错误仅包含固定信息、HTTP 状态码或路径/OS 错误类别。其他临时 JSON/HTTP 缓冲区不承诺内存完全清零。
- 凭据只请求 OpenAI OAuth / ChatGPT usage 服务；不上传到自建服务器，不包含遥测。禁止 HTTP 跟随重定向，避免凭据被带到其他站点。
- 操作系统认证文件按 CLI 约定仍然是明文。这是运行环境兼容需求，不代表加密账号库失效；本机同用户进程仍可能读取运行环境文件。
- 不记录历史明文 auth 备份；正常失败时临时文件自动删除。强制终止进程可能留下 0600 临时文件，需要在对应目录手动确认清理。SSD / 文件系统快照不提供安全擦除保证。
- 请勿只删除 Keychain 密钥后保留加密库，这会导致无法解密。完整重置需同时处理应用数据目录与应用密钥；此操作会永久丢失管理器账号。

## Token 生命周期与并发

1. 管理器实例启动时获取文件 advisory lock，阻止第二个管理器访问同一账号库。
2. 同账号 quota / refresh / switch / delete / reauthorize 使用同一个账号互斥锁。锁顺序固定为 `Switch → Account Token → Repository`。
3. 查询或切换前读取账号并识别运行环境内严格更新的 JWT（优先比较 `iat`，否则比较过期时间），回写 CLI 可能轮换的新凭据。无法判断新旧的分歧报 `RUNTIME_DESYNC`。
4. Token 剩余不足 120 秒才刷新；额度返回 401 时最多额外强制刷新一次。刷新 POST 不自动重试。
5. Refresh Token rotation 必须先持久化再释放账号锁。新 refresh_token 缺失时保留旧值；有新值则替换。持久化失败时，内存暂存轮换结果，后续操作先重试保存，绝不重新使用旧 Refresh Token。
6. `ROTATION_SAVE_FAILED` 时不要关闭应用，先修复权限/磁盘空间再重试。如果此时崩溃或被强制关闭，新的轮换结果可能丢失，需要重新授权。远端轮换与本地磁盘之间不存在分布式事务。

**边界：**应用内锁无法阻止官方 Codex / Pi 自己刷新 Token。这些客户端不共享管理器的 per-account mutex。多个活跃客户端共享一条 Refresh Token 链仍可能冲突；建议暂停后切换，不在多个长时间运行的进程同时使用同一账号。若 refresh 请求在远端已成功但网络断开，不能安全重试旧 token，应重新授权。

## 文件切换保证与限制

- 读取并校验原 JSON → 同步退出账号凭据 → 准备目标 Token → 生成/校验所有目标 JSON → 同文件系统临时文件 → `fsync` → 原子替换。
- Pi 保留其他 Provider 以及未知 JSON 字段（保留语义，不承诺原来的空白格式）。
- Codex 是完整的当前 OAuth 认证文件；API-key 登录模式会被 OAuth 模式替换。由 Pi 导入的账号可能没有 ID Token，切 Codex 前尝试刷新获取，若仍缺失则要求重新授权，**不写入不可用的 null ID Token 文件**。
- 准备失败不修改任何目标认证文件。双切换第二次提交失败时，第一份只有仍等于本次写入内容才会回滚；外部已更新则不覆盖，返回 `SWITCH_PARTIAL`。
- 两个文件不能进行真正的跨文件原子事务。进程在两次 rename 之间崩溃可能只切换一个环境；启动/错误后重新读取两份文件并显示实际状态，不保存容易失真的 currentAccountId 缓存。
- 提交前做内容冲突检查，但外部程序不共享锁时，检查与 rename 之间仍有 TOCTOU 窗口；因此不能承诺与外部 CLI 并发写入绝对安全。运行中切换不提供会话热迁移。

## 易变化的上游协议

- `oauth/protocol.rs`：OpenAI Codex public client ID、授权和 Token endpoint、JWT claim 路径；`oauth/login.rs`：授权参数。
- `quota/codex.rs`：`https://chatgpt.com/backend-api/wham/usage` 及 `Authorization` / `ChatGPT-Account-Id` / `OpenAI-Beta` headers。
- 额度 `used_percent = 35` 显示剩余 65%；空窗口显示未知，不虚构为 100%。已知窗口秒数不是 5H/Weekly 时返回 schema 错误而不是错标窗口。重置时间转毫秒。
- `runtime/codex.rs` / `runtime/pi_agent.rs`：运行环境认证格式。

这些接口不是稳定公共 API，可能受到账号权限、地区、计划及上游修改影响。JWT 只解码作元数据，不把未校验 JWT 当身份验证证明；真正 token 交换通过 OpenAI HTTPS 服务。导入本地账号同样不意味着其凭据一定有效。

## Commands

`list_accounts`、`start_oauth_login`（可传重新授权的 accountId）、`cancel_oauth_login`、`delete_account`、`refresh_account`、`refresh_all_quotas`、`switch_codex_account`、`switch_pi_account`、`switch_both`、`get_runtime_status`、`import_current_codex_account`、`import_current_pi_account`。

所有错误遵循 `{ code, message, detail? }`。批量查询逐账号返回错误；重新授权/添加成功后的额度错误单独返回，不伪装成 OAuth 失败。

## 验证范围

离线测试覆盖 state/重复参数、额度 schema 与百分比、Pi Provider 保留、原子替换、双文件失败回滚/拒绝覆盖外部修改、并发刷新单次 rotation、rotation 落盘失败重试、并发切换、独立运行环境、删除回退、CLI 新 Token 回写、IPC DTO 不泄露 Token。

真实 OpenAI OAuth 登录、实际账号额度以及 CLI 消费新认证文件需要用户授权后验收，离线测试不能替代这些验证。macOS 为当前编译验证环境；Windows/Linux 需要各平台依赖及实机验证。
