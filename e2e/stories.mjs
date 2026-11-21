// End-to-end check of stories in rc-photo (0.7.0) on a fresh RustCloud:
// photos taken this day a year ago make the day's story ("Год назад"); the
// row of circles above the feed, the player (bars, taps, Esc), seen, "don't
// show this again", the next day (the theme stays hidden), the
// administrator's switch.
//
//   node stories.mjs <base url>   (in mcr.microsoft.com/playwright, /e2e = this folder)
//
// Photos in /e2e/vision/*.jpg (not in git, the same as vision.mjs): their own
// EXIF is replaced with a date a year ago.
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

// ── A JPEG with a date: its APP1 segments replaced by one EXIF block ──

const ascii = (text) => ({ type: 2, count: text.length + 1, data: Buffer.from(text + '\0', 'latin1') });
const long = (n) => {
  const data = Buffer.alloc(4);
  data.writeUInt32LE(n);
  return { type: 4, count: 1, data };
};
function ifd(entries, start) {
  const list = Object.entries(entries)
    .map(([tag, value]) => ({ tag: Number(tag), ...value }))
    .sort((a, b) => a.tag - b.tag);
  const size = 2 + list.length * 12 + 4;
  const head = Buffer.alloc(size);
  head.writeUInt16LE(list.length, 0);
  const tail = [];
  let offset = start + size;
  list.forEach((entry, i) => {
    const at = 2 + i * 12;
    head.writeUInt16LE(entry.tag, at);
    head.writeUInt16LE(entry.type, at + 2);
    head.writeUInt32LE(entry.count, at + 4);
    if (entry.data.length <= 4) entry.data.copy(head, at + 8);
    else {
      head.writeUInt32LE(offset, at + 8);
      const padded = entry.data.length % 2 ? Buffer.concat([entry.data, Buffer.alloc(1)]) : entry.data;
      tail.push(padded);
      offset += padded.length;
    }
  });
  return Buffer.concat([head, ...tail]);
}
function exifSegment(when) {
  const exif = { 0x9003: ascii(when.replace(/^(\d{4})-(\d{2})-(\d{2})/, '$1:$2:$3')) };
  const ifd0 = (exifAt) => ({ 0x010f: ascii('E2E'), 0x0110: ascii('Camera'), 0x8769: long(exifAt) });
  const size0 = ifd(ifd0(0), 8).length;
  const tiff = Buffer.concat([
    Buffer.from([0x49, 0x49, 0x2a, 0x00, 0x08, 0x00, 0x00, 0x00]),
    ifd(ifd0(8 + size0), 8),
    ifd(exif, 8 + size0),
  ]);
  const length = Buffer.alloc(2);
  length.writeUInt16BE(2 + 6 + tiff.length);
  return Buffer.concat([Buffer.from([0xff, 0xe1]), length, Buffer.from('Exif\0\0', 'latin1'), tiff]);
}
function dated(jpeg, when) {
  const kept = [jpeg.subarray(0, 2), exifSegment(when)];
  let at = 2;
  while (at < jpeg.length && jpeg[at] === 0xff && jpeg[at + 1] !== 0xda) {
    const size = jpeg.readUInt16BE(at + 2);
    if (jpeg[at + 1] !== 0xe1) kept.push(jpeg.subarray(at, at + 2 + size));
    at += 2 + size;
  }
  kept.push(jpeg.subarray(at));
  return Buffer.concat(kept);
}

// ── Setup ────────────────────────────────────────────────────────────

const pad = (n) => String(n).padStart(2, '0');
const local = (date) => `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
const now = new Date();
const today = local(now);
const tomorrow = local(new Date(now.getTime() + 86_400_000));
// This day a year ago (29 February → 28 February).
const yearAgo = new Date(now.getFullYear() - 1, now.getMonth(), Math.min(now.getDate(), now.getMonth() === 1 ? 28 : 31));
const thatDay = local(yearAgo);
console.log(`today ${today}, a year ago ${thatDay}`);

const browser = await chromium.launch();
const setup = await browser.newContext({ baseURL: base });
const api = setup.request;
await ok(await api.post('/api/v1/auth/register', { data: { username: 'admin', password: 'password123' } }), 'register');
await json(await api.post('/api/v1/admin/plugins', { headers: H, data: readFileSync('/e2e/rc-photo.zip') }), 'install');
await json(await api.put('/api/v1/admin/plugins/rc-photo', { headers: H, data: { enabled: true } }), 'enable');
// Analysis isn't what this checks: off, so its models don't download.
await json(await api.put('/api/v1/admin/plugins/rc-photo/settings', { headers: H, data: { vision: false } }), 'vision off');

const root = (await json(await api.get('/api/v1/files/root'), 'root')).id;
const folder = (await json(await api.post(`/api/v1/files/${root}/folders`, { headers: H, data: { name: 'Фото' } }), 'folder')).id;
const sources = readdirSync('/e2e/vision').filter((name) => name.endsWith('.jpg')).sort();
const ids = {};
// Eight a year ago, ten minutes apart; three today.
for (let i = 0; i < 11; i++) {
  const when = i < 8 ? `${thatDay} 1${i % 10}:${pad((i * 10) % 60)}:00` : `${today} 09:0${i - 8}:00`;
  const name = `${i < 8 ? 'then' : 'now'}_${i}.jpg`;
  const data = dated(readFileSync(`/e2e/vision/${sources[i % sources.length]}`), when);
  const node = await json(await api.post(`/api/v1/files/${folder}/upload?name=${name}`, { headers: H, data }), `upload ${name}`);
  ids[node.id] = name;
}
await json(await api.post('/api/v1/grants', { headers: H, data: { app_id: 'rc-photo', node_id: folder, write: false, kind: 'standing' } }), 'grant');

const state = async (day = today) => json(await api.get(`/app/rc-photo/api/state?today=${day}`, { headers: H }), 'state');
const left = (s, kinds) => kinds.reduce((sum, kind) => sum + (s.progress[kind]?.waiting ?? 0), 0);
async function until(what, done, limit, day = today) {
  const started = Date.now();
  for (;;) {
    const s = await state(day);
    if (done(s)) return s;
    if (Date.now() - started > limit) throw new Error(`${what}: ${JSON.stringify({ progress: s.progress, stories: s.stories })}`);
    await sleep(1000);
  }
}
let s = await until('media jobs', (s) => !left(s, ['media', 'scan']) && s.shards.length, 120_000);
// The first look after the photos are in adds the day's job.
s = await until('the story', (s) => s.stories.list?.length || (!left(s, ['story']) && s.progress.story?.failed), 120_000);
const story = s.stories.list[0];
console.log(`story: ${JSON.stringify({ ...story, items: story.items.map((item) => `${ids[item.id]} ${item.ms}`) })}`);
check(story.kind === 'on-this-day' && story.params.years === 1 && story.params.date === thatDay, 'the day’s story is this day a year ago');
check(story.items.length === 8 && story.items.every((item) => ids[item.id].startsWith('then_')), 'it has the 8 photos of that day');
check(story.date === today && story.expires === local(new Date(now.getTime() + 2 * 86_400_000)), 'it shows for 3 days');
check(!s.progress.story?.failed, 'the story job didn’t fail');

// ── The page ─────────────────────────────────────────────────────────

const errors = [];
const context = await browser.newContext({ baseURL: base, viewport: { width: 1200, height: 800 }, locale: 'ru-RU' });
await context.request.post('/api/v1/auth/login', { data: { username: 'admin', password: 'password123' } });
const page = await context.newPage();
page.on('pageerror', (e) => errors.push(e.message));
await page.goto('/#/app/rc-photo');
const frame = page.frameLocator('iframe');
const circle = frame.locator('.stories .story');
await circle.first().waitFor();
check((await circle.count()) === 1, 'a circle above the feed');
check((await circle.first().textContent()).includes('Год назад'), 'titled "Год назад"');
check(!(await circle.first().getAttribute('class')).includes('seen'), 'not seen yet');
await page.screenshot({ path: '/out/stories-row.png' });

await circle.first().click();
const player = frame.locator('.story-player');
await player.waitFor();
check((await player.locator('.story-bars span').count()) === 8, 'the player has a bar for each photo');
const whole = await page.locator('iframe').boundingBox();
check(whole.width === 1200 && whole.height === 800, `the player takes the whole window (${whole.width}×${whole.height})`);
check((await player.locator('.story-title').textContent()) === 'Год назад', 'the player shows the title');
await sleep(1500);
await page.screenshot({ path: '/out/stories-player.png' });
const width = async (n) => player.locator('.story-bars span i').nth(n).evaluate((el) => el.style.width);
const box = await player.locator('.story-stage').boundingBox();
await page.mouse.click(box.x + box.width * 0.8, box.y + box.height / 2);
check((await width(0)) === '100%', 'a tap on the right goes on');
await page.mouse.click(box.x + box.width * 0.1, box.y + box.height / 2);
check((await width(0)) !== '100%', 'a tap on the left goes back');
// Photos go on by themselves.
await sleep(5000);
check((await width(0)) === '100%', 'a photo goes on after its time');
await page.keyboard.press('Escape');
await player.waitFor({ state: 'detached' });
check((await circle.first().getAttribute('class')).includes('seen'), 'seen, the circle turns grey');
s = await state();
check(s.stories.seen.includes(today), 'the server knows it was seen');

// Don't show this again: gone, and its theme with it.
await circle.first().click();
await player.waitFor();
await player.locator('button[aria-label="Больше не показывать"]').click();
await player.waitFor({ state: 'detached' });
check((await frame.locator('.stories .story').count()) === 0, 'hidden, the row is gone');
s = await state();
check(!s.stories.list.length, 'hidden on the server too');

// The next day: its photos had a story days ago (a 4-week break), there is no other.
s = await until('the next day', (s) => !left(s, ['story']), 60_000, tomorrow);
s = await state(tomorrow);
check(!s.stories.list.length && !s.progress.story?.failed, 'the next day has no story: its photos are on a break');

// ── The administrator's switch ───────────────────────────────────────

await json(await api.put('/api/v1/admin/plugins/rc-photo/settings', { headers: H, data: { vision: false, stories: false } }), 'stories off');
s = await state();
check(s.stories.enabled === false, 'turned off, stories say so');

check(!errors.length, `no page errors ${errors.join('; ')}`);
await browser.close();
console.log('STORIES E2E OK');
