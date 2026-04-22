# ChangeLog

## 2026-04-22

### 更新源切换到 `zekza/cc-switch-cli`

- 将编译元数据中的仓库地址改为 `https://github.com/zekza/cc-switch-cli`，让 `cc-switch update` 默认从这个 GitHub 仓库读取 release。
- 将 `install.sh` 的下载仓库改为 `zekza/cc-switch-cli`，保持脚本安装与 CLI 自更新使用同一发布源。
- 更新 release workflow、README 中的安装、手动下载、源码克隆和 issue 链接，统一对外指向新仓库。

### WebDAV 恢复补齐本地同步与代理重启

- `config webdav download` 和 `config webdav migrate-v1-to-v2` 现在会在恢复前捕获当前 proxy 运行状态与 takeover 状态。
- 如果本地 proxy 正在运行，恢复流程会先停掉当前 runtime，清掉 takeover 痕迹，完成 WebDAV 恢复后再按原模式自动重启。
- 恢复成功后会刷新 DB 配置快照，并把当前 provider 真正同步回本地 live 文件，不再只停留在界面展示。
- CLI 和 TUI 都改为走同一套 WebDAV 恢复包装流程，避免两边行为不一致。

### Managed proxy 重启挂起修复

- managed proxy 的 child reaper 不再绑定临时 tokio runtime。
- WebDAV 下载/迁移后自动重启 managed proxy 时，不会再因为等待 child reaper 而卡住命令返回。

### 回归测试

- 新增 WebDAV 下载后刷新 Claude live 文件的回归测试。
- 新增 WebDAV 下载后自动重启 proxy 并重新施加 takeover 的回归测试。
- 调整已有 proxy/WebDAV 测试清理逻辑，覆盖“runtime 已停但 takeover 痕迹仍在”的场景。
