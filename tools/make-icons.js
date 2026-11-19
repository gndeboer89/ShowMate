// Rendert assets/logo.svg naar PNG's (en daarna ico via ImageMagick, zie tools/make-icons.sh).
// Gebruik: node tools/make-icons.js <uitvoermap>
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

(async () => {
  const out = process.argv[2] || 'assets/icons';
  fs.mkdirSync(out, { recursive: true });
  const svg = fs.readFileSync(path.join(__dirname, '..', 'assets', 'logo.svg'), 'utf8');
  const browser = await chromium.launch();
  for (const size of [16, 20, 24, 32, 40, 48, 64, 128, 256, 512]) {
    const page = await browser.newPage({ viewport: { width: size, height: size }, deviceScaleFactor: 1 });
    await page.setContent(`<html><body style="margin:0;background:transparent">${svg.replace(/width="256" height="256"/, `width="${size}" height="${size}"`)}</body></html>`);
    await page.screenshot({ path: path.join(out, `icon-${size}.png`), omitBackground: true });
    await page.close();
  }
  await browser.close();
})();
