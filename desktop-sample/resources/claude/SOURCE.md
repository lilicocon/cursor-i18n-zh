# Claude Desktop 简体中文翻译记忆库

- 固定使用 `translation_memory.json` 内嵌上游快照 `20260730035926`, 共 22678 条.
- 工作台叠加库 `translation_memory_overlay.json` 版本 `20260822170000`, 覆盖官方 Claude Desktop / Claude Code 桌面客户端 1.34493.1 三个 `en-US.json` 中上游快照未收录或仍为英文 / 「您」的字符串.
- `translation_id_overlay.json` 向 `ion-dist/i18n/en-US.json` 补入 JS `defaultMessage` 有 ID 但不在官方 locale 文件里的条文案.
- 上游来源: https://github.com/GMYXDS/claude-desktop-zh-simple.
- 遵守 `APACHE-2.0.txt`.
- 只将英文 JSON 字符串值精确映射为简体中文.
- 禁止修改 JSON 键, `app.asar`, `Claude.exe` 或客户端配置.
