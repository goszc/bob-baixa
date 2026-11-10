import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import http from "node:http"
import path from "node:path"
import { fileURLToPath } from "node:url"
import { chromium } from "playwright"

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
        { id: "video1", name: "Disappeared Completely - No Place for Me (Pt.1) - Official Video.mp4", mimeType: "video/mp4", size: String(clip.length), appProperties: { bobKind: "video" } },
        { id: "cover1", name: "cover.jpg", mimeType: "image/jpeg", size: String(png.length), appProperties: { bobKind: "cover", bobMedia: "audio1" } },
        { id: "index1", name: "index.json", mimeType: "application/json", size: "10", appProperties: { bobKind: "index" } },
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
    if (id === "index1") {
      res.writeHead(200, { ...cors(), "content-type": "application/json" })
      res.end(JSON.stringify({
        version: 1,
        tracks: [
          { id: "audio1", name: "keep.wav", title: "I'll Keep Coming", artist: "Low Roar", album: "0", mime: "audio/wav", size: wav.length, kind: "audio", coverId: "cover1", lyrics: "[00:00.00]keep coming" },
          { id: "video1", name: "place.mp4", title: "No Place for Me", artist: "Disappeared Completely", mime: "video/mp4", size: clip.length, kind: "video" },
        ],
      }))
      return
    }
    if (id === "gone") {
      res.writeHead(403, cors())
      res.end("forbidden")
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
  const rel = decodeURIComponent(url.pathname.replace(/^\/ouvir\/?/, "") || "index.html")
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
const origin = `http://127.0.0.1:${site.address().port}/ouvir/`

const browser = await chromium.launch({ headless: true })
const page = await browser.newPage({ viewport: { width: 420, height: 900 } })
await page.addInitScript(() => {
  sessionStorage.setItem("bob-access", "test-token")
  sessionStorage.setItem("bob-exp", String(Date.now() + 3_600_000))
})
await page.goto(origin, { waitUntil: "networkidle" })
await page.getByRole("button", { name: "Low Roar — I'll Keep Coming" }).waitFor()
await page.waitForFunction(() => navigator.serviceWorker?.controller)
await page.getByRole("button", { name: "Low Roar — I'll Keep Coming" }).click()
await page.waitForFunction(() => {
  const audio = document.querySelector("#audio")
  return audio && Number.isFinite(audio.duration) && audio.duration > 0
})
const cover = await page.locator("#cover").evaluate((image) => image.naturalWidth)
assert.ok(cover > 0, "cover did not load")
await page.getByRole("button", { name: "keep coming", exact: true }).waitFor()
assert.ok(ranges.some((range) => range.startsWith("bytes=")), `expected a range request, got ${ranges.join(",")}`)
const src = await page.locator("#audio").evaluate((element) => element.currentSrc)
assert.match(src, /\/stream\/audio1/)
await page.getByRole("button", { name: "Disappeared Completely — No Place for Me" }).click()
await page.waitForFunction(() => {
  const video = document.querySelector("#video")
  return video && !video.hidden && video.videoWidth > 0
})
await page.locator("#search").fill("broken")
await page.getByRole("button", { name: "broken", exact: true }).click()
await page.getByText(/recusou a faixa \(403\)|não consegui tocar/).waitFor()
await browser.close()
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
