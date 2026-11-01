import fs from 'node:fs';
import path from 'node:path';
const root = path.resolve(process.argv[2] || '.');
const wiki = path.join(root, 'docs/wiki');
const metadata = JSON.parse(fs.readFileSync(path.join(wiki, 'articles.json')));
const coverage = JSON.parse(fs.readFileSync(path.join(wiki, 'reference-coverage.json')));
const sourceMap = JSON.parse(fs.readFileSync(path.join(wiki, 'source-evidence-phase3.json')));
const ids = new Set(metadata.articles.map(article => article.id));
if (ids.size !== metadata.articles.length) throw new Error('Duplicate article IDs');
let links = 0;
for (const name of fs.readdirSync(wiki).filter(name => name.endsWith('.md'))) {
  const content = fs.readFileSync(path.join(wiki, name), 'utf8');
  if (!content.startsWith('# ')) throw new Error('Missing title: ' + name);
  if (/[ \t]+$/m.test(content)) throw new Error('Trailing whitespace: ' + name);
  for (const match of content.matchAll(/\]\(([^)]+)\)/g)) {
    const target = match[1].split('#')[0];
    if (!target || /^https?:/.test(target)) continue;
    if (!fs.existsSync(path.resolve(wiki, target))) throw new Error('Broken link in ' + name + ': ' + target);
    links++;
  }
}
for (const article of metadata.articles) {
  if (!fs.existsSync(path.join(wiki, article.path))) throw new Error('Missing article: ' + article.id);
  if (!/^\d+\.\d+\.\d+\.\d+$/.test(article.documentedVersion)) throw new Error('Invalid version: ' + article.id);
  for (const evidence of article.verificationEvidence) {
    if (!fs.existsSync(path.resolve(wiki, evidence))) throw new Error('Missing verification evidence: ' + evidence);
  }
}
const recipe = fs.readFileSync(path.join(root, 'bridge/Achimeld.Core/IconRecipe.cs'), 'utf8');
for (const [model, rows] of [['IconLayerKind', coverage.layers], ['IconLayerEffectKind', coverage.effects]]) {
  const match = recipe.match(new RegExp('public enum ' + model + '\\s*\\{([^}]+)\\}'));
  if (!match) throw new Error('Missing model enum');
  const kinds = match[1].split(',').map(item => item.trim()).filter(Boolean);
  if (new Set(rows.map(row=>row.kind)).size !== rows.length) throw new Error('Duplicate coverage row');
  for (const kind of kinds) if (!rows.some(row=>row.kind === kind)) throw new Error('Unmapped model kind: ' + kind);
  for (const row of rows) {
    if (!kinds.includes(row.kind) || !ids.has(row.articleId)) throw new Error('Invalid coverage destination');
    const article = metadata.articles.find(article=>article.id === row.articleId);
    const content = fs.readFileSync(path.join(wiki,article.path),'utf8');
    if (row.section && !content.includes('## ' + row.section + '\n')) throw new Error('Missing coverage section: '+row.kind);
  }
}
for (const file of sourceMap.files) {
  if (!fs.existsSync(path.join(root, file.path))) throw new Error('Missing recorded source: ' + file.path);
}
const captures = [];
for (const folder of fs.readdirSync(path.join(wiki, 'screenshots'))) {
  if (!/^(?:\d+\.\d+\.\d+\.\d+|unversioned)$/.test(folder)) continue;
  const manifestRoot = path.join(wiki, 'screenshots', folder);
  const manifest = path.join(manifestRoot, 'manifest.json');
  if (!fs.existsSync(manifest)) continue;
  for (const capture of JSON.parse(fs.readFileSync(manifest))) {
    if (!/^[a-z0-9-]+\.(?:jpg|png)$/.test(capture.file)) throw new Error('Invalid screenshot filename');
    if (capture.applicationVersion !== (folder === 'unversioned' ? null : folder)) throw new Error('Screenshot version mismatch');
    if (!fs.existsSync(path.join(manifestRoot, capture.file))) throw new Error('Missing screenshot');
    if ((capture.articleIds || []).some(id => !ids.has(id))) throw new Error('Unknown screenshot article');
    captures.push(capture);
  }
}
console.log(JSON.stringify({articles:metadata.articles.length,validLocalLinks:links,layerKinds:coverage.layers.length,effectKinds:coverage.effects.length,recordedSources:sourceMap.files.length,captures:captures.length}));
