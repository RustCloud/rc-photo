// End-to-end check of file dates in rc-photo (0.8.1) on a fresh RustCloud
// with API 0.2.21: a picture without a shot time is dated by the times its
// client gave at upload — the earlier of `ctime` and `mtime` — not by the
// upload day; a time changed later (PATCH `born_at`) reaches the index on
// "Rebuild" without ffmpeg.
//
//   node dates.mjs <base url>   (in mcr.microsoft.com/playwright, /e2e = this folder)
//
// With /e2e/rc-photo-old.zip (0.8.0, not in git) the gallery is first made by
// it — every picture on the upload day — and the update to rc-photo.zip must
// date them again on its first look, without walking the folders.
import { existsSync, readFileSync } from 'node:fs';
import { request } from 'playwright';

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

// A 1×1 PNG: no EXIF, so no shot time.
const png = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==',
  'base64',
);
const today = new Date().toISOString().slice(0, 10);

const api = await request.newContext({ baseURL: base });
await ok(await api.post('/api/v1/auth/register', { data: { username: 'admin', password: 'password123' } }), 'register');
const old = existsSync('/e2e/rc-photo-old.zip');
async function install(zip) {
  const plugin = await json(await api.post('/api/v1/admin/plugins', { headers: H, data: readFileSync(zip) }), `install ${zip}`);
  console.log(`installed ${plugin.manifest.version}, enabled: ${plugin.enabled}`);
  return plugin;
}
await install(old ? '/e2e/rc-photo-old.zip' : '/e2e/rc-photo.zip');
await json(await api.put('/api/v1/admin/plugins/rc-photo', { headers: H, data: { enabled: true } }), 'enable');
await json(await api.put('/api/v1/admin/plugins/rc-photo/settings', { headers: H, data: { vision: false } }), 'vision off');

const root = (await json(await api.get('/api/v1/files/root'), 'root')).id;
const folder = (await json(await api.post(`/api/v1/files/${root}/folders`, { headers: H, data: { name: 'Фото' } }), 'folder')).id;

const upload = async (name, query = '') =>
  json(await api.post(`/api/v1/files/${folder}/upload?name=${name}${query}`, { headers: H, data: png }), `upload ${name}`);
// Shot on a phone that knew when: ctime earlier than mtime.
const both = await upload('both.png', '&ctime=2023-03-14T10:00:00Z&mtime=2024-01-01T00:00:00Z');
// Copied on Windows: born when copied, changed long before.
const copied = await upload('copied.png', '&ctime=2026-11-01T08:00:00Z&mtime=2022-05-01T12:00:00Z');
// Only mtime: born is the upload time, mtime wins.
const mtime = await upload('mtime.png', '&mtime=2021-07-07T07:07:07Z');
// Nothing said: the upload day.
const bare = await upload('bare.png');
// In parts, ctime in the start.
const started = await json(
  await api.post(`/api/v1/files/${folder}/uploads`, {
    headers: H,
    data: { name: 'parts.png', size: png.length, ctime: '2020-02-02T02:02:02Z' },
  }),
  'start upload',
);
await json(await api.put(`/api/v1/files/uploads/${started.id}?offset=0`, { headers: H, data: png }), 'part');
const { createHash } = await import('node:crypto');
const parts = await json(
  await api.post(`/api/v1/files/uploads/${started.id}/finish`, {
    headers: H,
    data: { sha256: createHash('sha256').update(png).digest('hex') },
  }),
  'finish',
);
check(both.born_at?.startsWith('2023-03-14T10:00:00'), `the server keeps ctime as born_at (${both.born_at})`);

await json(await api.post('/api/v1/grants', { headers: H, data: { app_id: 'rc-photo', node_id: folder, write: false, kind: 'standing' } }), 'grant');

const state = async () => json(await api.get(`/app/rc-photo/api/state?today=${today}`, { headers: H }), 'state');
const left = (s) => ['media', 'scan'].reduce((sum, kind) => sum + (s.progress[kind]?.waiting ?? 0), 0);
async function index(count) {
  const started = Date.now();
  for (;;) {
    const s = await state();
    const entries = [];
    for (const url of s.shards) entries.push(...(await json(await api.get(url), 'shard')));
    if (!left(s) && entries.length >= count) return Object.fromEntries(entries.map((e) => [e.id, e]));
    if (Date.now() - started > 120_000) throw new Error(`index: ${JSON.stringify(s.progress)}, ${entries.length} entries`);
    await sleep(1000);
  }
}

let entries = await index(5);
if (old) {
  check(Object.values(entries).every((e) => e.modified.startsWith(today)), 'the old gallery puts every picture on the upload day');
  check((await install('/e2e/rc-photo.zip')).enabled, 'the update stays on: it asks for nothing new');
  const first = await state();
  check((first.progress.media?.waiting ?? 0) > 0 && !first.progress.scan?.waiting, 'the first look sends the old entries to be dated, no walk');
  await sleep(1500);
  entries = await index(5);
} else {
  const again = await state();
  check(!again.progress.media?.waiting && !again.progress.scan?.waiting, 'a fresh gallery sends nothing to be dated again');
}
const day = (node) => entries[node.id]?.modified?.slice(0, 10);
for (const node of [both, copied, mtime, bare, parts]) console.log(`${node.name}: ${entries[node.id]?.modified}`);
check(Object.values(entries).every((e) => !e.taken), 'the pictures have no shot time');
check(day(both) === '2023-03-14', 'ctime earlier than mtime dates the picture');
check(day(copied) === '2022-05-01', 'mtime earlier than ctime (a copy) dates the picture');
check(day(mtime) === '2021-07-07', 'mtime alone dates the picture');
check(day(bare) === today, 'a picture without client times keeps the upload day');
check(day(parts) === '2020-02-02', 'ctime of an upload in parts dates the picture');

// A time changed later: no file event, "Rebuild" picks it up.
await json(await api.patch(`/api/v1/files/${bare.id}`, { headers: H, data: { born_at: '2019-09-09T09:09:09Z' } }), 'patch born_at');
await json(await api.post('/app/rc-photo/api/rebuild', { headers: H }), 'rebuild');
await sleep(1500);
entries = await index(5);
console.log(`bare.png after rebuild: ${entries[bare.id]?.modified}`);
check(day(bare) === '2019-09-09', 'Rebuild brings a changed born_at into the index');
check(entries[bare.id].thumb, 'the picture keeps its thumbnail');

console.log('DATES OK');
await api.dispose();
