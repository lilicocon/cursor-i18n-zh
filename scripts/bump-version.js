'use strict';

const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const VERSION_RE = /^\d+\.\d+\.\d+$/;
const STUB = '（发版时填写）';

function read(rel) {
  return fs.readFileSync(path.join(root, rel), 'utf8');
}

function write(rel, text) {
  fs.writeFileSync(path.join(root, rel), text);
}

function exists(rel) {
  return fs.existsSync(path.join(root, rel));
}

function packageVersion() {
  return JSON.parse(read('package.json')).version;
}

function replaceAll(text, from, to) {
  return text.split(from).join(to);
}

function setJsonVersion(rel, version) {
  const data = JSON.parse(read(rel));
  data.version = version;
  write(rel, `${JSON.stringify(data, null, 2)}\n`);
}

function setPackageLockVersion(version) {
  const data = JSON.parse(read('package-lock.json'));
  data.version = version;
  if (data.packages && data.packages['']) {
    data.packages[''].version = version;
  }
  write('package-lock.json', `${JSON.stringify(data, null, 2)}\n`);
}

function setCargoTomlVersion(version) {
  write(
    'desktop-sample/src-tauri/Cargo.toml',
    read('desktop-sample/src-tauri/Cargo.toml').replace(
      /^version = "[^"]+"/m,
      `version = "${version}"`,
    ),
  );
}

function setCargoLockVersion(version) {
  write(
    'desktop-sample/src-tauri/Cargo.lock',
    read('desktop-sample/src-tauri/Cargo.lock').replace(
      /(name = "cursor-i18n-desktop-sample"\nversion = ")[^"]+(")/,
      `$1${version}$2`,
    ),
  );
}

function todayUtc() {
  return new Date().toISOString().slice(0, 10);
}

function ensureChangelog(from, to) {
  const rel = 'CHANGELOG.md';
  let text = read(rel);
  if (!text.includes(`## [${to}]`)) {
    const heading = `## [${from}]`;
    const stub = `## [${to}] - ${todayUtc()}\n\n### 新增\n\n- ${STUB}\n\n`;
    if (!text.includes(heading)) {
      throw new Error(`CHANGELOG.md missing ${heading}`);
    }
    text = text.replace(heading, `${stub}${heading}`);
  }
  const link = `[${to}]: https://github.com/lilicocon/cursor-i18n-zh/compare/v${from}...v${to}`;
  if (!text.includes(`[${to}]:`)) {
    text = text.replace(`\n[${from}]:`, `\n${link}\n[${from}]:`);
  }
  write(rel, text);
}

function ensureReleaseNotes(from, to) {
  const rel = `.github/releases/v${to}.md`;
  if (exists(rel)) {
    return;
  }
  const previous = read(`.github/releases/v${from}.md`);
  const header = `# 汉化工作台 v${to}

## 下载

按本机系统和芯片选择对应安装包:

- Windows x86（Intel/AMD 64 位）: \`localization-workbench-v${to}-windows-x64.zip\` / \`.exe\`.
- Windows ARM: \`localization-workbench-v${to}-windows-arm64.zip\` / \`.exe\`.
- macOS ARM（Apple Silicon）: \`localization-workbench-v${to}-macos-arm64.dmg\`.
- macOS x86（Intel）: \`localization-workbench-v${to}-macos-x64.dmg\`.
- macOS 便携包: \`localization-workbench-v${to}-macos-arm64-app.zip\` / \`-macos-x64-app.zip\`.

## 本版

- ${STUB}

`;
  const standingStart = previous.indexOf('## 已保留的安全能力');
  const standing = standingStart === -1 ? '' : previous.slice(standingStart);
  write(rel, `${header}${standing}`);
}

function bump(to) {
  if (!VERSION_RE.test(to)) {
    throw new Error(`invalid version: ${to}`);
  }
  const from = packageVersion();
  if (from === to) {
    throw new Error(`package.json is already ${to}`);
  }

  setJsonVersion('package.json', to);
  setPackageLockVersion(to);
  setJsonVersion('desktop-sample/src-tauri/tauri.conf.json', to);
  setCargoTomlVersion(to);
  setCargoLockVersion(to);

  write(
    'desktop-sample/ui/index.html',
    replaceAll(read('desktop-sample/ui/index.html'), `v${from}`, `v${to}`),
  );
  write(
    'desktop-sample/ui/app.js',
    replaceAll(replaceAll(read('desktop-sample/ui/app.js'), `"${from}"`, `"${to}"`), `v${from}`, `v${to}`),
  );
  write(
    'README.md',
    replaceAll(read('README.md'), `localization-workbench-v${from}`, `localization-workbench-v${to}`),
  );
  write(
    'desktop-sample/README.md',
    replaceAll(replaceAll(read('desktop-sample/README.md'), `v${from}`, `v${to}`), `localization-workbench-v${from}`, `localization-workbench-v${to}`),
  );

  ensureChangelog(from, to);
  ensureReleaseNotes(from, to);
}

function requireContains(rel, snippet, errors) {
  if (!exists(rel)) {
    errors.push(`missing ${rel}`);
    return;
  }
  if (!read(rel).includes(snippet)) {
    errors.push(`${rel} missing ${snippet}`);
  }
}

function requireAbsent(rel, snippet, errors) {
  if (exists(rel) && read(rel).includes(snippet)) {
    errors.push(`${rel} still contains stub ${snippet}`);
  }
}

function check() {
  const version = packageVersion();
  if (!VERSION_RE.test(version)) {
    throw new Error(`package.json version is not x.y.z: ${version}`);
  }
  const errors = [];
  const lock = JSON.parse(read('package-lock.json'));
  if (lock.version !== version) {
    errors.push(`package-lock.json version ${lock.version} != ${version}`);
  }
  if (lock.packages?.['']?.version !== version) {
    errors.push(`package-lock.json packages[""].version != ${version}`);
  }

  const tauri = JSON.parse(read('desktop-sample/src-tauri/tauri.conf.json'));
  if (tauri.version !== version) {
    errors.push(`tauri.conf.json version ${tauri.version} != ${version}`);
  }

  requireContains('desktop-sample/src-tauri/Cargo.toml', `version = "${version}"`, errors);
  requireContains(
    'desktop-sample/src-tauri/Cargo.lock',
    `name = "cursor-i18n-desktop-sample"\nversion = "${version}"`,
    errors,
  );
  requireContains('desktop-sample/ui/index.html', `v${version}`, errors);
  requireContains('desktop-sample/ui/app.js', `adapterVersion: "${version}"`, errors);
  requireContains('desktop-sample/ui/app.js', `currentVersion: "${version}"`, errors);
  requireContains('README.md', `localization-workbench-v${version}-`, errors);
  requireContains('desktop-sample/README.md', `# 汉化工作台 v${version}`, errors);
  requireContains('CHANGELOG.md', `## [${version}]`, errors);
  requireContains('CHANGELOG.md', `[${version}]: https://github.com/lilicocon/cursor-i18n-zh/compare/`, errors);
  requireContains(`.github/releases/v${version}.md`, `# 汉化工作台 v${version}`, errors);
  requireContains(`.github/releases/v${version}.md`, `localization-workbench-v${version}-`, errors);
  requireContains(`.github/releases/v${version}.md`, '## 已保留的安全能力', errors);
  requireContains(`.github/releases/v${version}.md`, '## 使用提醒', errors);
  requireAbsent('CHANGELOG.md', STUB, errors);
  requireAbsent(`.github/releases/v${version}.md`, STUB, errors);

  if (errors.length) {
    throw new Error(`version ${version} is not synchronized:\n- ${errors.join('\n- ')}`);
  }
  return version;
}

function usage() {
  throw new Error('usage: node scripts/bump-version.js <x.y.z> | --check');
}

function main(argv) {
  const cmd = argv[0];
  if (cmd === '--check') {
    const version = check();
    process.stdout.write(`version ${version} is synchronized\n`);
    return;
  }
  if (!cmd) {
    usage();
  }
  bump(cmd);
  process.stdout.write(`bumped to ${cmd}; fill CHANGELOG.md and .github/releases/v${cmd}.md stubs, then run --check\n`);
}

module.exports = { bump, check, packageVersion };

if (require.main === module) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exit(1);
  }
}
