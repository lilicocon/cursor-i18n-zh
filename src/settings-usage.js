'use strict';

const ACCOUNT_ANCHOR = 'label:"Cursor Account",description:"Manage your account and billing"';
const PLAN_USAGE_ANCHOR = '[SettingsPlanUsageTab] Failed to fetch hard limit';
const ACCOUNT_DESC = 'description:"Manage your account and billing"';
const ACCOUNT_LABEL = 'label:"Cursor Account"';
const PLAN_USAGE_REACT = '[PlanUsageConfig] Failed to fetch hard limit';
const GENERAL_CHILDREN = 'title:"General",children:[';
const INJECTION_MARKER = 'i18nAccountUsage:!0';

function escapeRe(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function functionBefore(text, index) {
  const re = /function\s+([A-Za-z_$][\w$]*)\s*\([^)]*\)\{/g;
  let found = null;
  let match;
  while ((match = re.exec(text)) && match.index < index) {
    found = { name: match[1], index: match.index };
  }
  return found;
}

function lastMatch(re, text) {
  let found = null;
  let match;
  while ((match = re.exec(text))) found = match;
  return found;
}

function findAccountIndex(text) {
  let start = 0;
  while (start < text.length) {
    const desc = text.indexOf(ACCOUNT_DESC, start);
    if (desc < 0) return -1;
    const label = text.indexOf(ACCOUNT_LABEL, desc);
    if (label >= 0 && label - desc < 80) return desc;
    start = desc + ACCOUNT_DESC.length;
  }
  return -1;
}

function findPlanIndex(text) {
  const react = text.indexOf(PLAN_USAGE_REACT);
  return react >= 0 ? react : text.indexOf(PLAN_USAGE_ANCHOR);
}

function embedSolidAccountUsage(text) {
  const accountIndex = text.indexOf(ACCOUNT_ANCHOR);
  const planUsageIndex = text.indexOf(PLAN_USAGE_ANCHOR);
  if (accountIndex < 0 || planUsageIndex < 0) {
    return { text, injected: false, reason: 'anchors-missing' };
  }

  const generalFunction = functionBefore(text, accountIndex);
  const planUsageFunction = functionBefore(text, planUsageIndex);
  if (!generalFunction || !planUsageFunction) {
    return { text, injected: false, reason: 'functions-missing' };
  }

  const generalHead = text.slice(generalFunction.index, accountIndex);
  const signedIn = generalHead.match(/\{signedIn:([A-Za-z_$][\w$]*),membershipType:/)?.[1];
  const factory = lastMatch(/return\[([A-Za-z_$][\w$]*)\(/g, generalHead)?.[1];
  if (!signedIn || !factory) {
    return { text, injected: false, reason: 'general-symbols-missing' };
  }

  const accountWindow = text.slice(Math.max(generalFunction.index, accountIndex - 5000), accountIndex + 5000);
  const show = accountWindow.match(new RegExp(
    `${factory}\\(([A-Za-z_$][\\w$]*),\\{get when\\(\\)\\{return ${signedIn}\\(\\)\\}`,
  ))?.[1];
  if (!show) return { text, injected: false, reason: 'conditional-symbol-missing' };

  const preferencesIndex = text.indexOf('title:"Preferences"', accountIndex);
  if (preferencesIndex < 0 || preferencesIndex > planUsageIndex) {
    return { text, injected: false, reason: 'preferences-anchor-missing' };
  }

  const insertionIndex = text.lastIndexOf(`${factory}(${show},{when:`, preferencesIndex);
  if (insertionIndex < generalFunction.index) {
    return { text, injected: false, reason: 'insertion-point-missing' };
  }

  const addition = `${factory}(${show},{get when(){return ${signedIn}()},get children(){return ${factory}(${planUsageFunction.name},{${INJECTION_MARKER}})}}),`;
  return {
    text: text.slice(0, insertionIndex) + addition + text.slice(insertionIndex),
    injected: true,
    reason: null,
  };
}

function embedReactAccountUsage(text) {
  const accountIndex = findAccountIndex(text);
  const planUsageIndex = findPlanIndex(text);
  if (accountIndex < 0 || planUsageIndex < 0) {
    return { text, injected: false, reason: 'anchors-missing' };
  }

  const accountFunction = functionBefore(text, accountIndex);
  const planUsageFunction = functionBefore(text, planUsageIndex);
  if (!accountFunction || !planUsageFunction) {
    return { text, injected: false, reason: 'functions-missing' };
  }

  const planCall = lastMatch(
    new RegExp(`([A-Za-z_$][\\w$]*)\\(${escapeRe(planUsageFunction.name)},\\{`, 'g'),
    text.slice(0, planUsageFunction.index),
  );
  if (!planCall) return { text, injected: false, reason: 'plan-wrapper-missing' };
  const planWrapper = functionBefore(text, planCall.index);
  if (!planWrapper) return { text, injected: false, reason: 'plan-wrapper-missing' };

  const generalIndex = text.indexOf(GENERAL_CHILDREN);
  if (generalIndex < 0) return { text, injected: false, reason: 'general-anchor-missing' };
  const generalFunction = functionBefore(text, generalIndex);
  if (!generalFunction) return { text, injected: false, reason: 'general-anchor-missing' };

  const generalHead = text.slice(generalFunction.index, generalIndex);
  const factory = generalHead.match(
    new RegExp(`([A-Za-z_$][\\w$]*)\\(${escapeRe(accountFunction.name)},`),
  )?.[1];
  if (!factory) return { text, injected: false, reason: 'general-factory-missing' };

  const signedIn = generalHead.match(
    new RegExp(`([A-Za-z_$][\\w$]*)\\?${escapeRe(factory)}\\(${escapeRe(accountFunction.name)},`),
  )?.[1];
  if (!signedIn) return { text, injected: false, reason: 'signed-in-symbol-missing' };

  const after = generalIndex + GENERAL_CHILDREN.length;
  const firstChild = text.slice(after).match(/^([A-Za-z_$][\w$]*),/);
  if (!firstChild) return { text, injected: false, reason: 'insertion-point-missing' };

  const addition = `${signedIn}?${factory}(${planWrapper.name},{${INJECTION_MARKER}}):null,`;
  const insertionIndex = after + firstChild[0].length;
  return {
    text: text.slice(0, insertionIndex) + addition + text.slice(insertionIndex),
    injected: true,
    reason: null,
  };
}

function embedAccountUsage(text) {
  if (text.includes(INJECTION_MARKER)) {
    return { text, injected: false, reason: 'already-present' };
  }
  const solid = embedSolidAccountUsage(text);
  if (solid.injected) return solid;
  return embedReactAccountUsage(text);
}

// Server usage copy is remapped only when it exactly matches these Set keys.
// Keep the keys English; add variants Cursor started sending later.
const USAGE_MATCHER_SET = '["Consumed by Auto. Additional usage consumes API quota.","Additional usage beyond limits consumes API quota or on-demand spend.","Additional usage beyond limits consumes API quota or on-demand usage."]';
const USAGE_MATCHER_EXTRA = 'Additional usage beyond limits consumes on-demand spend.';

function expandUsageMatcherSets(text) {
  if (!text.includes(USAGE_MATCHER_SET) || text.includes(`"${USAGE_MATCHER_EXTRA}"`)) {
    return { text, expanded: false };
  }
  return {
    text: text.split(USAGE_MATCHER_SET).join(
      `["Consumed by Auto. Additional usage consumes API quota.","Additional usage beyond limits consumes API quota or on-demand spend.","Additional usage beyond limits consumes API quota or on-demand usage.","${USAGE_MATCHER_EXTRA}"]`,
    ),
    expanded: true,
  };
}

module.exports = { embedAccountUsage, expandUsageMatcherSets, INJECTION_MARKER };
