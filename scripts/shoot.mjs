import { mkdir } from "node:fs/promises"
import { chromium } from "playwright"

const out = "/opt/cursor/artifacts/screenshots"
await mkdir(out, { recursive: true })

const browser = await chromium.launch({
  args: ["--font-render-hinting=none", "--disable-dev-shm-usage"],
})
const page = await browser.newPage({
  viewport: { width: 1360, height: 900 },
  deviceScaleFactor: 2,
})
page.on("pageerror", (error) => console.log("PAGE", error))
page.on("console", (message) => {
  if (message.type() === "error") console.log("CONSOLE", message.text())
})

async function settle(theme, nav) {
  await page.emulateMedia({ colorScheme: theme })
  await page.goto("http://127.0.0.1:1420/?preview=1", { waitUntil: "domcontentloaded" })
  await page.waitForSelector("h1")
  await page.evaluate(() => document.fonts.ready)
  await page.waitForTimeout(300)
  if (nav) {
    await page.locator("aside nav").getByRole("button", { name: new RegExp(`^${nav}`) }).click()
    await page.waitForSelector("h1")
    await page.waitForTimeout(320)
  }
}

const shots = [
  ["home-light", "light"],
  ["home-dark", "dark"],
  ["queue-light", "light", "fila"],
  ["queue-dark", "dark", "fila"],
  ["settings-light", "light", "configurações"],
  ["settings-dark", "dark", "configurações"],
  ["search-light", "light", "buscar"],
  ["search-dark", "dark", "buscar"],
]

for (const [name, theme, nav] of shots) {
  await settle(theme, nav)
  const full = String(name).startsWith("settings")
  await page.screenshot({ path: `${out}/${name}.png`, fullPage: full })
  console.log("wrote", name)
}

await browser.close()
