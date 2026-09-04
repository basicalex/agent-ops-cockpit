// render-svg.mjs <svgDir> <outDir> [scale=1]   — screenshot every SVG page with Chromium.
import { chromium } from 'playwright';
import fs from 'node:fs';
import path from 'node:path';

setTimeout(() => { console.error('render-svg: hard timeout'); process.exit(2); }, 600_000);

const [svgDir, outDir, scaleArg] = process.argv.slice(2);
if (!svgDir || !outDir) { console.error('usage: render-svg.mjs <svgDir> <outDir> [scale]'); process.exit(1); }
const scale = Number(scaleArg || 1);
fs.mkdirSync(outDir, { recursive: true });

const files = fs.readdirSync(svgDir).filter(f => f.endsWith('.svg')).sort();
if (!files.length) { console.error('render-svg: no .svg files in ' + svgDir); process.exit(1); }

// Read the canvas from the first page's viewBox so ppt43 / a4 decks render at their own size.
const vb = (fs.readFileSync(path.join(svgDir, files[0]), 'utf8').match(/viewBox="([^"]+)"/) || [])[1];
const [, , w, h] = (vb || '0 0 1280 720').split(/\s+/).map(Number);

const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width: w, height: h }, deviceScaleFactor: scale });
  for (const [i, f] of files.entries()) {
    await page.goto('file://' + path.join(svgDir, f));
    await page.waitForTimeout(300);
    await page.screenshot({ path: path.join(outDir, `slide-${String(i + 1).padStart(2, '0')}.png`) });
  }
  console.log(`rendered ${files.length} pages at ${scale}x into ${outDir}`);
} finally {
  await browser.close();
}
process.exit(0); // bun keeps the event loop alive after browser.close(); exit explicitly
