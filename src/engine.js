'use strict';

const { tokenizer } = require('acorn');
const { PROPS, HTML_ATTRS } = require('./config');

function escRe(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function defaultCtx(en) {
  const words = en.trim().split(/\s+/).length;
  return words >= 2
    ? ['lit', 'prop', 'html-text', 'html-attr']
    : ['prop', 'html-text', 'html-attr'];
}

function bucketEntries(entries) {
  const buckets = { lit: new Map(), prop: new Map(), 'html-text': new Map(), 'html-attr': new Map() };
  for (const [en, e] of entries) {
    for (const c of (e.ctx || defaultCtx(en))) buckets[c].set(en, e.zh);
  }
  return buckets;
}

function alternatives(map) {
  return [...map.keys()].sort((a, b) => b.length - a.length).map(escRe).join('|');
}

function buildHtmlMatchers(buckets) {
  const attrs = HTML_ATTRS.map(escRe).join('|');
  const attrAlternatives = alternatives(buckets['html-attr']);
  const textAlternatives = alternatives(buckets['html-text']);
  return {
    attr: attrAlternatives
      ? new RegExp(`((?:${attrs})=")(${attrAlternatives})"`, 'g')
      : null,
    text: textAlternatives
      ? new RegExp(`(?<!=)>(${textAlternatives})(<|$)`, 'g')
      : null,
  };
}

function htmlReplacements(raw, buckets, matchers, counts) {
  let text = raw;
  let total = 0;

  if (matchers.attr && text.includes('="')) {
    text = text.replace(matchers.attr, (match, prefix, en) => {
      const zh = buckets['html-attr'].get(en);
      if (zh === undefined) return match;
      counts.set(en, (counts.get(en) || 0) + 1);
      total++;
      return `${prefix}${zh}"`;
    });
  }

  if (matchers.text && text.includes('>')) {
    text = text.replace(matchers.text, (match, en, suffix) => {
      const zh = buckets['html-text'].get(en);
      if (zh === undefined) return match;
      counts.set(en, (counts.get(en) || 0) + 1);
      total++;
      return `>${zh}${suffix}`;
    });
  }
  return { text, total };
}

function isStandaloneHtml(text) {
  return /^\s*<(?:!doctype\b|[a-z][\w:-]*(?:\s|\/?>))/i.test(text);
}

function encodeLiteral(value, quote) {
  let encoded = value
    .replace(/\\/g, '\\\\')
    .replace(/\r/g, '\\r')
    .replace(/\n/g, '\\n')
    .replace(/\u2028/g, '\\u2028')
    .replace(/\u2029/g, '\\u2029');
  if (quote === '`') return encoded.replace(/`/g, '\\`').replace(/\$\{/g, '\\${');
  return encoded.replace(new RegExp(escRe(quote), 'g'), `\\${quote}`);
}

function isPropName(token, props) {
  return token && (token.type.label === 'name' || token.type.label === 'string')
    && props.has(String(token.value));
}

// Cursor 3.16 React settings: label:cond?"A":"B" and children:["Open", icon].
// Keep a short token window — enough for a ternary or a small children array.
const PROP_LOOKBEHIND = 24;

function isInsidePropArray(previous, props) {
  let depth = 0;
  for (let i = previous.length - 1; i >= 0; i--) {
    const label = previous[i].type.label;
    if (label === ']') depth++;
    else if (label === '[') {
      if (depth === 0) {
        return i >= 2
          && previous[i - 1].type.label === ':'
          && isPropName(previous[i - 2], props);
      }
      depth--;
    } else if (depth === 0 && label === ';') {
      return false;
    }
  }
  return false;
}

function isTernaryPropValue(previous, props) {
  const last = previous[previous.length - 1];
  if (!last || (last.type.label !== '?' && last.type.label !== ':')) return false;
  for (let i = previous.length - 1; i >= 1; i--) {
    const label = previous[i].type.label;
    if (label === ':' && isPropName(previous[i - 1], props)) return true;
    // Do not treat { } as a stop: template `${...}` interpolations emit them
    // and would miss children:cond?`...`:"Disabled".
    if (label === ';' || label === ',') return false;
  }
  return false;
}

function isPropLiteralContext(previous, props) {
  if (previous.length >= 2
      && previous[previous.length - 1].type.label === ':'
      && isPropName(previous[previous.length - 2], props)) {
    return true;
  }
  if (previous.length >= 3
      && previous[previous.length - 1].type.label === '='
      && previous[previous.length - 2].type.label === 'name'
      && isPropName(previous[previous.length - 2], props)
      && previous[previous.length - 3].type.label === '.') {
    return true;
  }
  return isInsidePropArray(previous, props) || isTernaryPropValue(previous, props);
}

function applyToText(text, entries) {
  const buckets = bucketEntries(entries);
  const htmlMatchers = buildHtmlMatchers(buckets);
  const props = new Set(PROPS);
  const counts = new Map();

  if (isStandaloneHtml(text)) {
    const html = htmlReplacements(text, buckets, htmlMatchers, counts);
    return { text: html.text, counts, total: html.total };
  }

  const replacements = [];
  const tokens = tokenizer(text, {
    ecmaVersion: 'latest',
    sourceType: 'module',
    allowHashBang: true,
  });
  const previous = [];
  let forAwaitState = 0;
  let forAwaitParenDepth = 0;

  while (true) {
    const token = tokens.getToken();
    if (token.type.label === 'eof') break;

    if (forAwaitParenDepth > 0) {
      if (token.type.label === '(') forAwaitParenDepth++;
      if (token.type.label === ')' && --forAwaitParenDepth === 0) tokens.exprAllowed = true;
    } else if (token.type.label === 'for') {
      forAwaitState = 1;
    } else if (forAwaitState === 1 && token.type.label === 'name' && token.value === 'await') {
      forAwaitState = 2;
    } else if (forAwaitState === 2 && token.type.label === '(') {
      forAwaitState = 0;
      forAwaitParenDepth = 1;
    } else {
      forAwaitState = 0;
    }

    if (token.type.label === 'string' || token.type.label === 'template') {
      const rawStart = token.type.label === 'string' ? token.start + 1 : token.start;
      const rawEnd = token.type.label === 'string' ? token.end - 1 : token.end;
      const quote = token.type.label === 'string' ? text[token.start] : '`';
      const originalRaw = text.slice(rawStart, rawEnd);
      let replacement = originalRaw;
      let replacedLiteral = false;

      if (token.type.label === 'string' || token.type.label === 'template') {
        const literalMap = token.type.label === 'template'
          ? buckets.lit
          : isPropLiteralContext(previous, props)
            ? buckets.prop
            : buckets.lit;
        const en = String(token.value);
        const zh = literalMap.get(en);
        if (zh !== undefined) {
          replacement = encodeLiteral(zh, quote);
          replacedLiteral = true;
          counts.set(en, (counts.get(en) || 0) + 1);
        }
      }

      const html = htmlReplacements(replacement, buckets, htmlMatchers, counts);
      replacement = html.text;
      if (replacedLiteral || html.total > 0) {
        replacements.push({ start: rawStart, end: rawEnd, text: replacement });
      }
    }

    previous.push(token);
    if (previous.length > PROP_LOOKBEHIND) previous.shift();
  }

  if (!replacements.length) return { text, counts, total: 0 };
  let output = '';
  let cursor = 0;
  for (const item of replacements) {
    output += text.slice(cursor, item.start);
    output += item.text;
    cursor = item.end;
  }
  output += text.slice(cursor);
  return {
    text: output,
    counts,
    total: [...counts.values()].reduce((sum, count) => sum + count, 0),
  };
}

module.exports = { applyToText, defaultCtx };
