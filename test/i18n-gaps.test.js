'use strict';

const path = require('node:path');
const test = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');

const { loadDicts } = require('../src/dict');
const { applyToText } = require('../src/engine');

const dicts = loadDicts(path.join(__dirname, '..', 'dict'));

function patch(src) {
  const { text, total } = applyToText(src, dicts.code);
  assert.doesNotThrow(() => new vm.Script(src.startsWith('<') ? 'void 0' : text));
  return { text, total };
}

test('sidebar name literals and settings leftovers become Chinese', () => {
  const src = [
    'function $6C(){return re(vKt,{name:"Automations"})}',
    'function W6C(){return re(vKt,{name:"Customize"})}',
    'function L4w(t){return t.admin?"Run Mode Controlled by Team Admin":"Run Mode"}',
    'const row={label:"Keep This Computer Awake",description:"Use the legacy terminal tool in agent mode, for use on systems with unsupported shell configurations"}',
    'Zit("div",{children:"\\u26A0\\uFE0F Warning: Updates Apply Automatically"})',
    'C8y=function(t){switch(t){case"dev":return"Nightly"}}',
  ].join('\n');
  const { text } = patch(src);
  assert.match(text, /name:"自动化"/);
  assert.match(text, /name:"自定义"/);
  assert.match(text, /:"运行模式"/);
  assert.match(text, /label:"保持此电脑唤醒"/);
  assert.match(text, /children:"⚠️ 警告：更新会自动应用"/);
  assert.match(text, /return"每夜版"/);
});

test('Automations empty state, search default, and tips stay covered', () => {
  const src = [
    'const copy={emptyTitle:"No Automations Yet",emptyDescription:"Run agents on a schedule or automatically in response to events. Billed at plan rates.",createButton:"Add Automation",pageTitle:"Automations"}',
    'const k=m===void 0?"Search...":m',
    'k$("h2",{children:"From Cursor"})',
    'Cxe("h3",{children:"Ship better code, faster"})',
    'HgC="Tip dismissed. You can turn off future tips in Settings"',
    'EE(kn,{children:"Tip: you can paste your .env into the name input"})',
    'label:"Incidents & Triage"',
  ].join('\n');
  const { text } = patch(src);
  assert.match(text, /emptyTitle:"还没有自动化"/);
  assert.match(text, /createButton:"添加自动化"/);
  assert.match(text, /\?"搜索\.\.\."/);
  assert.match(text, /children:"来自 Cursor"/);
  assert.match(text, /children:"更快交付更好的代码"/);
  assert.match(text, /HgC="已隐藏提示。可在设置中关闭后续提示"/);
  assert.match(text, /children:"提示：可以把 \.env 粘贴到名称输入框"/);
  assert.match(text, /label:"事故与分诊"/);
});

test('does not translate usage matcher Set keys so server English still matches', () => {
  const src = 'const CJc=new Set(["Consumed by Auto. Additional usage consumes API quota.","Additional usage beyond limits consumes API quota or on-demand spend.","Additional usage beyond limits consumes API quota or on-demand usage."]);const show={autoBeyondLimitDescription:"Additional usage beyond limits consumes Other Models quota or on-demand spend."}';
  const { text } = patch(src);
  assert.match(text, /Consumed by Auto\. Additional usage consumes API quota\./);
  assert.match(text, /Additional usage beyond limits consumes API quota or on-demand spend\./);
  assert.match(text, /autoBeyondLimitDescription:"超出限额的用量将消耗其他模型配额或按需消费。"/);
});
