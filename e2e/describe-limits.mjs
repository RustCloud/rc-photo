// End-to-end check of search by description (0.8.0) on a server with the
// default [onnx] limits (max_model_mib 1024, memory_mib 2048): the text model
// is too large, so the page says what the administrator should raise, and
// the rest of photo analysis works.
//
//   node describe-limits.mjs <base url>   (in mcr.microsoft.com/playwright, /e2e = this folder)
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

const browser = await chromium.launch();
const api = (await browser.newContext({ baseURL: base })).request;
await ok(await api.post('/api/v1/auth/register', { data: { username: 'admin', password: 'password123' } }), 'register');
await json(await api.post('/api/v1/admin/plugins', { headers: H, data: readFileSync('/e2e/rc-photo.zip') }), 'install');
await json(await api.put('/api/v1/admin/plugins/rc-photo', { headers: H, data: { enabled: true } }), 'enable');
const root = (await json(await api.get('/api/v1/files/root'), 'root')).id;
const folder = (await json(await api.post(`/api/v1/files/${root}/folders`, { headers: H, data: { name: 'Фото' } }), 'folder')).id;
for (const name of readdirSync('/e2e/vision').filter((name) => name.startsWith('cat_'))) {
  await json(await api.post(`/api/v1/files/${folder}/upload?name=${name}`, { headers: H, data: readFileSync(`/e2e/vision/${name}`) }), `upload ${name}`);
}
await json(await api.post('/api/v1/grants', { headers: H, data: { app_id: 'rc-photo', node_id: folder, write: false, kind: 'standing' } }), 'grant');

const state = async () => json(await api.get('/app/rc-photo/api/state', { headers: H }), 'state');
let s;
for (const started = Date.now(); ; await sleep(2000)) {
  s = await state();
  const text = String(s.analysis?.parts?.text);
  if (text.startsWith('failed') || text === 'ready') break;
  if (Date.now() - started > 20 * 60_000) throw new Error(`the text model: ${JSON.stringify(s.analysis)}`);
}
console.log(`analysis: ${JSON.stringify(s.analysis)}`);
check(String(s.analysis.parts.text).startsWith('failed'), 'the text model fails with the default limits');
check(s.analysis.parts.clip === 'ready', 'the picture model works with them');
const reply = await json(await api.get('/app/rc-photo/api/describe?q=кот', { headers: H }), 'describe');
check(reply.ready === false && String(reply.why).startsWith('failed'), 'search by description says it can’t');
const again = await state();
check(again.analysis.parts.text === s.analysis.parts.text && !again.progress.describe?.waiting, 'it isn’t tried again on each look');

const context = await browser.newContext({ baseURL: base, viewport: { width: 1400, height: 900 }, locale: 'ru-RU' });
await context.request.post('/api/v1/auth/login', { data: { username: 'admin', password: 'password123' } });
const page = await context.newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
await page.goto('/#/app/rc-photo');
const frame = page.frameLocator('iframe');
const hint = frame.locator('.describe-failed');
await hint.waitFor({ timeout: 20_000 });
check((await hint.textContent()).includes('Поиск по описанию недоступен'), 'the page says search by description is off');
check((await frame.locator('.hint', { hasText: 'memory_mib = 3072' }).count()) === 1, 'and what the administrator should raise');
await page.screenshot({ path: '/out/describe-limits.png' });
// Words still search by name.
await frame.locator('.topbar .search input').fill('cat_1');
await sleep(1500);
check((await frame.locator('.tile').count()) === 1, 'search by name still works');
check(!errors.length, `no page errors ${errors.join('; ')}`);
await browser.close();
console.log('DESCRIBE LIMITS E2E OK');
