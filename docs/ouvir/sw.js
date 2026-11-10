/* Classic worker on purpose. A module worker fails to register on older iOS, and then
   /bob-baixa/ouvir/stream/<id> is a static 404, so the phone plays nothing. */

const CACHE = "bob-ouvir-5"
const MEDIA = "bob-ouvir-media"
const SHELL = ["./","index.html","./app.js","./library.js","./stream.js","./config.js","./manifest.webmanifest","./icons/icon-192.png","./icons/icon-512.png","./icons/icon-maskable-512.png"]

let token = ""
let expiresAt = 0
let api = "https://www.googleapis.com"
let lastStatus = 0

self.addEventListener("install", (event) => {
  event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(SHELL).catch(() => {})))
  self.skipWaiting()
})

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((key) => key !== CACHE).map((key) => caches.delete(key))))
      .then(() => self.clients.claim()),
  )
})

self.addEventListener("message", (event) => {
  const data = event.data || {}
  if (data.type === "debug") {
    event.ports?.[0]?.postMessage({ token: Boolean(token), expiresAt, lastStatus })
    return
  }
  if (data.type !== "token") return
  event.waitUntil(remember(data))
  event.ports?.[0]?.postMessage({ ok: Boolean(data.token) })
})

self.addEventListener("fetch", (event) => {
  const url = new URL(event.request.url)
  if (url.origin === location.origin && fileIdFrom(url.pathname)) {
    event.respondWith(streamFile(event.request, url))
    return
  }
  if (url.origin !== location.origin || event.request.method !== "GET") return
  if (url.pathname.endsWith("/config.js")) {
    event.respondWith(
      fetch(event.request)
        .then((response) => {
          const copy = response.clone()
          caches.open(CACHE).then((cache) => cache.put(event.request, copy))
          return response
        })
        .catch(() => caches.match(event.request)),
    )
    return
  }
  event.respondWith(caches.match(event.request).then((hit) => hit || fetch(event.request)))
})

async function remember(data) {
  token = typeof data.token === "string" ? data.token : ""
  expiresAt = Number(data.expiresAt) || 0
  if (typeof data.api === "string" && data.api.startsWith("http")) api = data.api.replace(/\/$/, "")
  const cache = await caches.open(CACHE)
  await cache.put("./token", new Response(JSON.stringify({ token, expiresAt, api })))
}

async function loadToken() {
  if (token && (!expiresAt || Date.now() <= expiresAt)) return
  const cache = await caches.open(CACHE)
  const saved = await cache.match("./token")
  if (!saved) return
  const data = await saved.json()
  token = typeof data.token === "string" ? data.token : ""
  expiresAt = Number(data.expiresAt) || 0
  if (typeof data.api === "string" && data.api.startsWith("http")) api = data.api.replace(/\/$/, "")
}

function tokenFrom(request) {
  const match = /(?:^|;\s*)bob-access=([^;]+)/.exec(request.headers.get("cookie") || "")
  if (!match) return ""
  try {
    return decodeURIComponent(match[1])
  } catch {
    return ""
  }
}

async function streamFile(request, url) {
  await loadToken()
  const hintedApi = url.searchParams.get("api")
  if (hintedApi && hintedApi.startsWith("http")) api = hintedApi.replace(/\/$/, "")
  if (!token) token = tokenFrom(request)
  const id = fileIdFrom(url.pathname)
  if (!token || (expiresAt && Date.now() > expiresAt)) {
    lastStatus = 401
    await tell({ type: "unauthorized", id, status: 401 })
    return new Response("sem sessão", { status: 401 })
  }
  const headers = new Headers()
  headers.set("Authorization", `Bearer ${token}`)
  const size = Number(url.searchParams.get("size") || 0)
  let range = request.headers.get("range")
  if (!range && size > 0) range = `bytes=0-${size - 1}`
  if (range) headers.set("Range", range)
  let upstream
  try {
    upstream = await fetch(`${api}/drive/v3/files/${id}?alt=media`, { headers })
  } catch {
    const cached = await cachedMedia(id, request)
    if (cached) return cached
    lastStatus = 0
    await tell({ type: "stream-error", id, status: 0 })
    return new Response("sem internet", { status: 502 })
  }
  lastStatus = upstream.status
  await tell({ type: "stream", id, status: upstream.status })
  if (upstream.status === 401) {
    await tell({ type: "unauthorized", id, status: 401 })
    return new Response("sessão", { status: 401 })
  }
  if (!upstream.ok && upstream.status !== 206) {
    await tell({ type: "stream-error", id, status: upstream.status })
    return new Response(`drive ${upstream.status}`, { status: upstream.status })
  }
  const hinted = url.searchParams.get("type")
  const init = mediaResponseInit(upstream.status, upstream.headers, range, size)
  if (hinted && !upstream.headers.get("content-type")) init.headers.set("Content-Type", hinted)
  const wanted = Number(init.headers.get("Content-Length") || 0)
  if (range && wanted > 0 && wanted <= 2_000_000) {
    const bytes = new Uint8Array(await upstream.arrayBuffer())
    let body = bytes
    if (upstream.status === 200 && size > 0) {
      const parsed = parseByteRange(range, size)
      if (parsed) body = bytes.slice(parsed.start, Math.min(bytes.length, parsed.end + 1))
    } else {
      body = bytes.slice()
    }
    init.headers.set("Content-Length", String(body.byteLength))
    return new Response(body, init)
  }
  return new Response(upstream.body, init)
}

async function cachedMedia(id, request) {
  const cache = await caches.open(MEDIA)
  const hit = await cache.match(new URL(`offline/${id}`, self.registration.scope).href)
  if (!hit) return null
  const bytes = new Uint8Array(await hit.arrayBuffer())
  const type = hit.headers.get("content-type") || "application/octet-stream"
  const range = request.headers.get("range")
  if (!range) {
    return new Response(bytes, {
      status: 200,
      headers: { "content-type": type, "content-length": String(bytes.length), "accept-ranges": "bytes" },
    })
  }
  const parsed = parseByteRange(range, bytes.length)
  const slice = parsed ? bytes.slice(parsed.start, parsed.end + 1) : bytes
  return new Response(slice, {
    status: parsed ? 206 : 200,
    headers: {
      "content-type": type,
      "content-length": String(slice.length),
      "accept-ranges": "bytes",
      ...(parsed ? { "content-range": `bytes ${parsed.start}-${parsed.end}/${bytes.length}` } : {}),
    },
  })
}

function fileIdFrom(pathname) {
  const parts = String(pathname || "").split("/").filter(Boolean)
  const at = parts.lastIndexOf("stream")
  if (at < 0 || at + 1 >= parts.length) return ""
  const id = parts[at + 1]
  return /^[A-Za-z0-9_-]+$/.test(id) ? id : ""
}

function parseByteRange(header, size) {
  const match = /^bytes=(\d*)-(\d*)$/i.exec(String(header || "").trim())
  if (!match || size <= 0) return null
  if (!match[1] && !match[2]) return null
  if (!match[1]) {
    const length = Math.min(size, Number(match[2]))
    return { start: size - length, end: size - 1 }
  }
  const start = Number(match[1])
  if (!Number.isFinite(start) || start >= size) return null
  const end = match[2] ? Math.min(size - 1, Number(match[2])) : size - 1
  if (!Number.isFinite(end) || end < start) return null
  return { start, end }
}

function mediaResponseInit(upstreamStatus, upstreamHeaders, requestRange, size) {
  const headers = new Headers()
  const rawType = upstreamHeaders.get("content-type") || "application/octet-stream"
  headers.set("Content-Type", rawType.split(";")[0])
  headers.set("Accept-Ranges", "bytes")
  headers.set("Cache-Control", "private, no-store")
  const exposedRange = upstreamHeaders.get("content-range")
  const exposedLength = upstreamHeaders.get("content-length")
  let status = upstreamStatus
  if (upstreamStatus === 206 || exposedRange) {
    status = 206
    if (exposedRange) headers.set("Content-Range", exposedRange)
    else {
      const parsed = parseByteRange(requestRange, size)
      if (parsed) headers.set("Content-Range", `bytes ${parsed.start}-${parsed.end}/${size}`)
    }
  }
  if (status === 200 && requestRange && size > 0) {
    const parsed = parseByteRange(requestRange, size)
    if (parsed) {
      status = 206
      headers.set("Content-Range", `bytes ${parsed.start}-${parsed.end}/${size}`)
    }
  }
  const match = /^bytes (\d+)-(\d+)\/(\d+|\*)$/i.exec(headers.get("Content-Range") || "")
  if (match) {
    status = 206
    headers.set("Content-Length", String(Number(match[2]) - Number(match[1]) + 1))
  } else if (exposedLength) headers.set("Content-Length", exposedLength)
  else if (status === 200 && size > 0) headers.set("Content-Length", String(size))
  return { status, headers }
}

async function tell(message) {
  const pages = await self.clients.matchAll({ type: "window", includeUncontrolled: true })
  for (const page of pages) page.postMessage(message)
}
