// End-to-end check of photo analysis in rc-photo (0.6.0) on a fresh RustCloud
// with use.onnx: the server downloads SigLIP 2 and PP-OCRv5, the page's first
// look adds `clip` and `ocr` jobs for every photo, the jobs run; similar
// photos and the text in photos through the API and the page; the
// administrator's switch.
//
//   node vision.mjs <base url>   (in mcr.microsoft.com/playwright, /e2e = this folder)
//
// Photos in /e2e/vision/<kind>_<n>.jpg (not in git): photos of one kind are
// alike (two cats, two forests); sign_1.jpg is a street sign reading
// "УЛИЦА САМОИЛОВА".
import { readFileSync, readdirSync } from 'node:fs';
import { chromium } from 'playwright';

const base = process.argv[2];
const H = { 'x-requested-with': 'e2e' };
const ok = async (reply, what) => {
  if (!reply.ok()) throw new Error(`${what}: ${reply.status()} ${await reply.text()}`);
  return reply;
};
const json = async (reply, what) => (await ok(reply, what)).json();
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const check = (good, what) => {
  if (!good) throw new Error(`FAILED: ${what}`);
  console.log(`ok: ${what}`);
};

// ── Setup ────────────────────────────────────────────────────────────

const browser = await chromium.launch();
const setup = await browser.newContext({ baseURL: base });
const api = setup.request;
await ok(await api.post('/api/v1/auth/register', { data: { username: 'admin', password: 'password123' } }), 'register');
const installed = await json(await api.post('/api/v1/admin/plugins', { headers: H, data: readFileSync('/e2e/rc-photo.zip') }), 'install');
console.log(`installed ${installed.id} ${installed.manifest?.version}`);
const settings = installed.manifest?.server_settings ?? [];
check(
  ['vision', 'stories'].every((key) => settings.find((s) => s.key === key)?.default === true),
  'the switches vision and stories are on by default in the manifest',
);
await json(await api.put('/api/v1/admin/plugins/rc-photo', { headers: H, data: { enabled: true } }), 'enable');

const root = (await json(await api.get('/api/v1/files/root'), 'root')).id;
const folder = (await json(await api.post(`/api/v1/files/${root}/folders`, { headers: H, data: { name: 'Фото' } }), 'folder')).id;
const names = readdirSync('/e2e/vision').filter((name) => name.endsWith('.jpg')).sort();
const ids = {};
const byName = {};
for (const name of names) {
  const node = await json(
    await api.post(`/api/v1/files/${folder}/upload?name=${encodeURIComponent(name)}`, { headers: H, data: readFileSync(`/e2e/vision/${name}`) }),
    `upload ${name}`,
  );
  ids[node.id] = name;
  byName[name] = node.id;
}
await json(await api.post('/api/v1/grants', { headers: H, data: { app_id: 'rc-photo', node_id: folder, write: false, kind: 'standing' } }), 'grant');
console.log(`photos: ${names.join(', ')}`);

const state = async () => json(await api.get('/app/rc-photo/api/state', { headers: H }), 'state');
const left = (s, kinds) => kinds.reduce((sum, kind) => sum + (s.progress[kind]?.waiting ?? 0), 0);
async function until(what, done, limit) {
  const started = Date.now();
  for (;;) {
    const s = await state();
    if (done(s)) return { s, seconds: (Date.now() - started) / 1000 };
    if (Date.now() - started > limit) throw new Error(`${what}: ${JSON.stringify({ progress: s.progress, analysis: s.analysis })}`);
    await sleep(2000);
  }
}
let { s, seconds } = await until('media jobs', (s) => !left(s, ['media', 'scan']) && s.shards.length, 120_000);
console.log(`media jobs done in ${seconds.toFixed(1)} s; analysis: ${JSON.stringify(s.analysis)}, stories: ${s.stories.enabled}`);
check(s.analysis.enabled === true && s.stories.enabled === true, 'analysis and stories are on with nothing saved');

// The models download (≈385 MB); each look at the state adds the jobs once they are there.
({ s, seconds } = await until(
  'models',
  (s) => Object.values(s.analysis.parts ?? {}).every((part) => part !== 'downloading'),
  20 * 60_000,
));
console.log(`models there in ${seconds.toFixed(0)} s: ${JSON.stringify(s.analysis.parts)}`);
check(s.analysis.parts.clip === 'ready' && s.analysis.parts.ocr === 'ready', 'both parts are ready');
({ s, seconds } = await until('analysis jobs', (s) => !left(s, ['clip', 'ocr']), 15 * 60_000));
console.log(`analysis of ${names.length} photos done in ${seconds.toFixed(1)} s: ${JSON.stringify(s.progress)}`);
check(!s.progress.clip?.failed && !s.progress.ocr?.failed, 'no analysis job failed');
check(s.analysis.parts.clip === 'ready' && s.analysis.parts.ocr === 'ready', 'no model failed');

// ── Similar photos and text, through the API ─────────────────────────

const similar = async (name) => json(await api.get(`/app/rc-photo/api/similar/${byName[name]}`, { headers: H }), `similar ${name}`);
for (const name of names) {
  const found = await similar(name);
  console.log(`${name} → ${found.ids.map((id, i) => `${ids[id]} ${found.scores[i]}`).join(', ') || '(none)'}`);
}
for (const kind of ['cat', 'forest']) {
  const found = await similar(`${kind}_1.jpg`);
  check(found.ready && ids[found.ids[0]] === `${kind}_2.jpg`, `the photo most like ${kind}_1 is ${kind}_2`);
}
const unlike = await similar('cake_1.jpg');
check(!unlike.ids.some((id) => ids[id].startsWith('cat_')), 'a cake is like no cat');

const texts = {};
for (const name of names) {
  const text = await json(await api.get(`/app/rc-photo/api/text/${byName[name]}`, { headers: H }), `text ${name}`);
  texts[name] = text.lines;
  if (text.lines.length) console.log(`text of ${name}: ${JSON.stringify(text.lines)}`);
}
check(texts['sign_1.jpg'].join(' ').toLowerCase().includes('самоилова'), 'the street sign is read');
check(!texts['forest_1.jpg'].length, 'a forest has no text');

// ── The page ─────────────────────────────────────────────────────────

const errors = [];
const context = await browser.newContext({ baseURL: base, viewport: { width: 1400, height: 900 }, locale: 'ru-RU' });
await context.request.post('/api/v1/auth/login', { data: { username: 'admin', password: 'password123' } });
const page = await context.newPage();
page.on('pageerror', (e) => errors.push(e.message));
await page.goto('/#/app/rc-photo');
const frame = page.frameLocator('iframe');
const tiles = frame.locator('.tile');
await tiles.first().waitFor();
check((await tiles.count()) === names.length, 'the feed shows every photo');

const open = async (name) => {
  await frame.locator(`.tile[data-id="${byName[name]}"]`).click();
  await frame.locator('.viewer').waitFor();
};
await open('cat_1.jpg');
const likeButton = frame.locator('.viewer .bar button[aria-label="Похожие фото"]');
check((await likeButton.count()) === 1, 'the viewer has "Похожие фото"');
await likeButton.click();
await frame.locator('.viewer').waitFor({ state: 'detached' });
await frame.locator('.chip', { hasText: 'Похожие на cat_1.jpg' }).first().waitFor();
const shownIds = await tiles.evaluateAll((all) => all.map((tile) => tile.dataset.id));
const expected = new Set([byName['cat_1.jpg'], ...(await similar('cat_1.jpg')).ids]);
check(shownIds.length === expected.size && shownIds.every((id) => expected.has(id)), `the feed shows cat_1 and the photos like it (${shownIds.map((id) => ids[id]).join(', ')})`);
await page.screenshot({ path: '/out/vision-similar.png' });
await frame.locator('.chip', { hasText: 'Похожие на cat_1.jpg' }).first().click();
await frame.locator('.chip', { hasText: 'Похожие на' }).first().waitFor({ state: 'detached' });
check((await tiles.count()) === names.length, 'clearing the chip shows every photo again');

await page.reload(); // goto to the same address would only change the hash
await tiles.first().waitFor();
await open('sign_1.jpg');
// The viewer remembers whether its details were open.
if (await frame.locator('.viewer .info').isHidden()) await frame.locator('.viewer .bar button[aria-label="Сведения"]').click();
const textRow = frame.locator('.viewer .info .info-row', { hasText: 'Текст на фото' });
await textRow.waitFor({ timeout: 10_000 });
check((await textRow.textContent()).toLowerCase().includes('самоилова'), 'the viewer shows the text of the photo');
await page.screenshot({ path: '/out/vision-text.png' });

// Search finds the text read in photos (since 0.6.1).
await page.reload(); // goto to the same address would only change the hash
await tiles.first().waitFor();
await frame.locator('.topbar .search input').fill('самоилова');
await frame.locator('.tile').nth(1).waitFor({ state: 'detached' }).catch(() => {});
const searched = await tiles.evaluateAll((all) => all.map((tile) => tile.dataset.id));
check(searched.length === 1 && searched[0] === byName['sign_1.jpg'], `search by text in photos finds the sign (${searched.map((id) => ids[id]).join(', ')})`);
await frame.locator('.topbar .search input').fill('');

// ── The administrator's switch ───────────────────────────────────────

await json(await api.put('/api/v1/admin/plugins/rc-photo/settings', { headers: H, data: { vision: false } }), 'vision off');
s = await state();
check(s.analysis.enabled === false && s.stories.enabled === true, 'turned off, analysis says so; stories stay on');
await page.reload(); // goto to the same address would only change the hash
await tiles.first().waitFor();
await open('cat_1.jpg');
check((await likeButton.count()) === 0, 'turned off, the viewer has no "Похожие фото"');

// An emptied field gives the default again: on, and every photo gets its jobs again, skipped at once.
await json(await api.put('/api/v1/admin/plugins/rc-photo/settings', { headers: H, data: {} }), 'vision default');
({ s, seconds } = await until('analysis again', (s) => s.analysis.enabled && !left(s, ['clip', 'ocr']), 120_000));
console.log(`on again: jobs for ${names.length} photos done in ${seconds.toFixed(1)} s (done ones are skipped)`);
check(ids[(await similar('cat_1.jpg')).ids[0]] === 'cat_2.jpg', 'similar photos work again');

check(!errors.length, `no page errors ${errors.join('; ')}`);
await browser.close();
console.log('VISION E2E OK');
