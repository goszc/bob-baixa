import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import http from "node:http"
import path from "node:path"
import { fileURLToPath } from "node:url"
import { chromium, webkit, devices } from "playwright"

const root = path.dirname(fileURLToPath(import.meta.url))
const clipPath = path.resolve(root, "../../public/preview/clip.mp4")
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
  "base64",
)
const wav = makeWav()
const clip = await readFile(clipPath)
const ranges = []

const api = http.createServer((req, res) => {
  const url = new URL(req.url || "/", "http://127.0.0.1")
  if (req.method === "OPTIONS") {
    res.writeHead(204, cors())
    res.end()
    return
  }
  const auth = req.headers.authorization || ""
  if (auth !== "Bearer test-token") {
    res.writeHead(401, cors())
    res.end("unauthorized")
    return
  }
  if (url.pathname === "/drive/v3/files") {
    const body = JSON.stringify({
      files: [
        { id: "audio1", name: 'Low Roar - "I\'ll Keep Coming".opus', mimeType: "audio/ogg", size: String(wav.length), appProperties: { bobKind: "audio" } },
        { id: "audio1b", name: 'Low Roar - "I\'ll Keep Coming".mp3', mimeType: "audio/mpeg", size: String(wav.length), appProperties: { bobKind: "audio" } },
        { id: "video1", name: "Disappeared Completely - No Place for Me (Pt.1) - Official Video.mp4", mimeType: "video/mp4", size: String(clip.length), appProperties: { bobKind: "video" } },
        { id: "hania", name: "Gondwana Records — Hania Rani — 'Leaving' (Official Video) [Gondwana Records].mp4", mimeType: "video/mp4", size: "12", appProperties: { bobKind: "video" } },
        { id: "leaf", name: "Sub Pop — The Album Leaf - The Outer Banks [OFFICIAL VIDEO].mp4", mimeType: "video/mp4", size: "12", appProperties: { bobKind: "video" } },
        { id: "caps", name: "DISAPPEARED COMPLETELY — Disappeared Completley - Once In a While (short mood video).mp3", mimeType: "audio/mpeg", size: String(wav.length), appProperties: { bobKind: "audio" } },
        { id: "pic", name: "cover.jpg", mimeType: "image/jpeg", size: String(png.length), appProperties: { bobKind: "lyrics", bobMedia: "leaf" } },
        { id: "cover1", name: "cover.jpg", mimeType: "image/jpeg", size: String(png.length), appProperties: { bobKind: "cover", bobMedia: "audio1" } },
        { id: "index1", name: "index.json", mimeType: "application/json", size: "10", appProperties: { bobKind: "index" } },
        { id: "state1", name: "biblioteca.json", mimeType: "application/json", size: "40", appProperties: { bobKind: "state" } },
        { id: "coverv", name: "video1.jpg", mimeType: "image/svg+xml", size: "80", appProperties: { bobKind: "cover", bobMedia: "video1" } },
        { id: "gone", name: "broken.mp3", mimeType: "audio/mpeg", size: "10", appProperties: { bobKind: "audio" } },
      ],
    })
    res.writeHead(200, { ...cors(), "content-type": "application/json" })
    res.end(body)
    return
  }
  const media = url.pathname.match(/^\/drive\/v3\/files\/([^/]+)$/)
  if (media && url.searchParams.get("alt") === "media") {
    const id = media[1]
    if (id === "state1") {
      res.writeHead(200, { ...cors(), "content-type": "application/json" })
      res.end(JSON.stringify({
        updatedAt: 50,
        likes: ["drive:audio1", "drive:video1", "drive:hania", "drive:leaf"],
        playlists: [{ name: "Noite", paths: ["audio1"] }],
        queue: [],
      }))
      return
    }
    if (id === "coverv") {
      res.writeHead(200, { ...cors(), "content-type": "image/svg+xml" })
      res.end(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8"><rect width="8" height="8" fill="#3d6ea8"/></svg>`)
      return
    }
    if (id === "index1") {
      res.writeHead(200, { ...cors(), "content-type": "application/json" })
      res.end(JSON.stringify({
        version: 1,
        tracks: [
          { id: "audio1", name: "keep.wav", title: "I'll Keep Coming", artist: "Low Roar", album: "0", mime: "audio/wav", size: wav.length, kind: "audio", coverId: "cover1", lyrics: "[00:00.00]keep coming" },
          { id: "video1", name: "place.mp4", title: "No Place for Me", artist: "Disappeared Completely", mime: "video/mp4", size: clip.length, kind: "video" },
          { id: "hania", title: "Gondwana Records — Hania Rani — 'Leaving' (Official Video) [Gondwana Records]", artist: "Gondwana Records", kind: "video" },
          { id: "leaf", title: "The Album Leaf - The Outer Banks [OFFICIAL VIDEO]", artist: "Sub Pop", kind: "video", lyricsId: "pic", lyrics: "ÿØÿà JFIF" },
          { id: "caps", title: "DISAPPEARED COMPLETELY — Disappeared Completley - Once In a While (short mood video)", artist: "DISAPPEARED COMPLETELY", kind: "audio" },
        ],
      }))
      return
    }
    if (id === "gone") {
      res.writeHead(403, cors())
      res.end("forbidden")
      return
    }
    if (id === "pic") {
      res.writeHead(200, { ...cors(), "content-type": "image/jpeg" })
      res.end(png)
      return
    }
    const bytes = id === "cover1" ? png : id === "video1" ? clip : wav
    const type = id === "cover1" ? "image/png" : id === "video1" ? "video/mp4" : "audio/wav"
    sendRange(req, res, bytes, type)
    return
  }
  res.writeHead(404, cors())
  res.end("missing")
})

const site = http.createServer(async (req, res) => {
  const url = new URL(req.url || "/", "http://127.0.0.1")
  if (url.pathname.endsWith("/config.js")) {
    res.writeHead(200, { "content-type": "text/javascript", "cache-control": "no-store" })
    res.end(`window.BOB_WEB = { clientId: "test.apps.googleusercontent.com", api: "http://127.0.0.1:${api.address().port}" }\n`)
    return
  }
  const rel = decodeURIComponent(url.pathname.replace(/^\/bob-baixa\/ouvir\/?/, "") || "index.html")
  if (rel.startsWith("stream/")) {
    res.writeHead(404, { "content-type": "text/plain" })
    res.end("the service worker should have intercepted this")
    return
  }
  const file = path.join(root, rel)
  if (!file.startsWith(root)) {
    res.writeHead(403)
    res.end()
    return
  }
  try {
    const body = await readFile(file)
    res.writeHead(200, { "content-type": typeFor(file), "cache-control": "no-store" })
    res.end(body)
  } catch {
    res.writeHead(404)
    res.end("missing")
  }
})

await new Promise((resolve) => api.listen(0, "127.0.0.1", resolve))
await new Promise((resolve) => site.listen(0, "127.0.0.1", resolve))
const origin = `http://localhost:${site.address().port}/bob-baixa/ouvir/`
const phone = devices["Pixel 7"]

async function exercise(browser, label) {
  const context = await browser.newContext({ ...phone })
  const page = await context.newPage()
  await page.addInitScript(() => {
    localStorage.setItem("bob-access", "test-token")
    localStorage.setItem("bob-exp", String(Date.now() + 3_600_000))
  })
  await page.goto(origin, { waitUntil: "domcontentloaded" })
  await page.getByRole("button", { name: "Low Roar — I'll Keep Coming" }).first().waitFor({ timeout: 20_000 })
  await page.waitForFunction(() => navigator.serviceWorker?.controller)
  if (process.env.SHOT && label === "chromium") await page.screenshot({ path: "/opt/cursor/artifacts/phone-home-pixel.png" })
  await page.locator("#version").click()
  await page.locator("#debug").waitFor()
  const debug = await page.locator("#debug").innerText()
  assert.match(debug, /controlado/)
  assert.match(debug, /token válido/)
  await page.locator("#version").click()
  await page.getByRole("button", { name: "Biblioteca" }).click()
  await page.getByRole("button", { name: "Hania Rani — Leaving" }).waitFor()
  await page.getByRole("button", { name: "The Album Leaf — The Outer Banks" }).waitFor()
  await page.getByRole("button", { name: "Disappeared Completely — Once In a While" }).waitFor()
  assert.equal(await page.getByRole("button", { name: "Low Roar — I'll Keep Coming" }).count(), 1)
  if (process.env.SHOT && label === "chromium") await page.screenshot({ path: "/opt/cursor/artifacts/phone-library-pixel.png" })
  if (process.env.SHOT && label === "chromium") {
    await page.getByRole("button", { name: "Curtidas" }).click()
    await page.screenshot({ path: "/opt/cursor/artifacts/phone-likes-pixel.png" })
    await page.getByRole("button", { name: "Descobrir", exact: true }).click()
    await page.screenshot({ path: "/opt/cursor/artifacts/phone-discover-pixel.png" })
    await page.getByRole("button", { name: "Biblioteca" }).click()
  }
  await page.getByRole("button", { name: "Low Roar — I'll Keep Coming" }).click()
  await page.waitForFunction(() => {
    const audio = document.querySelector("#audio")
    const image = document.querySelector("#cover")
    return audio && Number.isFinite(audio.duration) && audio.duration > 0 && image && image.naturalWidth > 0
  })
  assert.match(await page.locator("#audio").evaluate((element) => element.currentSrc), /\/bob-baixa\/ouvir\/stream\/audio1/)
  await page.evaluate(() => document.querySelector("#audio").pause())
  if (process.env.SHOT && label === "chromium") {
    await page.waitForTimeout(350)
    await page.screenshot({ path: "/opt/cursor/artifacts/phone-now-pixel.png" })
  }
  await page.getByRole("tab", { name: "Letra" }).click()
  await page.getByRole("button", { name: "keep coming", exact: true }).waitFor()
  if (process.env.SHOT && label === "chromium") await page.screenshot({ path: "/opt/cursor/artifacts/phone-lyrics-pixel.png" })
  await page.getByRole("tab", { name: "Fila" }).click()
  if (process.env.SHOT && label === "chromium") await page.screenshot({ path: "/opt/cursor/artifacts/phone-queue-pixel.png" })
  await page.getByRole("button", { name: "fechar", exact: true }).click()
  await page.getByRole("button", { name: "The Album Leaf — The Outer Banks" }).click()
  await page.getByRole("tab", { name: "Letra" }).click()
  assert.doesNotMatch(await page.locator("#lyric").innerText(), /JFIF|ÿØ/)
  await page.getByRole("button", { name: "fechar", exact: true }).click()
  await page.getByRole("button", { name: "Disappeared Completely — No Place for Me" }).click()
  const canVideo = await page.evaluate(() => document.createElement("video").canPlayType('video/mp4; codecs="avc1.42E01E"'))
  if (canVideo) {
    await page.waitForFunction(() => {
      const video = document.querySelector("#video")
      return video && !video.hidden && video.videoWidth > 0
    })
    if (process.env.SHOT && label === "chromium") await page.screenshot({ path: "/opt/cursor/artifacts/phone-video-pixel.png" })
  }
  await page.getByRole("button", { name: "fechar", exact: true }).click()
  if (!canVideo) {
    await page.locator("#version").click()
    await page.locator("#debug").waitFor()
    assert.match(await page.locator("#debug").innerText(), /206|200/)
    await page.locator("#version").click()
  }
  await page.locator("#search").fill("broken")
  await page.getByRole("button", { name: "broken", exact: true }).click()
  await page.locator("#err").getByText(/recusou a faixa \(403\)|não consegui tocar/).waitFor()
  const kept = await page.evaluate(() => localStorage.getItem("bob-access"))
  assert.equal(kept, "test-token")
  await context.close()
  console.log(`${label} played a ranged drive stream on /bob-baixa/ouvir/`)
}

const chromiumBrowser = await chromium.launch({ headless: true })
await exercise(chromiumBrowser, "chromium")
const again = await chromiumBrowser.newContext({ ...phone })
const quiet = await again.newPage()
await quiet.addInitScript(() => {
  localStorage.setItem("bob-access", "test-token")
  localStorage.setItem("bob-exp", String(Date.now() + 3_600_000))
})
await quiet.route("https://accounts.google.com/**", (route) => route.abort())
await quiet.goto(origin, { waitUntil: "domcontentloaded" })
await quiet.getByRole("button", { name: "Low Roar — I'll Keep Coming" }).first().waitFor({ timeout: 20_000 })
assert.equal(await quiet.locator("#gate").isHidden(), true)
await again.close()
await chromiumBrowser.close()

let webkitBrowser = null
try {
  webkitBrowser = await webkit.launch({ headless: true })
  await exercise(webkitBrowser, "webkit")
  await webkitBrowser.close()
} catch (error) {
  console.error(`webkit failed: ${error instanceof Error ? error.stack || error.message : error}`)
  if (webkitBrowser) await webkitBrowser.close()
  throw error
}

assert.ok(ranges.some((range) => range.startsWith("bytes=")), `expected a range request, got ${ranges.join(",")}`)

const loggedOut = await chromium.launch({ headless: true })
const out = await loggedOut.newContext({ ...phone })
await out.route("https://accounts.google.com/**", (route) => route.abort())
const signedOut = await out.newPage()
await signedOut.goto(origin, { waitUntil: "domcontentloaded" })
await signedOut.locator("#signin").waitFor({ timeout: 1000 })
assert.equal(await signedOut.locator("#player").isHidden(), true)
assert.equal(await signedOut.locator("#signin").isVisible(), true)
const stale = await loggedOut.newContext({ ...phone })
await stale.route("https://accounts.google.com/**", (route) => route.abort())
const expired = await stale.newPage()
await expired.addInitScript(() => {
  localStorage.setItem("bob-access", "old-token")
  localStorage.setItem("bob-exp", "1")
  localStorage.setItem("bob-hint", "ouvinte@example.com")
})
await expired.goto(origin, { waitUntil: "domcontentloaded" })
await expired.locator("#signin").waitFor({ timeout: 4000 })
assert.match(await expired.locator("#err").innerText(), /google|entrar|sozinho|não/i)
if (process.env.SHOT) {
  await signedOut.screenshot({ path: "/opt/cursor/artifacts/phone-onboarding-pixel.png", animations: "disabled" })
  const iphone = {
    viewport: { width: 393, height: 852 },
    deviceScaleFactor: 2,
    isMobile: true,
    hasTouch: true,
    userAgent: devices["iPhone 15"]?.userAgent || devices["iPhone 13"].userAgent,
  }
  const shot = { animations: "disabled", timeout: 15_000 }
  const iphoneOut = await loggedOut.newContext(iphone)
  await iphoneOut.route("https://accounts.google.com/**", (route) => route.abort())
  const iphonePage = await iphoneOut.newPage()
  await iphonePage.goto(origin, { waitUntil: "domcontentloaded" })
  await iphonePage.waitForTimeout(1200)
  await iphonePage.locator("#signin").waitFor({ timeout: 3000 })
  await iphonePage.screenshot({ path: "/opt/cursor/artifacts/phone-onboarding-iphone.png", animations: "disabled", timeout: 15_000 })
  const iphoneIn = await loggedOut.newContext(iphone)
  await iphoneIn.route("https://accounts.google.com/**", (route) => route.abort())
  const tour = await iphoneIn.newPage()
  await tour.addInitScript(() => {
    localStorage.setItem("bob-access", "test-token")
    localStorage.setItem("bob-exp", String(Date.now() + 3_600_000))
  })
  await tour.goto(origin, { waitUntil: "domcontentloaded" })
  await tour.waitForTimeout(1200)
  await tour.getByRole("button", { name: "Low Roar — I'll Keep Coming" }).first().waitFor({ timeout: 20_000 })
  await tour.screenshot({ path: "/opt/cursor/artifacts/phone-home-iphone.png", ...shot })
  await tour.getByRole("button", { name: "Biblioteca" }).click()
  await tour.screenshot({ path: "/opt/cursor/artifacts/phone-library-iphone.png", ...shot })
  await tour.getByRole("button", { name: "Curtidas" }).click()
  await tour.screenshot({ path: "/opt/cursor/artifacts/phone-likes-iphone.png", ...shot })
  await tour.getByRole("button", { name: "Descobrir", exact: true }).click()
  await tour.screenshot({ path: "/opt/cursor/artifacts/phone-discover-iphone.png", ...shot })
  await tour.getByRole("button", { name: "Biblioteca" }).click()
  await tour.getByRole("button", { name: "Low Roar — I'll Keep Coming" }).click()
  await tour.waitForFunction(() => document.querySelector("#cover")?.naturalWidth > 0)
  await tour.evaluate(() => document.querySelector("#audio")?.pause())
  await tour.screenshot({ path: "/opt/cursor/artifacts/phone-now-iphone.png", ...shot })
  await tour.getByRole("tab", { name: "Letra" }).click()
  await tour.getByRole("button", { name: "keep coming", exact: true }).waitFor()
  await tour.screenshot({ path: "/opt/cursor/artifacts/phone-lyrics-iphone.png", ...shot })
  await tour.getByRole("tab", { name: "Fila" }).click()
  await tour.screenshot({ path: "/opt/cursor/artifacts/phone-queue-iphone.png", ...shot })
  await iphoneOut.close()
  await iphoneIn.close()
}
await out.close()
await stale.close()
await loggedOut.close()

site.close()
api.close()
console.log("phone played a ranged drive stream, cover, lyrics and video")

function sendRange(req, res, bytes, type) {
  const total = bytes.length
  const header = req.headers.range || ""
  if (header) ranges.push(header)
  const match = /^bytes=(\d+)-(\d*)$/i.exec(header)
  if (!match) {
    res.writeHead(200, { ...cors(), "content-type": type, "content-length": total, "accept-ranges": "bytes" })
    res.end(bytes)
    return
  }
  if (header === "bytes=0-1") {
    res.writeHead(200, { ...cors(), "content-type": type, "content-length": total, "accept-ranges": "bytes" })
    res.end(bytes)
    return
  }
  const start = Number(match[1])
  const end = match[2] ? Math.min(total - 1, Number(match[2])) : Math.min(total - 1, start + 1024 * 256 - 1)
  const slice = bytes.subarray(start, end + 1)
  res.writeHead(206, {
    ...cors(),
    "content-type": type,
    "content-length": slice.length,
    "content-range": `bytes ${start}-${end}/${total}`,
    "accept-ranges": "bytes",
  })
  res.end(slice)
}

function cors() {
  return {
    "access-control-allow-origin": "*",
    "access-control-allow-headers": "authorization, range, content-type",
    "access-control-allow-methods": "GET, OPTIONS",
    "access-control-expose-headers": "content-range, content-length, content-type, accept-ranges",
  }
}

function typeFor(file) {
  if (file.endsWith(".js") || file.endsWith(".mjs")) return "text/javascript"
  if (file.endsWith(".html")) return "text/html"
  if (file.endsWith(".webmanifest")) return "application/manifest+json"
  if (file.endsWith(".png")) return "image/png"
  return "application/octet-stream"
}

function makeWav() {
  const rate = 8000
  const samples = rate
  const data = Buffer.alloc(samples * 2)
  for (let index = 0; index < samples; index += 1) data.writeInt16LE(Math.round(Math.sin(index / 8) * 3000), index * 2)
  const header = Buffer.alloc(44)
  header.write("RIFF", 0)
  header.writeUInt32LE(36 + data.length, 4)
  header.write("WAVE", 8)
  header.write("fmt ", 12)
  header.writeUInt32LE(16, 16)
  header.writeUInt16LE(1, 20)
  header.writeUInt16LE(1, 22)
  header.writeUInt32LE(rate, 24)
  header.writeUInt32LE(rate * 2, 28)
  header.writeUInt16LE(2, 32)
  header.writeUInt16LE(16, 34)
  header.write("data", 36)
  header.writeUInt32LE(data.length, 40)
  return Buffer.concat([header, data])
}
