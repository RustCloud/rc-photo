// End-to-end check of faces in rc-photo on a fresh RustCloud with use.onnx:
// the server downloads the models, the user turns faces on in the page, the
// jobs find faces and group them into people; naming, filtering, the viewer.
//
//   node faces.mjs <base url>   (in mcr.microsoft.com/playwright, /e2e = this folder)
//
// Photos of people in /e2e/faces/<person>_<n>.jpg (not in git): the person
// is the part of the name before the first "_"; a group photo has several.
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

// ── Setup ────────────────────────────────────────────────────────────

const browser = await chromium.launch();
const setup = await browser.newContext({ baseURL: base });
const api = setup.request;
await ok(await api.post('/api/v1/auth/register', { data: { username: 'admin', password: 'password123' } }), 'register');
await json(await api.post('/api/v1/admin/plugins', { headers: H, data: readFileSync('/e2e/rc-photo.zip') }), 'install');
await json(await api.put('/api/v1/admin/plugins/rc-photo', { headers: H, data: { enabled: true } }), 'enable');

const root = (await json(await api.get('/api/v1/files/root'), 'root')).id;
const folder = (await json(await api.post(`/api/v1/files/${root}/folders`, { headers: H, data: { name: 'Люди' } }), 'folder')).id;
const names = readdirSync('/e2e/faces').filter((name) => name.endsWith('.jpg')).sort();
const ids = {};
for (const name of names) {
  const node = await json(
    await api.post(`/api/v1/files/${folder}/upload?name=${encodeURIComponent(name)}`, { headers: H, data: readFileSync(`/e2e/faces/${name}`) }),
    `upload ${name}`,
  );
  ids[node.id] = name;
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
    if (Date.now() - started > limit) throw new Error(`${what}: ${JSON.stringify({ progress: s.progress, faces: s.faces })}`);
    await sleep(1000);
  }
}
let { seconds } = await until('media jobs', (s) => !left(s, ['media', 'scan']) && s.shards.length, 120_000);
console.log(`media jobs done in ${seconds.toFixed(1)} s`);

// ── The page: faces are off until turned on ──────────────────────────

const errors = [];
const context = await browser.newContext({ baseURL: base, viewport: { width: 1400, height: 900 }, locale: 'ru-RU' });
await context.request.post('/api/v1/auth/login', { data: { username: 'admin', password: 'password123' } });
const page = await context.newPage();
page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
page.on('pageerror', (e) => errors.push(e.message));
page.on('response', (r) => r.status() >= 400 && errors.push(`${r.status()} ${r.request().method()} ${r.url().replace(base, '').slice(0, 80)}`));
await page.goto('/#/app/rc-photo');
const frame = page.frameLocator('iframe');
await frame.locator('.tile img.loaded').first().waitFor({ timeout: 30_000 });
const section = frame.locator('.filters section', { has: frame.locator('h3', { hasText: 'Люди' }) });
console.log(`people section, off: "${(await section.innerText()).replace(/\s+/g, ' ')}"`);
await section.getByRole('button', { name: 'Найти людей' }).click();
await frame.locator('dialog[open] h2', { hasText: 'Искать людей' }).waitFor();
await page.screenshot({ path: '/out/faces-ask.png' });
await frame.locator('dialog[open]').getByRole('button', { name: 'Найти людей' }).click();

// The models download (≈185 MB), then the jobs run.
({ seconds } = await until('models', (s) => s.faces.state === 'ready', 600_000));
console.log(`models ready after ${seconds.toFixed(0)} s`);
let s;
({ s, seconds } = await until('faces jobs', (s) => !left(s, ['faces', 'people']) && s.people, 900_000));
console.log(`faces and people done in ${seconds.toFixed(0)} s: ${JSON.stringify(s.progress)}`);

// What the plugin found, through the API.
const people = (await json(await api.get(s.people), 'people')).people;
for (const person of people) {
  console.log(`person ${person.id}: ${person.count} faces in ${person.files.map((id) => ids[id]).join(', ')}`);
}
for (const [id, name] of Object.entries(ids)) {
  const { faces } = await json(await api.get(`/app/rc-photo/api/faces/${id}`, { headers: H }), 'faces');
  console.log(`  ${name}: ${faces.length} faces: ${faces.map((f) => `${f.p ?? '-'}/${f.w}px`).join(' ')}`);
}
// Every photo of a person is that person's, by the file names.
for (const person of people) {
  const who = new Set(person.files.map((id) => ids[id].split('_')[0]));
  console.log(`  ${person.id} is ${[...who].join(' + ')}`);
}

// How close each face is to each person's average face: the faces' shards
// are files of the user's AppData (`.appdata/rc-photo/faces`).
const child = async (parent, name) =>
  (await json(await api.get(`/api/v1/files/${parent}/children`), 'children')).find((node) => node.name === name);
const appdata = await child((await child(root, '.appdata')).id, 'rc-photo');
const shards = await json(await api.get(`/api/v1/files/${(await child(appdata.id, 'faces')).id}/children`), 'shards');
const all = [];
for (const shard of shards) {
  for (const file of await json(await api.get(`/api/v1/files/${shard.id}/content`), 'shard')) {
    for (const face of file.faces) {
      const v = [...Buffer.from(face.v, 'base64')].map((b) => (b > 127 ? b - 256 : b));
      const length = Math.hypot(...v);
      all.push({ name: ids[file.id], p: face.p, w: face.w, v: v.map((x) => x / length) });
    }
  }
}
const dot = (a, b) => a.reduce((sum, x, i) => sum + x * b[i], 0);
for (const person of people) {
  const own = all.filter((face) => face.p === person.id);
  const average = own[0].v.map((_, i) => own.reduce((sum, face) => sum + face.v[i], 0));
  const length = Math.hypot(...average);
  const close = all
    .filter((face) => face.p !== person.id)
    .map((face) => ({ ...face, s: dot(average, face.v) / length }))
    .sort((a, b) => b.s - a.s)
    .slice(0, 3);
  console.log(`  closest to ${person.id}, not in it: ${close.map((f) => `${f.name}/${f.w}px ${f.s.toFixed(2)}`).join(', ')}`);
}

// ── People in the page ───────────────────────────────────────────────

await page.reload();
await frame.locator('.tile img.loaded').first().waitFor({ timeout: 30_000 });
await section.locator('.person').first().waitFor({ timeout: 20_000 });
console.log(`people in the page: ${await section.locator('.person').count()}, faces drawn ${await section.locator('.avatar[style*="background-image"]').count()}`);
await page.waitForTimeout(500);
await page.screenshot({ path: '/out/faces-people.png' });

// Name the one with the most photos.
const first = people[0];
const firstName = ids[first.files[0]].split('_')[0];
await section.locator('.person .name').first().click();
await frame.locator('dialog[open] h2', { hasText: 'Кто это' }).waitFor();
await frame.locator('dialog[open] input[type=text]').fill(firstName);
await page.screenshot({ path: '/out/faces-name.png' });
await frame.locator('dialog[open]').getByRole('button', { name: 'Сохранить' }).click();
await section.locator('.person .name', { hasText: firstName }).waitFor();
console.log(`named ${first.id}: ${JSON.stringify((await state()).names.names)}`);

// Filter by them: their photos only.
await section.locator('.person .face').first().click();
await page.waitForTimeout(300);
console.log(`filter by ${firstName}: "${await frame.locator('.topbar .count').innerText()}", chip "${await frame.locator('.chips .chip').first().innerText()}"`);
await page.screenshot({ path: '/out/faces-filtered.png' });

// The viewer says who is in the photo.
await frame.locator('.tile').first().click();
await frame.locator('.viewer .info .people-chips').waitFor({ timeout: 10_000 });
console.log(`viewer: people "${await frame.locator('.viewer .info .people-chips').innerText()}"`);
await page.screenshot({ path: '/out/faces-viewer.png' });
await page.keyboard.press('Escape');

// Naming a second person with the same name merges them.
if (people.length > 1) {
  await section.locator('.person.on .face').click();
  const second = section.locator('.person', { hasNot: frame.locator('.name', { hasText: firstName }) }).first();
  await second.locator('.name').click();
  await frame.locator('dialog[open] input[type=text]').fill(firstName);
  await frame.locator('dialog[open]').getByRole('button', { name: 'Сохранить' }).click();
  await frame.locator('dialog[open] h2', { hasText: 'Объединить' }).waitFor();
  await frame.locator('dialog[open]').getByRole('button', { name: 'Объединить' }).click();
  await page.waitForTimeout(500);
  console.log(`merged: people in the page ${await section.locator('.person').count()}, names ${JSON.stringify((await state()).names)}`);
  ({ s } = await until('regrouping', (s) => !left(s, ['people']), 120_000));
  const after = (await json(await api.get(s.people), 'people')).people;
  console.log(`after the merge job: ${after.map((p) => `${p.id}:${p.files.length}`).join(' ')}`);
}

// Off, then everything deleted.
const forget = await json(await api.delete('/app/rc-photo/api/faces', { headers: H }), 'forget');
const gone = await state();
console.log(`forgotten: ${JSON.stringify(forget)}, people link ${gone.people}, names ${JSON.stringify(gone.names)}`);

console.log(errors.length ? `console errors:\n${errors.join('\n')}` : 'no console errors');
await browser.close();
