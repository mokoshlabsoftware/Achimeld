// Documentation discovery only. No application build or runtime dependency.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';

const root = path.resolve(process.argv[2] || '.');
const output = path.resolve(process.argv[3] || path.join(root, 'docs/wiki/source-inventory.json'));
const hash = data => crypto.createHash('sha256').update(data).digest('hex');
const relative = file => path.relative(root, file).replaceAll('\\', '/');
const text = file => fs.readFileSync(path.join(root, file), 'utf8');
const lineAt = (source, offset) => source.slice(0, offset).split('\n').length;
function walk(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name, 'en')).flatMap(entry =>
    entry.isDirectory() ? walk(path.join(directory, entry.name)) : [path.join(directory, entry.name)]);
}
function articles(file) {
  const name = path.basename(file, path.extname(file));
  if (file.includes('achimeld-mcp') || /automation|mcp-server/.test(name)) return ['automation'];
  if (/synchronization|sync_previews/.test(name)) return ['sync', 'steam'];
  if (/steam|browser_profile|steamworks/.test(name)) return ['steam', 'sync'];
  if (/style_|style-/.test(name) || name === 'styles') return ['styles'];
  if (/art_bank|art_images|editor-image-drops/.test(name)) return ['art-data-bank'];
  if (/general_image/.test(name)) return ['general-images', 'achievements-general'];
  if (/achievement_general|AchievementText/.test(name)) return ['achievements-general'];
  if (/mask/.test(name)) return ['masks'];
  if (/translation|localization/.test(name)) return ['localization'];
  if (/icon_export/.test(name)) return ['exports'];
  if (/settings|preferences|field-labels|numeric-sliders|popup-dropdowns/.test(name)) return ['preferences', 'projects'];
  if (/about|change_log|update-notes/.test(name)) return ['getting-started', 'troubleshooting'];
  if (/diagnostic|terminal/.test(name)) return ['troubleshooting'];
  if (/effect|blend/.test(name)) return ['icon-effects'];
  if (/gradient|particle|explosion|artifact|text3d|curves|perlin|vignette|shape|text_content|text_alignment|font_picker/.test(name.toLowerCase())) return ['icon-layers'];
  if (/layer|IconRecipe|IconStyle/.test(name)) return ['icon-editor', 'icon-layers'];
  if (/canvas|node_|sorting|reorder/.test(name)) return ['achievements-general', 'stats', 'icon-editor'];
  if (/inspector/.test(name)) return ['achievements-general', 'stats', 'localization', 'icon-editor'];
  if (/desktop|app_dialog|ports|lib|delta|history|document|validation|editor_state|recent_projects/.test(name)) return ['getting-started', 'projects', 'achievements-general', 'stats'];
  if (file.includes('bridge/Achimeld.Core')) return ['achievements-general', 'stats', 'icon-layers'];
  return ['icon-editor'];
}
function csv(source) {
  const rows = []; let row = [], cell = '', quoted = false;
  for (let i = 0; i < source.length; i++) {
    const c = source[i];
    if (c === '"') {
      if (quoted && source[i + 1] === '"') { cell += '"'; i++; } else quoted = !quoted;
    } else if (c === ',' && !quoted) { row.push(cell); cell = ''; }
    else if (c === '\n' && !quoted) { row.push(cell.replace(/\r$/, '')); rows.push(row); row = []; cell = ''; }
    else cell += c;
  }
  if (cell || row.length) { row.push(cell); rows.push(row); }
  return rows;
}
const translations = new Map(csv(text('crates/achimeld-ui/assets/strings.csv')).slice(1).map(row => [row[0], row[2]]));
const inputFiles = [
  ...['achimeld-ui', 'achimeld-application', 'achimeld-mcp'].flatMap(crate =>
    walk(path.join(root, 'crates', crate, 'src')).filter(file => file.endsWith('.rs'))),
  ...walk(path.join(root, 'crates/achimeld-app/src')).filter(file =>
    file.endsWith('.rs') && !/\/diagnostics\//.test(relative(file))),
  ...walk(path.join(root, 'bridge/Achimeld.Core')).filter(file => file.endsWith('.cs') && !/\/(obj|bin)\//.test(relative(file))),
  ...fs.readdirSync(path.join(root, 'docs')).filter(file => file.endsWith('.md')).map(file => path.join(root, 'docs', file)),
  ...['CHANGELOG.md', 'crates/achimeld-ui/assets/strings.csv'].map(file => path.join(root, file)),
].filter(file => !/tests|regressions|performance_benchmark|test_output/.test(path.basename(file))).sort();
const files = []; const candidates = []; const declarations = [];
for (const file of inputFiles) {
  const bytes = fs.readFileSync(file); const source = bytes.toString('utf8'); const sourcePath = relative(file);
  const articleIds = articles(sourcePath);
  files.push({ path: sourcePath, sha256: hash(bytes), articleIds,
    sourceReview: 'indexed', runtimeVerification: 'pending',
    task: 'Inspect user-facing branches and callers; reconcile indexed candidates with visible controls and conditional states.' });
  if (!file.endsWith('.rs') && !file.endsWith('.cs')) continue;
  // These are lexical candidates, including internal IDs and model keys.
  // They deliberately require human triage before becoming published controls.
  const production = source.split(/#\[cfg\(test\)\]\s*(?:\r?\n)?mod\s/)[0];
  for (const match of production.matchAll(/"(?:\\.|[^"\\\r\n])*"/g)) {
    const value = match[0].slice(1, -1);
    if (!value.trim() || value.length > 240 || /(?:include_bytes!|include_str!)/.test(production.slice(Math.max(0, match.index - 25), match.index))) continue;
    const line = lineAt(source, match.index);
    candidates.push({ id: hash(sourcePath + ':' + line + ':' + value).slice(0, 16),
      source: sourcePath, line, value, english: translations.get(value) || null, articleIds,
      triage: 'pending', verificationTask: 'Classify as visible control, option, tooltip, condition, data key or internal string; verify placement and behavior in matching build.' });
  }
  for (const match of production.matchAll(/(?:pub(?:\([^)]*\))?\s+)?(?:enum|struct)\s+([A-Za-z_][A-Za-z_0-9]*)|public\s+(?:sealed\s+)?(?:record|enum)\s+([A-Za-z_][A-Za-z_0-9]*)/g)) {
    declarations.push({ name: match[1] || match[2], source: sourcePath, line: lineAt(source, match.index), articleIds, triage: 'pending' });
  }
}
const recipe = text('bridge/Achimeld.Core/IconRecipe.cs');
function enumValues(name) {
  const match = recipe.match(new RegExp('public enum ' + name + '\\s*\\{([^}]+)\\}'));
  if (!match) throw new Error('Missing enum ' + name);
  return match[1].split(',').map(value => value.trim()).filter(Boolean);
}
const kinds = enumValues('IconLayerKind'), effects = enumValues('IconLayerEffectKind');
const layerIcons = text('crates/achimeld-ui/src/layer_icons.rs');
const picker = name => {
  const match = layerIcons.match(new RegExp('const ' + name + ':[^=]+=[\\s\\S]*?\\[([\\s\\S]*?)\\];'));
  if (!match) throw new Error('Missing picker ' + name);
  return [...match[1].matchAll(/"([^"]+)"/g)].map(match => match[1]);
};
const addLayers = picker('LAYERS'), adjustments = picker('ADJUSTMENTS');
const modelCoverage = {
  source: 'bridge/Achimeld.Core/IconRecipe.cs',
  layers: kinds.map(kind => ({kind, surface: addLayers.includes(kind) ? 'layer-picker' : adjustments.includes(kind) ? 'adjustment-picker' : 'special-workflow',
    articleId: kind === 'MaskReference' ? 'masks' : 'icon-layers', status: 'source-identified', runtimeVerification: 'pending'})),
  effects: effects.map(kind => ({kind, articleId: 'icon-effects', status: 'source-identified', runtimeVerification: 'pending'})),
  blendModes: enumValues('IconBlendMode'), maskModes: enumValues('IconMaskMode'),
  gradientKinds: enumValues('IconGradientKind'), shapeKinds: enumValues('IconShapeKind'),
};
const changelog = text('CHANGELOG.md');
const heading = [...changelog.matchAll(/^## ([0-9]+\.[0-9]+\.[0-9]+\.[0-9]+) \(([^)]+)\)/gm)];
const history = heading.map((match, index) => {
  const body = changelog.slice(match.index + match[0].length, heading[index + 1]?.index ?? changelog.length);
  const changes = [...body.matchAll(/^- (.+)$/gm)].map(match => match[1].trim());
  const clues = changes.join(' ').toLowerCase();
  const suggestions = new Set();
  for (const [pattern, ids] of [
    [/steam|browser|synchroniz|upload|remote|validation/, ['steam','sync']],
    [/image|artwork|atlas|bank/, ['art-data-bank','general-images']],
    [/layer|icon|gradient|particle|explosion|mask|text|blend/, ['icon-editor','icon-layers','icon-effects']],
    [/stat|unlock|achievement/, ['achievements-general','stats']],
    [/translat|language|font/, ['localization','preferences']],
    [/save|project|recover/, ['projects','troubleshooting']],
    [/log|diagnostic|crash|failure/, ['troubleshooting']],
    [/style|collection/, ['styles']],
    [/export/, ['exports']],
    [/mcp|automation/, ['automation']],
  ]) if (pattern.test(clues)) ids.forEach(id => suggestions.add(id));
  return { version: match[1], date: match[2], changes, suggestedArticleIds: [...suggestions],
    decision: 'pending-human-review', task: 'Confirm affected articles or record an explicit no-impact reason; suggestions are not reviewed impact decisions.' };
});
let sourceCommit = null;
try { sourceCommit = execFileSync('git', ['-C', root, 'rev-parse', 'HEAD'], {encoding:'utf8',stdio:['ignore','pipe','ignore']}).trim(); } catch {}
const snapshotSha256 = hash(files.map(file => file.path + '\0' + file.sha256).join('\n'));
const result = {
  schemaVersion: 1, inventoryDate: new Intl.DateTimeFormat('en-CA', {timeZone:'Europe/Warsaw'}).format(new Date()), applicationVersion: heading[0][1],
  sourceCommit, snapshotSha256, snapshotScope: 'Listed documentation and feature source files only; file hashes identify working contents including uncommitted edits.',
  method: 'Static lexical discovery and model/picker comparison. Candidates and history suggestions require manual review.',
  runtimeEvidence: {status:'not-collected-by-this-tool', reason:'This utility only indexes source. Consult the phase 1 report and capture manifest for separately collected runtime observations.'},
  counts: {files:files.length,candidates:candidates.length,declarations:declarations.length,layers:kinds.length,effects:effects.length,historyEntries:history.length},
  files, modelCoverage, declarations, candidates, history,
};
fs.mkdirSync(path.dirname(output), {recursive:true});
fs.writeFileSync(output, JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify({output, applicationVersion:result.applicationVersion,snapshotSha256,counts:result.counts}));
