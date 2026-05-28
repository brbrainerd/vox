// Bake docs/src/assets/template_durable.html with the four node PNGs and capture
// 34 seconds of frames via headless Chrome. Frames go to ./frames/, then the
// caller (scripts/render-durable-animation.vox) encodes them to animated WebP
// via ffmpeg.
//
// Usage: node render.js <repo-root> <chrome-path>
const fs = require('fs');
const path = require('path');
const puppeteer = require('puppeteer-core');

const [REPO, CHROME] = process.argv.slice(2);
if (!REPO || !CHROME) {
    console.error('Usage: node render.js <repo-root> <chrome-path>');
    process.exit(2);
}

const ASSETS = path.join(REPO, 'docs', 'src', 'assets');
const TEMPLATE = path.join(ASSETS, 'template_durable.html');
const HERE = __dirname;
const OUT_HTML = path.join(HERE, 'baked.html');
const FRAMES_DIR = path.join(HERE, 'frames');

const SIZE = 960;
const FPS = 20;
const DURATION_S = 34;
const TOTAL_FRAMES = FPS * DURATION_S;

const b64 = name => fs.readFileSync(path.join(ASSETS, name)).toString('base64');

(async () => {
    let html = fs.readFileSync(TEMPLATE, 'utf8');
    html = html
        .replace('{{PORTAL_B64}}', b64('node_portal.png'))
        .replace('{{WORKFLOW_B64}}', b64('node_workflow.png'))
        .replace(/\{\{WORKER_B64\}\}/g, b64('node_worker.png'))
        .replace('{{VAULT_B64}}', b64('node_vault.png'));
    fs.writeFileSync(OUT_HTML, html);

    if (fs.existsSync(FRAMES_DIR)) fs.rmSync(FRAMES_DIR, { recursive: true });
    fs.mkdirSync(FRAMES_DIR);

    const browser = await puppeteer.launch({
        executablePath: CHROME,
        headless: 'new',
        defaultViewport: { width: SIZE, height: SIZE, deviceScaleFactor: 1 },
        args: [`--window-size=${SIZE},${SIZE}`, '--hide-scrollbars'],
    });
    const page = await browser.newPage();
    await page.goto('file:///' + OUT_HTML.replace(/\\/g, '/'), { waitUntil: 'networkidle0' });
    await new Promise(r => setTimeout(r, 1500));

    const interval = 1000 / FPS;
    const start = Date.now();
    for (let i = 0; i < TOTAL_FRAMES; i++) {
        const wait = (start + i * interval) - Date.now();
        if (wait > 0) await new Promise(r => setTimeout(r, wait));
        await page.screenshot({ path: path.join(FRAMES_DIR, `f${String(i).padStart(4, '0')}.png`) });
    }
    await browser.close();
    console.log(`Captured ${TOTAL_FRAMES} frames into ${FRAMES_DIR}`);
})();
