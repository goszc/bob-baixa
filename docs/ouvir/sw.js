import { fileIdFrom, mediaResponseInit } from "./stream.js"

const CACHE = "bob-ouvir-3"
const SHELL = [
  "./",
  "./index.html",
  "./app.js",
  "./library.js",
  "./stream.js",
  "./config.js",
  "./manifest.webmanifest",
  "./icons/icon-192.png",
  "./icons/icon-512.png",
  "./icons/icon-maskable-512.png",
]

let token = ""
let expiresAt = 0
let api = "https://www.googleapis.com"

self.addEventListener("install", (event) => {
  event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(SHELL)))
  self.skipWaiting()
})

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches.keys().then((keys) => Promise.all(keys.filter((key) => key !== CACHE).map((key) => caches.delete(key)))),
  )
  self.clients.claim()
})

self.addEventListener("message", (event) => {
  const data = event.data || {}
  if (data.type !== "token") return
  token = typeof data.token === "string" ? data.token : ""
  expiresAt = Number(data.expiresAt) || 0
  if (typeof data.api === "string" && data.api.startsWith("http")) api = data.api.replace(/\/$/, "")
  event.ports?.[0]?.postMessage({ ok: Boolean(token) })
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

async function streamFile(request, url) {
  const id = fileIdFrom(url.pathname)
  if (!token || (expiresAt && Date.now() > expiresAt)) {
    await tell({ type: "unauthorized", id })
    return new Response("sem sessão", { status: 401 })
  }
  const headers = new Headers()
  headers.set("Authorization", `Bearer ${token}`)
  const range = request.headers.get("range")
  if (range) headers.set("Range", range)
  let upstream
  try {
    upstream = await fetch(`${api}/drive/v3/files/${id}?alt=media`, { headers })
  } catch {
    await tell({ type: "stream-error", id, status: 0 })
    return new Response("sem internet", { status: 502 })
  }
  if (upstream.status === 401) {
    await tell({ type: "unauthorized", id, status: upstream.status })
    return new Response("sessão", { status: 401 })
  }
  if (!upstream.ok && upstream.status !== 206) {
    await tell({ type: "stream-error", id, status: upstream.status })
    return new Response(`drive ${upstream.status}`, { status: upstream.status })
  }
  const size = Number(url.searchParams.get("size") || 0)
  const hinted = url.searchParams.get("type")
  const init = mediaResponseInit(upstream.status, upstream.headers, range, size)
  if (hinted && !upstream.headers.get("content-type")) init.headers.set("Content-Type", hinted)
  return new Response(upstream.body, init)
}

async function tell(message) {
  const pages = await self.clients.matchAll({ type: "window", includeUncontrolled: true })
  for (const page of pages) page.postMessage(message)
}
