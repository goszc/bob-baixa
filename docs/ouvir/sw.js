const SHELL = ["./index.html", "./app.js", "./config.js", "./manifest.webmanifest"]

self.addEventListener("install", (event) => {
  event.waitUntil(caches.open("bob-ouvir").then((cache) => cache.addAll(SHELL)))
  self.skipWaiting()
})

self.addEventListener("fetch", (event) => {
  const url = new URL(event.request.url)
  if (url.origin !== location.origin) return
  event.respondWith(caches.match(event.request).then((hit) => hit || fetch(event.request)))
})
