import { mkdir } from "node:fs/promises"
import { chromium } from "playwright"

const out = "/cursor/stores/self/artifacts/screenshots/themes"
await mkdir(out, { recursive: true })

const browser = await chromium.launch({ args: ["--font-render-hinting=none", "--disable-dev-shm-usage"] })
const page = await browser.newPage({ viewport: { width: 1360, height: 900 }, deviceScaleFactor: 2 })
page.on("pageerror", (error) => console.log("PAGE", error))

const themes = [
  ["claro", "light"],
  ["escuro", "dark"],
  ["vidro-claro", "glass-light"],
  ["vidro-escuro", "glass-dark"],
]
const pages = [
  ["home", "baixar"],
  ["queue", "fila"],
  ["settings", "configurações"],
]

async function openPreview() {
  await page.emulateMedia({ colorScheme: "light" })
  await page.goto("http://127.0.0.1:1420/?preview=1", { waitUntil: "domcontentloaded" })
  await page.waitForSelector("h1")
  await page.evaluate(() => document.fonts.ready)
}

for (const [fileName, theme] of themes) {
  await openPreview()
  await page.locator("aside nav").getByRole("button", { name: /^configurações/ }).click()
  await page.waitForSelector(`[data-theme="${theme}"]`)
  await page.locator(`[data-theme="${theme}"]`).click()
  await page.waitForTimeout(250)
  for (const [pageName, nav] of pages) {
    await page.locator("aside nav").getByRole("button", { name: new RegExp(`^${nav}`) }).click()
    await page.waitForSelector("h1")
    await page.waitForTimeout(280)
    const path = `${out}/${pageName}-${fileName}.png`
    await page.screenshot({ path })
    console.log("wrote", path)
  }
}

await browser.close()
