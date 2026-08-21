'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const root = path.resolve(__dirname, '..');
const html = fs.readFileSync(path.join(root, 'desktop-sample', 'ui', 'index.html'), 'utf8');
const script = fs.readFileSync(path.join(root, 'desktop-sample', 'ui', 'app.js'), 'utf8');
const styles = fs.readFileSync(path.join(root, 'desktop-sample', 'ui', 'styles.css'), 'utf8');
const cargo = fs.readFileSync(path.join(root, 'desktop-sample', 'src-tauri', 'Cargo.toml'), 'utf8');
const network = fs.readFileSync(
  path.join(root, 'desktop-sample', 'src-tauri', 'src', 'network.rs'),
  'utf8',
);
const github = fs.readFileSync(
  path.join(root, 'desktop-sample', 'src-tauri', 'src', 'github.rs'),
  'utf8',
);
const adapterIcons = fs.readFileSync(
  path.join(root, 'desktop-sample', 'src-tauri', 'src', 'adapters', 'icons.rs'),
  'utf8',
);
const adapterMod = fs.readFileSync(
  path.join(root, 'desktop-sample', 'src-tauri', 'src', 'adapters', 'mod.rs'),
  'utf8',
);
const desktopMain = fs.readFileSync(
  path.join(root, 'desktop-sample', 'src-tauri', 'src', 'main.rs'),
  'utf8',
);
const extensions = fs.readFileSync(
  path.join(root, 'desktop-sample', 'src-tauri', 'src', 'extensions.rs'),
  'utf8',
);
const extensionTargets = fs.readFileSync(
  path.join(root, 'desktop-sample', 'src-tauri', 'src', 'extensions', 'targets.rs'),
  'utf8',
);
const extensionHealth = fs.readFileSync(
  path.join(root, 'desktop-sample', 'src-tauri', 'src', 'extensions', 'health.rs'),
  'utf8',
);
const extensionHistory = fs.readFileSync(
  path.join(root, 'desktop-sample', 'src-tauri', 'src', 'extensions', 'history.rs'),
  'utf8',
);
const extensionSecurity = fs.readFileSync(
  path.join(root, 'desktop-sample', 'src-tauri', 'src', 'extensions', 'security.rs'),
  'utf8',
);
const extensionTransfer = fs.readFileSync(
  path.join(root, 'desktop-sample', 'src-tauri', 'src', 'extensions', 'transfer.rs'),
  'utf8',
);
const market = fs.readFileSync(
  path.join(root, 'desktop-sample', 'src-tauri', 'src', 'market.rs'),
  'utf8',
);
const release = fs.readFileSync(
  path.join(root, 'desktop-sample', 'src-tauri', 'src', 'release.rs'),
  'utf8',
);
const sessions = fs.readFileSync(
  path.join(root, 'desktop-sample', 'src-tauri', 'src', 'sessions.rs'),
  'utf8',
);
const readme = fs.readFileSync(path.join(root, 'README.md'), 'utf8');
const desktopReadme = fs.readFileSync(path.join(root, 'desktop-sample', 'README.md'), 'utf8');
const securityCheck = fs.readFileSync(path.join(root, 'scripts', 'security-check.js'), 'utf8');
const buildWorkflow = fs.readFileSync(path.join(root, '.github', 'workflows', 'build.yml'), 'utf8');
const cursorCompatWorkflow = fs.readFileSync(path.join(root, '.github', 'workflows', 'cursor-compat.yml'), 'utf8');
const cursorCompatMacosWorkflow = fs.readFileSync(
  path.join(root, '.github', 'workflows', 'cursor-compat-macos.yml'),
  'utf8',
);
const cursorReleaseMacos = fs.readFileSync(
  path.join(root, 'scripts', 'get-cursor-release-macos.sh'),
  'utf8',
);

test('desktop UI exposes usage and backup history controls', () => {
  for (const id of [
    'refreshUsageButton',
    'usageContent',
    'usageModelList',
    'usageDailyTab',
    'usageEventsTab',
    'usageDayList',
    'usageEventList',
    'refreshSessionsButton',
    'sessionProcessList',
    'sessionConsentCheckbox',
    'sessionKillRemoteButton',
    'sessionDetachChatsButton',
    'sessionChatList',
    'backupHistoryList',
    'restoreConsentCheckbox',
    'backupRestoreProgress',
    'cursorCompatibility',
    'claudeCompatibility',
  ]) {
    assert.match(html, new RegExp(`id=["']${id}["']`));
  }
  assert.match(script, /invoke\("cursor_usage"\)/);
  assert.match(script, /invoke\("cursor_sessions"\)/);
  assert.match(script, /invoke\("manage_cursor_session"/);
  assert.match(script, /confirm: action !== "refresh"/);
  assert.match(script, /function canUseLocalPrivileges\(/);
  assert.match(script, /detach-chats/);
  assert.match(script, /function renderSessions\(/);
  assert.match(script, /data-usage-tab/);
  assert.match(desktopMain, /mod sessions;/);
  assert.match(desktopMain, /cursor_sessions,/);
  assert.match(desktopMain, /manage_cursor_session,/);
  const usageRs = fs.readFileSync(path.join(root, 'desktop-sample', 'src-tauri', 'src', 'usage.rs'), 'utf8');
  const sessionsRs = fs.readFileSync(path.join(root, 'desktop-sample', 'src-tauri', 'src', 'sessions.rs'), 'utf8');
  const chatsRs = fs.readFileSync(path.join(root, 'desktop-sample', 'src-tauri', 'src', 'chats.rs'), 'utf8');
  assert.match(usageRs, /api\/dashboard\/get-filtered-usage-events/);
  assert.match(usageRs, /Origin.*cursor\.com/);
  assert.match(usageRs, /fn classify_pool/);
  assert.match(sessionsRs, /kill-remote/);
  assert.match(sessionsRs, /remote-control/);
  assert.match(sessionsRs, /detach-chats/);
  assert.match(chatsRs, /createdFromBackgroundAgent/);
  assert.match(chatsRs, /isArchived/);
  assert.match(chatsRs, /fn detach_stuck_chats/);
  assert.match(chatsRs, /misclassified/);
  assert.match(chatsRs, /fn is_live_status/);
  assert.match(chatsRs, /fn is_finished_status/);
  assert.match(chatsRs, /fn composer_index_fields/);
  assert.match(script, /function chatStateLabel\(/);
  assert.match(sessionsRs, /is_workbench_process/);
  assert.match(script, /invoke\("list_backups"\)/);
  assert.match(script, /backupVersion:\s*record\.version/);
  assert.match(script, /function runBackupRestore\(/);
  assert.match(script, /modalCompleted/);
  assert.match(script, /"完成"/);
  assert.match(script, /modalCompletedAction === "restore"/);
  assert.match(script, /app\.compatibilityMessage/);
  assert.match(script, /app\.autoCompatible === false/);
  const backupBody = script.slice(
    script.indexOf('async function runBackup(appId)'),
    script.indexOf('async function runBackupRestore(recordId)'),
  );
  const actionBody = script.slice(
    script.indexOf('async function runAction(action)'),
    script.indexOf('async function registerProgressListener()'),
  );
  assert.doesNotMatch(backupBody, /modalCompletedAction\s*=\s*action/);
  assert.match(actionBody, /modalCompletedAction\s*=\s*action/);
  assert.match(styles, /\.backup-history-row/);
  assert.match(styles, /\.usage-model-row/);
  assert.match(styles, /\.usage-day-row/);
  assert.match(styles, /\.session-process-row/);
  assert.match(styles, /\.session-chat-row/);
});

test('desktop UI exposes About, GitHub and optional update checks', () => {
  for (const id of [
    'about',
    'updateStatusCard',
    'updateState',
    'updateCurrentVersion',
    'updateLatestVersion',
    'checkUpdateButton',
    'downloadUpdateButton',
    'updateDownloadProgress',
    'updateDownloadProgressText',
    'updateDownloadProgressValue',
    'updateDownloadProgressBar',
    'viewUpdateButton',
    'githubAvatar',
    'githubProjectsState',
    'githubProjectsGrid',
    'refreshProjectsButton',
    'reviewConsentButton',
  ]) {
    assert.match(html, new RegExp(`id=["']${id}["']`));
  }
  assert.match(html, /github\.com\/lilicocon\/cursor-i18n-zh/);
  assert.match(html, /github\.com\/lilicocon\.png\?size=160/);
  assert.match(html, /86jp_DfoGmTool/);
  assert.match(html, /检查时不自动下载、不静默安装、不强制更新/);
  assert.match(html, /下载并安装更新/);
  assert.match(script, /invoke\("check_for_updates"\)/);
  assert.match(script, /invoke\("github_projects"\)/);
  assert.match(script, /86jp_DfoGmTool/);
  assert.match(release, /api\.github\.com\/repos\/lilicocon\/cursor-i18n-zh\/releases\/latest/);
  assert.match(release, /PROJECT_REPOSITORY_URL: &str = "https:\/\/github\.com\/lilicocon\/cursor-i18n-zh"/);
  assert.match(release, /PROJECT_RELEASES_URL: &str = "https:\/\/github\.com\/lilicocon\/cursor-i18n-zh\/releases"/);
  assert.match(script, /invoke\("open_github_url"/);
  assert.match(script, /invoke\("open_project_page"/);
  assert.match(script, /function renderGitHubProjects\(/);
  assert.match(script, /dataset\.projectUrl = project\.htmlUrl/);
  assert.match(script, /前往 Star/);
  assert.match(script, /登录后点击右上角 Star/);
  assert.match(script, /content\.classList\.toggle\("about-mode", aboutMode\)/);
  assert.match(styles, /\.content:not\(\.about-mode\) > #about/);
  assert.match(styles, /\.content\.about-mode > :not\(#about\)/);
  assert.match(styles, /\.github-project-grid/);
  assert.match(styles, /\.github-project-card/);
  assert.match(styles, /\.project-star-button/);
});

test('desktop GitHub project feed is public, sorted and URL restricted', () => {
  assert.match(github, /api\.github\.com\/users\/lilicocon\/repos/);
  assert.match(github, /PINNED_REPOSITORIES: &\[&str\] = &\["86jp_DfoGmTool"\]/);
  assert.match(github, /right\s*\.stars\s*\.cmp\(&left\.stars\)/);
  assert.match(github, /!repository\.fork/);
  assert.match(github, /!repository\.archived/);
  assert.match(github, /projects\.truncate\(MAX_PROJECTS\)/);
  assert.match(github, /https:\/\/github\.com\/lilicocon\//);
  assert.match(desktopMain, /async fn github_projects\(/);
  assert.match(desktopMain, /fn open_github_url\(/);
  assert.match(desktopMain, /github::is_safe_project_url\(&url\)/);
  assert.doesNotMatch(`${html}\n${script}\n${github}`, /github[_-]?token/i);
  assert.match(network, /fn github_api_error\(/);
  assert.match(network, /GitHub 公开接口已被限流/);
  assert.match(github, /network::github_api_error\(error, "项目接口"\)/);
  assert.match(release, /network::github_api_error\(other, "版本接口"\)/);
  assert.match(market, /network::github_api_error\(other, "市场接口"\)/);
  assert.match(script, /不影响汉化、备份等本地功能/);
  assert.match(script, /function browserFallbackGitHubProjects\(/);
});

test('desktop UI manages Cursor and Claude Code MCP, Skills, prompts and market', () => {
  for (const id of [
    'extensions',
    'extensionWorkspaceControl',
    'extensionMcpList',
    'extensionSkillList',
    'extensionPromptList',
    'extensionMarketList',
    'extensionHistoryList',
    'extensionTransferPanel',
    'extensionTargetMeta',
    'extensionActivityBanner',
    'addMcpButton',
    'addSkillButton',
    'addPromptButton',
    'refreshMarketButton',
    'checkAllMcpButton',
    'refreshExtensionHistoryButton',
    'previewExtensionCopyButton',
    'chooseExtensionImportButton',
    'previewSelectedImportButton',
    'extensionExportPassword',
    'extensionExportPasswordConfirm',
    'extensionImportPassword',
    'mcpEditorBackdrop',
    'skillEditorBackdrop',
    'promptEditorBackdrop',
    'mcpEnvInput',
    'mcpHeadersInput',
    'skillContentInput',
    'promptContentInput',
  ]) {
    assert.match(html, new RegExp(`id=["']${id}["']`));
  }
  for (const command of [
    'extension_inventory',
    'extension_mcp_details',
    'extension_save_mcp',
    'extension_toggle_mcp',
    'extension_delete_mcp',
    'extension_skill_details',
    'extension_save_skill',
    'extension_toggle_skill',
    'extension_delete_skill',
    'extension_prompt_details',
    'extension_save_prompt',
    'extension_toggle_prompt',
    'extension_delete_prompt',
    'extension_market',
    'extension_install_market_item',
    'extension_targets',
    'extension_check_mcp',
    'extension_history',
    'extension_restore_history',
    'extension_export_bundle',
    'extension_preview_import',
    'extension_import_bundle',
    'extension_preview_copy',
    'extension_copy',
    'extension_batch_toggle',
    'choose_extension_bundle_path',
    'choose_extension_workspace',
  ]) {
    assert.match(script, new RegExp(`invoke\\("${command}"`));
    assert.match(desktopMain, new RegExp(command));
  }
  assert.match(script, /content\.classList\.toggle\("extensions-mode", extensionMode\)/);
  assert.match(script, /••••••/);
  assert.match(styles, /\.extension-item-card/);
  assert.match(styles, /\.extension-editor-modal/);
  assert.match(extensionTargets, /home\.join\("\.cursor\/mcp\.json"\)/);
  assert.match(extensionTargets, /home\.join\("\.claude\.json"\)/);
  assert.match(extensionTargets, /workspace\.join\("\.mcp\.json"\)/);
  assert.match(extensionTargets, /home\.join\("\.cursor\/skills-cursor"\)/);
  assert.match(extensionTargets, /home\.join\("\.cursor\/rules"\)/);
  assert.match(extensionTargets, /home\.join\("\.claude\/rules"\)/);
  assert.match(extensions, /REDACTED_VALUE/);
  assert.match(extensionHealth, /"method": "initialize"/);
  assert.match(extensionHistory, /MAX_HISTORY_RECORDS/);
  assert.match(extensionHistory, /restore_snapshot/);
  assert.match(extensionSecurity, /missing_markdown_references/);
  assert.match(extensionSecurity, /has_shell_commands/);
  assert.match(extensionTransfer, /MAX_BUNDLE_BYTES/);
  assert.match(extensionTransfer, /set_private_permissions/);
  assert.match(extensionTransfer, /Argon2id/);
  assert.match(extensionTransfer, /Aes256Gcm/);
  assert.match(extensionTransfer, /ENCRYPTED_AAD/);
  assert.match(extensionTransfer, /\.zeroize\(\)/);
  assert.match(extensionTransfer, /Zeroizing/);
  assert.match(extensionTransfer, /包含密钥的配置包必须使用密码加密导出/);
  assert.match(extensionTargets, /ExtensionTargetDescriptor/);
  assert.match(script, /loadExtensionTargets/);
  assert.match(extensions, /extension-registry/);
  assert.match(extensions, /install_skill_bundle/);
  assert.match(market, /MAX_TOTAL_BYTES/);
  assert.match(market, /fetch_repository_directory/);
  assert.match(market, /official.*verified.*community/);
  assert.match(market, /allow_overwrite_modified/);
  assert.match(market, /install_market_mcp/);
  assert.match(extensions, /install_market_skill_bundle/);
  assert.match(extensions, /install_market_prompt_with_origin/);
  assert.match(cargo, /windows-sys/);
  assert.match(market, /api\.github\.com\/repos\/\{slug\}\/commits/);
  assert.match(market, /raw\.githubusercontent\.com/);
  assert.match(buildWorkflow, /runs-on: macos-14/);
  assert.match(buildWorkflow, /package-macos\.sh/);
  assert.match(buildWorkflow, /windows-11-arm/);
  assert.match(buildWorkflow, /aarch64-pc-windows-msvc/);
  assert.match(buildWorkflow, /x86_64-pc-windows-msvc/);
  assert.match(buildWorkflow, /aarch64-apple-darwin/);
  assert.match(buildWorkflow, /x86_64-apple-darwin/);
  assert.match(buildWorkflow, /localization-workbench-windows-x64/);
  assert.match(buildWorkflow, /localization-workbench-windows-arm64/);
  assert.match(buildWorkflow, /localization-workbench-macos-arm64/);
  assert.match(buildWorkflow, /localization-workbench-macos-x64/);
  assert.match(desktopMain, /target_os = "macos"/);
});

test('desktop release flow downloads verified optional updates and scans publish artifacts', () => {
  const selfUpdate = fs.readFileSync(
    path.join(root, 'desktop-sample', 'src-tauri', 'src', 'self_update.rs'),
    'utf8',
  );
  assert.match(script, /invoke\("install_latest_update"\)/);
  assert.match(script, /invoke\("quit_for_update"\)/);
  assert.match(script, /invoke\("open_downloaded_update"/);
  assert.match(desktopMain, /mod self_update;/);
  assert.match(desktopMain, /async fn download_latest_update\(/);
  assert.match(desktopMain, /async fn install_latest_update\(/);
  assert.match(desktopMain, /fn quit_for_update\(/);
  assert.match(selfUpdate, /fn apply_downloaded_update/);
  assert.match(selfUpdate, /local_app_data\(\)/);
  assert.match(selfUpdate, /Expand-Archive/);
  assert.match(selfUpdate, /CREATE_NO_WINDOW/);
  assert.match(selfUpdate, /\$appPid/);
  assert.match(selfUpdate, /Remove-Item -LiteralPath \$cache/);
  assert.match(selfUpdate, /rm -rf \\"\$cache\\"/);
  assert.match(selfUpdate, /hdiutil/);
  assert.match(selfUpdate, /cursor-i18n-desktop-sample\.exe/);
  assert.match(selfUpdate, /\/Volumes\//);
  assert.doesNotMatch(selfUpdate, /tauri-plugin-updater/);
  assert.doesNotMatch(selfUpdate, /dirs::/);
  assert.doesNotMatch(cargo, /tauri-plugin-updater/);
  assert.match(release, /SHA256SUMS-macos\.txt/);
  assert.match(release, /macos-arm64\.dmg/);
  assert.match(release, /macos-x64\.dmg/);
  assert.match(release, /windows-arm64\.zip/);
  assert.match(release, /windows-x64\.zip/);
  assert.match(release, /target_arch = "aarch64"/);
  assert.match(release, /with_config\(\)\s*\.limit/);
  assert.match(release, /fn download_file\(/);
  assert.match(release, /\[0_u8; 64 \* 1024\]/);
  assert.match(release, /fn sha256_file\(/);
  assert.match(release, /fn commit_download\(/);
  assert.match(release, /pub cached: bool/);
  assert.match(script, /result\.cached/);
  assert.match(script, /update-download-progress/);
  assert.match(script, /setUpdateDownloadProgress/);
  assert.match(script, /updateProgressHideTimer/);
  assert.match(script, /更新包已完成校验, 但无法打开所在目录/);
  assert.match(script, /requestedBrowserUpdateProgress/);
  assert.match(desktopMain, /UpdateDownloadProgress/);
  assert.match(desktopMain, /app\.emit\(\s*"update-download-progress"/);
  assert.match(release, /releases\/download\//);
  assert.match(securityCheck, /cursor-session/);
  assert.match(securityCheck, /screenshot-email/);
  assert.match(buildWorkflow, /Scan sensitive information/);
  assert.match(buildWorkflow, /WINDOWS_CERTIFICATE/);
  assert.match(buildWorkflow, /Get-AuthenticodeSignature/);
  assert.match(buildWorkflow, /actions\/cache\/restore@v5/);
  assert.match(buildWorkflow, /actions\/cache\/save@v5/);
});

test('desktop UI provides accessible focus, keyboard navigation and long-operation feedback', () => {
  assert.match(html, /id="extensionActivityBanner"[^>]*role="status"[^>]*aria-live="polite"/);
  assert.match(html, /data-extension-tab="mcp"[^>]*aria-selected="true"/);
  assert.match(html, /id="toast"[^>]*role="status"[^>]*aria-live="polite"/);
  assert.match(script, /function setExtensionActivity\(/);
  assert.match(script, /setAttribute\("aria-busy", String\(active\)\)/);
  assert.match(script, /\["ArrowLeft", "ArrowRight", "Home", "End"\]/);
  assert.match(script, /event\.key === "Escape"/);
  assert.match(script, /event\.metaKey \|\| event\.ctrlKey/);
  assert.match(html, /id="zoomButton"/);
  assert.match(html, /class="traffic-lights"/);
  assert.match(script, /\$\("#zoomButton"\)/);
  assert.match(script, /dblclick/);
  assert.match(script, /requestAnimationFrame\(\(\) => \$\("#mcpNameInput"\)\.focus\(\)\)/);
  assert.match(styles, /button:focus-visible/);
  assert.match(styles, /\.extension-activity-banner/);
  assert.match(styles, /\.extension-section\.is-busy/);
  assert.match(styles, /animation-duration: \.01ms !important/);
});

test('desktop UI gates first launch before local or network initialization', () => {
  for (const id of [
    'firstRunBackdrop',
    'firstRunTitle',
    'firstRunConsentCheckbox',
    'firstRunAcceptButton',
    'firstRunExitButton',
    'firstRunCloseButton',
  ]) {
    assert.match(html, new RegExp(`id=["']${id}["']`));
  }
  assert.match(html, /软件声明/);
  assert.match(html, /隐私说明/);
  assert.match(script, /i18nWorkbench\.firstRunConsent\.v2/);
  assert.match(script, /invoke\("has_first_run_consent"\)/);
  assert.match(script, /invoke\("accept_first_run_consent"\)/);
  assert.match(desktopMain, /fn has_first_run_consent/);
  assert.match(desktopMain, /fn accept_first_run_consent/);
  assert.match(desktopMain, /adapters::require_first_run_consent\(\)\?/);
  assert.match(adapterMod, /first-run-consent/);
  assert.match(script, /if \(!browserPreviewSection\) await waitForFirstRunConsent\(\);\s*await refreshEnvironmentAndApps\(\);/);
  assert.match(script, /function canUseLocalPrivileges\(/);
  assert.match(script, /if \(!canUseLocalPrivileges\(\)\) return;/);
  assert.match(script, /function appNeedsElevation\(/);
  assert.match(script, /function isProtectedAppInstall\(/);
  assert.doesNotMatch(script, /app\.id === "claude" \|\| state\.environment\.platform === "macos"/);
  assert.match(sessions, /QUIT_FORCE_ATTEMPTS: u32 = 4/);
  assert.match(readme, /Cursor 3\.16/);
  assert.doesNotMatch(readme, /3\.11\.19/);
  assert.doesNotMatch(desktopReadme, /Universal 双架构/);
  assert.match(script, /get\("preview"\)/);
  assert.match(script, /\["about", "extensions"\]\.includes\(requestedBrowserPreview\)/);
  assert.ok(
    script.indexOf('await waitForFirstRunConsent();')
      < script.indexOf('await Promise.all([loadUsage(), loadSessions(), loadUpdateStatus({ notify: true })]);'),
  );
});

test('desktop network uses the Windows trusted certificate chain', () => {
  assert.match(cargo, /"platform-verifier"/);
  assert.match(cargo, /"win-system-proxy"/);
  assert.match(network, /RootCerts::PlatformVerifier/);
  assert.doesNotMatch(network, /disable_verification\s*\(\s*true\s*\)/);
  assert.match(network, /pub fn with_retry/);
  assert.match(network, /500 \| 502 \| 503 \| 504/);
  assert.match(github, /network::with_retry/);
  assert.match(market, /network::with_retry/);
  assert.match(release, /network::with_retry/);
});

test('desktop UI exposes Node.js 18 runtime detection', () => {
  for (const id of [
    'nodeRuntimeCard',
    'nodeRuntimeState',
    'nodeRuntimeVersion',
    'nodeRuntimeRequired',
    'nodeRuntimePath',
    'nodeRuntimeRefreshButton',
  ]) {
    assert.match(html, new RegExp(`id=["']${id}["']`));
  }
  assert.match(html, /仅 Cursor 汉化功能需要/);
  assert.match(html, /Node\.js 18\+/);
  assert.match(html, /node-logo\.png/);
  assert.match(html, /mcp-logo\.png/);
  assert.match(script, /function applyBrandMark\(/);
  assert.match(script, /invoke\("environment_status"\)/);
  assert.match(script, /function refreshEnvironmentAndApps\(/);
});

test('software center loads local app icons and keeps original light fallbacks', () => {
  assert.match(html, /class="app-logo cursor-logo"/);
  assert.match(html, /class="app-logo claude-logo has-local-icon"/);
  assert.match(html, /id="appGrid"/);
  assert.match(html, /id="cursorState"/);
  assert.match(html, /id="claudeState"/);
  assert.match(script, /function applyAppLogoElement\(/);
  assert.match(script, /function syncAppLogos\(/);
  assert.match(script, /app\.iconDataUrl/);
  assert.match(script, /data:image\/png;base64,/);
  assert.match(adapterMod, /pub icon_data_url: Option<String>/);
  assert.match(adapterIcons, /fn data_url_for_cursor\(/);
  assert.match(adapterIcons, /fn data_url_for_claude\(/);
  assert.match(adapterIcons, /fn extract_png_from_icns\(/);
  assert.match(adapterIcons, /fn extract_png_from_ico\(/);
  assert.doesNotMatch(adapterIcons, /include_bytes!\s*\(/);
  assert.doesNotMatch(adapterIcons, /include_str!\s*\(/);
  assert.match(styles, /\.app-logo\.has-local-icon/);
  assert.match(styles, /\.cursor-logo\s*\{[^}]*var\(--primary\)/);
  assert.match(styles, /\.claude-logo\s*\{[^}]*#eef2ff/);
  assert.doesNotMatch(styles, /\.cursor-logo\s*\{[^}]*#27272a/);
  assert.doesNotMatch(styles, /\.claude-logo\s*\{[^}]*#d3845c/);
  assert.match(script, /claude-logo\.png/);
  const extensionTargetSlice = script.slice(
    script.indexOf('const segment = $("#extensionTargetSegment")'),
    script.indexOf('function renderExtensionHistory'),
  );
  assert.doesNotMatch(extensionTargetSlice, /iconDataUrl/);
  const uiImages = fs.readdirSync(path.join(root, 'desktop-sample', 'ui'))
    .filter((name) => /\.(png|ico|icns|svg|webp)$/i.test(name))
    .sort();
  assert.deepEqual(uiImages, ['app-icon.png', 'claude-logo.png', 'mcp-logo.png', 'node-logo.png']);
  assert.match(
    fs.readFileSync(path.join(root, 'desktop-sample', 'resources', 'brand-icons', 'SOURCE.md'), 'utf8'),
    /nodedotjs\.svg/,
  );
  assert.match(
    fs.readFileSync(path.join(root, 'desktop-sample', 'resources', 'brand-icons', 'SOURCE.md'), 'utf8'),
    /favicon\.svg/,
  );
  assert.ok(!fs.existsSync(path.join(root, 'desktop-sample', 'ui', 'Cursor.icns')));
  assert.match(
    fs.readFileSync(path.join(root, 'desktop-sample', 'resources', 'claude-icon', 'SOURCE.md'), 'utf8'),
    /Claude_AI_symbol\.svg/,
  );
});

test('desktop frontend never receives or renders Cursor credentials', () => {
  const frontend = `${html}\n${script}\n${styles}`;
  assert.doesNotMatch(frontend, /accessToken/i);
  assert.doesNotMatch(frontend, /WorkosCursorSessionToken/i);
  assert.doesNotMatch(frontend, /Authorization\s*:/i);
});

test('desktop and package versions are synchronized', () => {
  const { check } = require(path.join(root, 'scripts', 'bump-version.js'));
  const packageJson = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8'));
  const tauriConfig = JSON.parse(fs.readFileSync(
    path.join(root, 'desktop-sample', 'src-tauri', 'tauri.conf.json'),
    'utf8',
  ));
  assert.equal(check(), packageJson.version);
  assert.equal(tauriConfig.identifier, 'com.licocon.i18n-workbench');
  assert.ok(cargo.split(/\r?\n/).includes('authors = ["licocon"]'));
});

test('release playbook is the only bump path', () => {
  const skill = fs.readFileSync(path.join(root, '.cursor', 'skills', 'release', 'SKILL.md'), 'utf8');
  const agents = fs.readFileSync(path.join(root, 'AGENTS.md'), 'utf8');
  assert.match(skill, /node scripts\/bump-version\.js NEW/);
  assert.match(skill, /git tag -a vNEW/);
  assert.match(agents, /\.cursor\/skills\/release\/SKILL\.md/);
});

test('Cursor compatibility workflow bounds and cleans silent installer execution', () => {
  assert.match(cursorCompatWorkflow, /compatibility:\s*[\s\S]*?timeout-minutes:\s*45/);
  assert.match(cursorCompatWorkflow, /Download and install official Cursor build\s*\n\s*timeout-minutes:\s*15/);
  assert.match(cursorCompatWorkflow, /Start-Process[^\n]+-PassThru\s*$/m);
  assert.doesNotMatch(cursorCompatWorkflow, /Start-Process -FilePath \$installer[^\n]+-Wait/);
  for (const flag of ['/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-', '/NOICONS', '/CURRENTUSER']) {
    assert.ok(cursorCompatWorkflow.includes(`'${flag}'`));
  }
  assert.match(cursorCompatWorkflow, /"\/DIR=\$installRoot"/);
  assert.match(cursorCompatWorkflow, /"\/LOG=\$installerLog"/);
  assert.match(cursorCompatWorkflow, /Detected installed Cursor identity/);
  assert.match(cursorCompatWorkflow, /Get-Content[^\n]+\$installerLog -Tail 160/);
  assert.match(cursorCompatWorkflow, /\[DateTime\]::UtcNow\.AddMinutes\(12\)/);
  assert.match(cursorCompatWorkflow, /candidate\.version[^\n]+release\.version/);
  assert.match(cursorCompatWorkflow, /candidate\.commit -match '\^\[0-9a-f\]\{40\}\$'/);
  assert.match(cursorCompatWorkflow, /differs from signed installer product commit/);
  assert.match(cursorCompatWorkflow, /Start-Process -FilePath taskkill\.exe[^\n]+-Wait -PassThru/);
  assert.match(cursorCompatWorkflow, /cleanup\.ExitCode -ne 0/);
  assert.match(cursorCompatWorkflow, /needs\.compatibility\.result != 'success'/);
  assert.match(cursorCompatWorkflow, /resolve-failure:[\s\S]+needs\.compatibility\.result == 'success'/);
  assert.match(cursorCompatWorkflow, /Close resolved compatibility issue/);
  assert.match(cursorCompatWorkflow, /state_reason: 'completed'/);
  assert.match(cursorCompatWorkflow, /status === 410/);
});

test('Cursor macOS compatibility workflow bounds and cleans silent installer execution', () => {
  assert.match(cursorCompatMacosWorkflow, /runs-on:\s*macos-14/);
  assert.match(cursorCompatMacosWorkflow, /compatibility:\s*[\s\S]*?timeout-minutes:\s*45/);
  assert.match(cursorCompatMacosWorkflow, /Download and install official Cursor build\s*\n\s*timeout-minutes:\s*15/);
  assert.match(cursorCompatMacosWorkflow, /hdiutil attach/);
  assert.doesNotMatch(cursorCompatMacosWorkflow, /Start-Process -FilePath \$installer[^\n]+-Wait/);
  assert.match(cursorCompatMacosWorkflow, /ditto "\$mount\/Cursor\.app" "\$app"/);
  assert.match(cursorCompatMacosWorkflow, /codesign --verify --deep --strict/);
  assert.match(cursorCompatMacosWorkflow, /Authority=\.\+\(Anysphere\|Cursor\)/);
  assert.match(cursorCompatMacosWorkflow, /Detected installed Cursor identity/);
  assert.match(cursorCompatMacosWorkflow, /tail -n 160 "\$installer_log"/);
  assert.match(cursorCompatMacosWorkflow, /SECONDS \+ 12 \* 60/);
  assert.match(cursorCompatMacosWorkflow, /candidate_version" == "\$version"/);
  assert.match(cursorCompatMacosWorkflow, /candidate_commit" =~ \^\[0-9a-f\]\{40\}\$/);
  assert.match(cursorCompatMacosWorkflow, /differs from signed installer product commit/);
  assert.match(cursorCompatMacosWorkflow, /pkill -x Cursor/);
  assert.match(cursorCompatMacosWorkflow, /hdiutil detach/);
  assert.match(cursorCompatMacosWorkflow, /package-macos\.sh/);
  assert.match(cursorCompatMacosWorkflow, /compat\/cursor-stable-macos\.json/);
  assert.match(cursorCompatMacosWorkflow, /needs\.compatibility\.result != 'success'/);
  assert.match(cursorCompatMacosWorkflow, /resolve-failure:[\s\S]+needs\.compatibility\.result == 'success'/);
  assert.match(cursorCompatMacosWorkflow, /Close resolved compatibility issue/);
  assert.match(cursorCompatMacosWorkflow, /macOS 自动兼容构建失败/);
  assert.match(cursorCompatMacosWorkflow, /state_reason: 'completed'/);
  assert.match(cursorCompatMacosWorkflow, /status === 410/);
  assert.match(cursorReleaseMacos, /darwin-universal/);
  assert.match(cursorReleaseMacos, /downloads\.cursor\.com/);
  assert.match(cursorReleaseMacos, /FORCE_COMPAT_CHECK/);
});

test('desktop UI can copy and download run logs', () => {
  for (const id of ['copyLogsButton', 'downloadLogsButton', 'clearLogButton', 'logArea']) {
    assert.match(html, new RegExp(`id=["']${id}["']`));
  }
  assert.match(html, /id="copyLogsButton"[^>]*class="[^"]*secondary-button[^"]*"[^>]*aria-label="复制运行日志"/);
  assert.match(html, /id="downloadLogsButton"[^>]*class="[^"]*secondary-button[^"]*"[^>]*aria-label="下载运行日志"/);
  assert.match(html, /id="copyLogsButton"[^>]*>复制</);
  assert.match(html, /id="downloadLogsButton"[^>]*>下载</);
  assert.match(script, /function collectLogText\(/);
  assert.match(script, /querySelectorAll\("\.log-line"\)/);
  assert.match(script, /async function copyRunLogs\(/);
  assert.match(script, /async function downloadRunLogs\(/);
  assert.match(script, /\$\("#copyLogsButton"\)\.addEventListener\("click", copyRunLogs\)/);
  assert.match(script, /\$\("#downloadLogsButton"\)\.addEventListener\("click", downloadRunLogs\)/);
  assert.match(script, /暂无日志可导出/);
  assert.match(script, /navigator\.clipboard/);
  assert.match(script, /writeText\(/);
  assert.match(script, /execCommand\("copy"\)/);
  assert.match(script, /i18n-workbench-logs-/);
  assert.match(script, /padStart\(2, "0"\)/);
  assert.match(script, /invoke\("save_run_logs"/);
  assert.match(script, /createObjectURL\(/);
  assert.match(script, /link\.download = filename/);
  assert.match(script, /日志已复制到剪贴板/);
  assert.match(script, /复制日志失败/);
  assert.match(script, /导出日志失败/);
  assert.match(desktopMain, /fn save_run_logs\(/);
  assert.match(desktopMain, /save_run_logs,/);
  assert.match(desktopMain, /Downloads/);
  assert.match(styles, /\.log-actions/);
});
