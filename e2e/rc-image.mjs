// End-to-end check of the rc-image (Photos) plugin on a fresh RustCloud with ffmpeg:
// install, a folder of photos and videos, the jobs, then the page in a real
// browser — timeline, filters, viewer — with screenshots in /out.
//
//   node rc-image.mjs <base url>      (in mcr.microsoft.com/playwright, /e2e = this folder)
import { readFileSync, existsSync } from 'node:fs';
import { chromium } from 'playwright';

const base = process.argv[2];
const H = { 'x-requested-with': 'e2e' };
const ok = async (reply, what) => {
  if (!reply.ok()) throw new Error(`${what}: ${reply.status()} ${await reply.text()}`);
  return reply;
};
const json = async (reply, what) => (await ok(reply, what)).json();

// ── EXIF, written by hand: a little-endian TIFF block in an APP1 segment ──

const ASCII = 2, SHORT = 3, LONG = 4, RATIONAL = 5;
const ascii = (text) => ({ type: ASCII, count: text.length + 1, data: Buffer.from(text + '\0', 'latin1') });
const short = (n) => {
  const data = Buffer.alloc(2);
  data.writeUInt16LE(n);
  return { type: SHORT, count: 1, data };
};
const long = (n) => {
  const data = Buffer.alloc(4);
  data.writeUInt32LE(n);
  return { type: LONG, count: 1, data };
};
const rationals = (...pairs) => {
  const data = Buffer.alloc(pairs.length * 8);
  pairs.forEach(([top, bottom], i) => {
    data.writeUInt32LE(top, i * 8);
    data.writeUInt32LE(bottom, i * 8 + 4);
  });
  return { type: RATIONAL, count: pairs.length, data };
};

/** One IFD at `start` (from the TIFF header), its long values after it. */
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
    if (entry.data.length <= 4) {
      entry.data.copy(head, at + 8);
    } else {
      head.writeUInt32LE(offset, at + 8);
      const padded = entry.data.length % 2 ? Buffer.concat([entry.data, Buffer.alloc(1)]) : entry.data;
      tail.push(padded);
      offset += padded.length;
    }
  });
  return Buffer.concat([head, ...tail]);
}

function exifSegment(p) {
  const [date, time] = p.date.split(' ');
  const exif = {
    0x829a: rationals([1, p.shutter]),
    0x829d: rationals([Math.round(p.f * 10), 10]),
    0x8827: short(p.iso),
    0x9003: ascii(`${date.replaceAll('-', ':')} ${time}`),
    0x920a: rationals([Math.round(p.focal * 10), 10]),
  };
  if (p.lens) exif[0xa434] = ascii(p.lens);
  const dms = (value) => {
    const v = Math.abs(value);
    const d = Math.floor(v);
    const m = Math.floor((v - d) * 60);
    const s = Math.round(((v - d) * 60 - m) * 60 * 100);
    return rationals([d, 1], [m, 1], [s, 100]);
  };
  const gps = p.gps && {
    0x0001: ascii(p.gps[0] >= 0 ? 'N' : 'S'),
    0x0002: dms(p.gps[0]),
    0x0003: ascii(p.gps[1] >= 0 ? 'E' : 'W'),
    0x0004: dms(p.gps[1]),
  };
  const ifd0 = (exifAt, gpsAt) => ({
    0x010f: ascii(p.make),
    0x0110: ascii(p.model),
    0x0112: short(1),
    0x8769: long(exifAt),
    ...(gps ? { 0x8825: long(gpsAt) } : {}),
  });
  const size0 = ifd(ifd0(0, 0), 8).length;
  const exifBlock = ifd(exif, 8 + size0);
  const gpsAt = 8 + size0 + exifBlock.length;
  const tiff = Buffer.concat([
    Buffer.from([0x49, 0x49, 0x2a, 0x00, 0x08, 0x00, 0x00, 0x00]),
    ifd(ifd0(8 + size0, gpsAt), 8),
    exifBlock,
    gps ? ifd(gps, gpsAt) : Buffer.alloc(0),
  ]);
  const length = Buffer.alloc(2);
  length.writeUInt16BE(2 + 6 + tiff.length);
  return Buffer.concat([Buffer.from([0xff, 0xe1]), length, Buffer.from('Exif\0\0', 'latin1'), tiff]);
}

/** A JPEG with the EXIF segment right after SOI. */
const withExif = (jpeg, p) => Buffer.concat([jpeg.subarray(0, 2), exifSegment(p), jpeg.subarray(2)]);

// ── The photos ───────────────────────────────────────────────────────

const CAMERAS = [
  { make: 'Apple', model: 'iPhone 15 Pro', lens: 'iPhone 15 Pro back triple camera 6.765mm f/1.78', f: 1.8, focal: 6.8 },
  { make: 'Canon', model: 'Canon EOS R6', lens: 'RF24-105mm F4 L IS USM', f: 4, focal: 50 },
  { make: 'FUJIFILM', model: 'X100V', lens: null, f: 2, focal: 23 },
];
const SHAPES = [
  [1600, 1200],
  [1200, 1600],
  [1400, 1400],
  [1600, 1067],
  [3600, 1100],
];
const PLACES = [[55.7558, 37.6173], [59.9343, 30.3351], [43.5855, 39.7231], null];
const photos = [];
for (let i = 0; i < 46; i++) {
  const month = (i * 7) % 12;
  const year = 2023 + Math.floor(i / 14);
  const day = 1 + ((i * 5) % 27);
  const camera = CAMERAS[i % 3];
  const [width, height] = SHAPES[(i * 3) % SHAPES.length];
  photos.push({
    name: `IMG_${String(4100 + i).padStart(4, '0')}.jpg`,
    folder: i % 4 === 0 ? 'Отпуск' : i % 4 === 1 ? 'Дом' : null,
    year,
    width,
    height,
    hue: (i * 47) % 360,
    date: `${year}-${String(month + 1).padStart(2, '0')}-${String(day).padStart(2, '0')} ${String(8 + (i % 12)).padStart(2, '0')}:${String((i * 13) % 60).padStart(2, '0')}:00`,
    shutter: [60, 125, 250, 1000][i % 4],
    iso: [100, 200, 400, 1600][i % 4],
    gps: PLACES[i % PLACES.length],
    ...camera,
  });
}

// ── Setup: admin, plugin, folders, files, grant ──────────────────────

const browser = await chromium.launch();
const setup = await browser.newContext({ baseURL: base });
const api = setup.request;
await ok(await api.post('/api/v1/auth/register', { data: { username: 'admin', password: 'password123' } }), 'register');
const installed = await json(await api.post('/api/v1/admin/plugins', { headers: H, data: readFileSync('/e2e/rc-image.zip') }), 'install');
console.log('installed:', JSON.stringify(installed).slice(0, 300));
await json(await api.put('/api/v1/admin/plugins/rc-image', { headers: H, data: { enabled: true } }), 'enable');

const root = (await json(await api.get('/api/v1/files/root'), 'root')).id;
const folder = async (parent, name) =>
  (await json(await api.post(`/api/v1/files/${parent}/folders`, { headers: H, data: { name } }), `folder ${name}`)).id;
const upload = async (parent, name, data) =>
  json(
    await api.post(`/api/v1/files/${parent}/upload?name=${encodeURIComponent(name)}`, { headers: H, data }),
    `upload ${name}`,
  );

const photosFolder = await folder(root, 'Фото');
const years = {};
for (const year of [2023, 2024, 2025, 2026]) years[year] = await folder(photosFolder, String(year));
const sub = {};

// The pictures themselves: drawn on a canvas, so each looks different.
const painter = await (await browser.newContext()).newPage();
for (const photo of photos) {
  const dataUrl = await painter.evaluate(({ width, height, hue, name, date }) => {
    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    const g = canvas.getContext('2d');
    const sky = g.createLinearGradient(0, 0, width, height);
    sky.addColorStop(0, `hsl(${hue} 70% 62%)`);
    sky.addColorStop(1, `hsl(${(hue + 60) % 360} 60% 30%)`);
    g.fillStyle = sky;
    g.fillRect(0, 0, width, height);
    for (let i = 0; i < 6; i++) {
      g.fillStyle = `hsla(${(hue + i * 40) % 360} 80% 70% / 0.35)`;
      g.beginPath();
      g.arc(((i * 0.37 + 0.1) % 1) * width, ((i * 0.53 + 0.2) % 1) * height, Math.min(width, height) * (0.12 + i * 0.03), 0, Math.PI * 2);
      g.fill();
    }
    g.fillStyle = 'rgba(255,255,255,0.9)';
    g.font = `bold ${Math.round(Math.min(width, height) / 12)}px sans-serif`;
    g.fillText(name, width * 0.06, height * 0.85);
    g.font = `${Math.round(Math.min(width, height) / 20)}px sans-serif`;
    g.fillText(date, width * 0.06, height * 0.93);
    return canvas.toDataURL('image/jpeg', 0.88);
  }, photo);
  const jpeg = Buffer.from(dataUrl.split(',')[1], 'base64');
  let parent = years[photo.year];
  if (photo.folder) {
    const key = `${photo.year}/${photo.folder}`;
    sub[key] ??= await folder(parent, photo.folder);
    parent = sub[key];
  }
  await upload(parent, photo.name, withExif(jpeg, photo));
}
for (const name of ['photo.heic', 'video.mp4', 'hevc.mkv', 'long.mkv']) {
  if (existsSync(`/e2e/samples/${name}`)) await upload(years[2026], name, readFileSync(`/e2e/samples/${name}`));
}
// Something the gallery must skip.
await upload(photosFolder, 'notes.txt', Buffer.from('not a photo'));

await json(
  await api.post('/api/v1/grants', { headers: H, data: { app_id: 'rc-image', node_id: photosFolder, write: false, kind: 'standing' } }),
  'grant',
);

// ── The jobs ─────────────────────────────────────────────────────────

const started = Date.now();
let state;
for (;;) {
  state = await json(await api.get('/app/rc-image/api/state', { headers: H }), 'state');
  const left = Object.values(state.progress).reduce((sum, kind) => sum + kind.waiting, 0);
  if (!left && state.shards.length) break;
  if (Date.now() - started > 180_000) throw new Error(`jobs not done: ${JSON.stringify(state.progress)}`);
  await new Promise((resolve) => setTimeout(resolve, 1000));
}
console.log(`jobs done in ${((Date.now() - started) / 1000).toFixed(1)} s:`, JSON.stringify(state.progress), `ffmpeg=${state.ffmpeg}`);
let entries = [];
for (const url of state.shards) entries.push(...(await json(await api.get(url), 'shard')));
console.log(`index: ${entries.length} entries in ${state.shards.length} shards`);
const sample = entries.find((e) => e.name === 'IMG_4100.jpg');
console.log('IMG_4100:', JSON.stringify(sample));
for (const e of entries.filter((e) => !e.name.endsWith('.jpg'))) {
  console.log(`${e.name}: ${e.mime} ${e.container} ${e.width}x${e.height} thumb=${e.thumb} preview=${e.preview} codec=${e.codec ?? '-'} error=${e.error ?? '-'}`);
}
const links = await json(
  await api.post('/app/rc-image/api/links', { headers: H, data: { items: [{ id: sample.id, kind: 'thumb' }, { id: sample.id, kind: 'original' }] } }),
  'links',
);
const thumb = await fetch(base + links.urls[0]);
console.log(`thumb link: ${thumb.status} ${thumb.headers.get('content-type')} ${(await thumb.arrayBuffer()).byteLength} B, cors=${thumb.headers.get('access-control-allow-origin')}`);

// ── The page ─────────────────────────────────────────────────────────

const errors = [];
async function look(colorScheme, prefix) {
  const context = await browser.newContext({ baseURL: base, locale: 'ru-RU', colorScheme, viewport: { width: 1440, height: 900 } });
  await context.request.post('/api/v1/auth/login', { data: { username: 'admin', password: 'password123' } });
  const page = await context.newPage();
  page.on('console', (m) => m.type() === 'error' && errors.push(`${prefix}: ${m.text()}`));
  page.on('pageerror', (e) => errors.push(`${prefix}: ${e.message}`));
  await page.goto('/#/app/rc-image');
  const frame = page.frameLocator('iframe');
  await frame.locator('.tile img.loaded').first().waitFor({ timeout: 30_000 });
  await page.waitForTimeout(1500);
  await page.screenshot({ path: `/out/${prefix}-timeline.png` });
  console.log(`${prefix}: tiles drawn ${await frame.locator('.tile').count()}, with pictures ${await frame.locator('.tile img.loaded').count()}`);
  console.log(`${prefix}: header "${(await frame.locator('.topbar').innerText()).replace(/\s+/g, ' ')}"`);
  return { context, page, frame };
}

const light = await look('light', 'light');
const { page, frame } = light;

// Filters: a camera, then a year.
await frame.locator('.filters .check', { hasText: 'Canon EOS R6' }).click();
await page.waitForTimeout(500);
console.log(`camera filter: "${await frame.locator('.topbar .count').innerText()}", chips: ${await frame.locator('.chips .chip').count()}`);
await frame.locator('.filters .pill', { hasText: '2024' }).click();
await page.waitForTimeout(500);
console.log(`+ year 2024: "${await frame.locator('.topbar .count').innerText()}"`);
await page.screenshot({ path: '/out/light-filtered.png' });
await frame.locator('.chips .chip.clear').click();
await frame.locator('.search input').fill('отпуск');
await page.waitForTimeout(600);
console.log(`search "отпуск": "${await frame.locator('.topbar .count').innerText()}"`);
await frame.locator('.search input').fill('');
await page.waitForTimeout(600);

// The viewer: open, wait for the original, info, next, favourite.
await frame.locator('.tile[aria-label$=".jpg"]').first().click();
await frame.locator('.viewer .media img.full.loaded').waitFor({ timeout: 20_000 });
await page.waitForTimeout(600);
await page.screenshot({ path: '/out/light-viewer.png' });
console.log(`viewer: "${(await frame.locator('.viewer .bar .title').innerText()).replace(/\s+/g, ' ')}"`);
console.log(`info: ${(await frame.locator('.viewer .info').innerText()).replace(/\s+/g, ' ').slice(0, 300)}`);
await page.keyboard.press('ArrowRight');
await page.waitForTimeout(800);
await page.keyboard.press('f');
await page.waitForTimeout(800);
console.log(`after → and f: "${(await frame.locator('.viewer .bar .title .name').innerText())}", favorite on: ${await frame.locator('.viewer .bar .icon-btn.on').count()}`);
await page.mouse.move(700, 450);
await page.mouse.wheel(0, -600);
await page.waitForTimeout(400);
console.log(`zoom transform: ${await frame.locator('.viewer .media').evaluate((el) => el.style.transform)}`);
await page.screenshot({ path: '/out/light-zoom.png' });
await page.keyboard.press('Escape');
await page.waitForTimeout(600);
console.log(`closed: viewers left ${await frame.locator('.viewer').count()}, favorites ${JSON.stringify((await json(await api.get('/app/rc-image/api/state', { headers: H }), 'state')).favorites.length)}`);

// HEIC: shown by its preview. Video: plays.
for (const name of ['photo.heic', 'video.mp4', 'hevc.mkv']) {
  if (!entries.some((e) => e.name === name)) continue;
  await frame.locator('.search input').fill(name.split('.')[0]);
  await page.waitForTimeout(600);
  await frame.locator('.tile').first().click();
  if (name.endsWith('heic')) {
    await frame.locator('.viewer .media img.full.loaded').waitFor({ timeout: 20_000 });
  } else {
    await frame
      .locator('.viewer video')
      .evaluate((video) => new Promise((resolve) => (video.readyState >= 2 ? resolve() : video.addEventListener('loadeddata', resolve, { once: true }))))
      .catch(() => {});
    const video = await frame.locator('.viewer video').evaluate((v) => ({ ready: v.readyState, src: v.currentSrc.slice(0, 60), error: v.error?.code ?? null }));
    console.log(`${name}: ${JSON.stringify(video)} status="${await frame.locator('.viewer .status').innerText()}"`);
  }
  await page.waitForTimeout(1200);
  await page.screenshot({ path: `/out/light-${name.replace('.', '-')}.png` });
  await page.keyboard.press('Escape');
  await page.waitForTimeout(400);
}
await frame.locator('.search input').fill('');

// Dark theme, and a narrow window.
const dark = await look('dark', 'dark');
await dark.context.close();
await page.setViewportSize({ width: 390, height: 844 });
await page.waitForTimeout(1000);
await page.screenshot({ path: '/out/light-phone.png' });

// Events and Rebuild: a deleted file leaves the gallery, Rebuild finds the rest again.
const countEntries = async () => {
  for (const started = Date.now(); ; ) {
    const now = await json(await api.get('/app/rc-image/api/state', { headers: H }), 'state');
    if (!Object.values(now.progress).some((kind) => kind.waiting)) {
      let n = 0;
      for (const url of now.shards) n += (await json(await api.get(url), 'shard')).length;
      return n;
    }
    if (Date.now() - started > 120_000) throw new Error('jobs not done');
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }
};
await ok(await api.delete(`/api/v1/files/${sample.id}`, { headers: H }), 'delete');
await new Promise((resolve) => setTimeout(resolve, 1500));
console.log(`after deleting IMG_4100: ${await countEntries()} entries`);
await json(await api.post('/app/rc-image/api/rebuild', { headers: H }), 'rebuild');
await new Promise((resolve) => setTimeout(resolve, 1500));
console.log(`after Rebuild: ${await countEntries()} entries`);

// A new user: the invitation, and the panel's access dialog behind its button.
await json(
  await api.post('/api/v1/admin/users', { headers: H, data: { username: 'bob', password: 'password123' } }),
  'user',
);
const bob = await browser.newContext({ baseURL: base, locale: 'ru-RU', viewport: { width: 1280, height: 800 } });
await bob.request.post('/api/v1/auth/login', { headers: H, data: { username: 'bob', password: 'password123' } });
const bobPage = await bob.newPage();
bobPage.on('pageerror', (e) => errors.push(`bob: ${e.message}`));
await bobPage.goto('/#/app/rc-image');
const choose = bobPage.frameLocator('iframe').getByRole('button', { name: 'Выбрать папку' });
await choose.waitFor({ timeout: 15_000 });
await bobPage.screenshot({ path: '/out/bob-empty.png' });
await choose.click();
await bobPage.getByRole('button', { name: 'Разрешить' }).waitFor({ timeout: 10_000 });
await bobPage.screenshot({ path: '/out/bob-access.png' });
await bobPage.getByRole('button', { name: 'Разрешить' }).click();
await bobPage.frameLocator('iframe').getByText('Фото пока нет').waitFor({ timeout: 15_000 });
console.log(`bob: folder given, "${(await bobPage.frameLocator('iframe').locator('.empty h2').innerText())}"`);
await bob.close();

console.log(errors.length ? `console errors:\n${errors.join('\n')}` : 'no console errors');
await browser.close();
