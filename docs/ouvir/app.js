import { DRIVE_SCOPE, activeLyric, choosePlayback, deviceSupport, indexFile, libraryQuery, lyricLines, mergeLibrary } from "./library.js"

const gate = document.querySelector("#gate")
const player = document.querySelector("#player")
const err = document.querySelector("#err")
const status = document.querySelector("#status")
const audio = document.querySelector("#audio")
const video = document.querySelector("#video")
const cover = document.querySelector("#cover")
const signin = document.querySelector("#signin")
const install = document.querySelector("#install")
const search = document.querySelector("#search")
const clientId = window.BOB_WEB?.clientId || ""
const api = (window.BOB_WEB?.api || "https://www.googleapis.com").replace(/\/$/, "")

let tokenClient = null
let access = sessionStorage.getItem("bob-access") || ""
let expiresAt = Number(sessionStorage.getItem("bob-exp") || 0)
let tracks = []
let visible = []
let index = -1
let deferredInstall = null
let pending = null
let promptUsed = false
let blobUrl = ""
let retrying = false
let explained = false

if ("serviceWorker" in navigator) {
  navigator.serviceWorker.register("./sw.js", { type: "module" }).catch(() => {})
  navigator.serviceWorker.addEventListener("controllerchange", () => void pushToken())
  navigator.serviceWorker.addEventListener("message", onWorker)
}

window.addEventListener("beforeinstallprompt", (event) => {
  event.preventDefault()
  deferredInstall = event
  install.hidden = false
})

install.onclick = async () => {
  if (!deferredInstall) return
  deferredInstall.prompt()
  await deferredInstall.userChoice
  deferredInstall = null
  install.hidden = true
}

signin.onclick = () => {
  if (!clientId || clientId.startsWith("COLOQUE_")) {
    setErr("falta o client id web. o readme explica o segredo BOB_GOOGLE_WEB_CLIENT_ID.")
    return
  }
  void requestToken(false)
}

document.querySelector("#prev").onclick = () => step(-1)
document.querySelector("#next").onclick = () => step(1)
document.querySelector("#toggle").onclick = () => toggle()
search.oninput = () => renderList()
audio.addEventListener("timeupdate", () => onTime(audio))
video.addEventListener("timeupdate", () => onTime(video))
audio.addEventListener("loadedmetadata", () => onTime(audio))
video.addEventListener("loadedmetadata", () => onTime(video))
audio.addEventListener("ended", () => step(1))
video.addEventListener("ended", () => step(1))
audio.addEventListener("waiting", () => setStatus("carregando…"))
video.addEventListener("waiting", () => setStatus("carregando…"))
audio.addEventListener("playing", () => setStatus(""))
video.addEventListener("playing", () => setStatus(""))
audio.addEventListener("error", () => onMediaError(audio))
video.addEventListener("error", () => onMediaError(video))
document.querySelector("#seek").oninput = (event) => {
  const media = activeMedia()
  if (!media || !Number.isFinite(media.duration)) return
  media.currentTime = (Number(event.target.value) / 1000) * media.duration
}

if (access && expiresAt > Date.now()) {
  void openLibrary()
} else if (access) {
  void requestToken(false)
}

setInterval(() => {
  if (!access) return
  if (expiresAt - Date.now() < 120_000) void requestToken(false)
  else void pushToken()
}, 60_000)

function ensureClient() {
  const oauth = window.google?.accounts?.oauth2
  if (!oauth || !clientId || clientId.startsWith("COLOQUE_")) return null
  if (!tokenClient) {
    tokenClient = oauth.initTokenClient({
      client_id: clientId,
      scope: DRIVE_SCOPE,
      callback: (response) => {
        if (response.error || !response.access_token) {
          if (!promptUsed) {
            promptUsed = true
            tokenClient.requestAccessToken({ prompt: "consent" })
            return
          }
          const message = "o google não autorizou."
          pending?.reject(new Error(message))
          pending = null
          if (!access) setErr(message)
          return
        }
        promptUsed = false
        access = response.access_token
        expiresAt = Date.now() + (Number(response.expires_in) || 3600) * 1000 - 60_000
        sessionStorage.setItem("bob-access", access)
        sessionStorage.setItem("bob-exp", String(expiresAt))
        setErr("")
        void pushToken()
        const wait = pending
        pending = null
        wait?.resolve(access)
        if (!wait) {
          if (player.hidden) void openLibrary()
        }
      },
    })
  }
  return tokenClient
}

function requestToken(force) {
  const client = ensureClient()
  if (!client) return Promise.reject(new Error("entra de novo."))
  promptUsed = force
  return new Promise((resolve, reject) => {
    pending = { resolve, reject }
    client.requestAccessToken({ prompt: force ? "consent" : "" })
  })
}

async function openLibrary(retried = false) {
  setStatus("carregando a biblioteca…")
  setErr("")
  gate.hidden = true
  player.hidden = false
  const found = []
  let page = ""
  try {
    do {
      const url = new URL(`${api}/drive/v3/files`)
      url.searchParams.set("q", libraryQuery())
      url.searchParams.set("fields", "nextPageToken,files(id,name,mimeType,size,appProperties)")
      url.searchParams.set("pageSize", "100")
      if (page) url.searchParams.set("pageToken", page)
      const response = await fetch(url, { headers: { authorization: `Bearer ${access}` } })
      if (response.status === 401 || response.status === 403) {
        if (!retried) {
          try {
            await requestToken(false)
            return openLibrary(true)
          } catch {
            /* fall through to the sign-in gate */
          }
        }
        access = ""
        sessionStorage.removeItem("bob-access")
        gate.hidden = false
        player.hidden = true
        setErr("a sessão acabou. entra de novo.")
        setStatus("")
        return
      }
      if (!response.ok) throw new Error(String(response.status))
      const body = await response.json()
      found.push(...(body.files || []))
      page = body.nextPageToken || ""
    } while (page && found.length < 500)
  } catch {
    gate.hidden = false
    player.hidden = true
    setErr("não consegui ler o drive.")
    setStatus("")
    return
  }
  let phoneIndex = null
  const index = indexFile(found)
  if (index?.id) {
    try {
      const response = await fetch(`${api}/drive/v3/files/${index.id}?alt=media`, { headers: { authorization: `Bearer ${access}` } })
      if (response.ok) phoneIndex = await response.json()
    } catch {
      /* the file list still plays */
    }
  }
  tracks = mergeLibrary(found, phoneIndex)
  await pushToken()
  renderList()
  setStatus(tracks.length ? "" : "")
  if (!tracks.length) setErr("nenhuma faixa do bob baixa neste projeto. o cliente web precisa estar no mesmo projeto do cliente desktop.")
}

function renderList() {
  const query = search.value.trim().toLocaleLowerCase()
  visible = tracks.filter((track) => `${track.artist} ${track.title} ${track.album}`.toLocaleLowerCase().includes(query))
  const list = document.querySelector("#list")
  list.innerHTML = ""
  if (!visible.length) {
    const item = document.createElement("li")
    item.textContent = tracks.length ? "nada com esse nome." : "nenhuma faixa."
    list.append(item)
    return
  }
  visible.forEach((track, trackIndex) => {
    const item = document.createElement("li")
    const button = document.createElement("button")
    button.type = "button"
    button.textContent = track.artist ? `${track.artist} — ${track.title}` : track.title
    if (trackIndex === index) button.setAttribute("aria-current", "true")
    button.onclick = () => play(trackIndex)
    item.append(button)
    list.append(item)
  })
}

function step(delta) {
  if (!visible.length) return
  const next = index < 0 ? 0 : (index + delta + visible.length) % visible.length
  play(next)
}

function toggle() {
  const media = activeMedia()
  if (!media?.src) {
    if (visible.length) play(index < 0 ? 0 : index)
    return
  }
  if (media.paused) void media.play().catch(() => setErr("toca de novo para começar."))
  else media.pause()
}

function play(trackIndex) {
  const track = visible[trackIndex]
  if (!track) return
  index = trackIndex
  retrying = false
  explained = false
  document.querySelector("#title").textContent = track.title || "—"
  document.querySelector("#artist").textContent = [track.artist, track.album].filter(Boolean).join(" · ")
  setErr("")
  const support = deviceSupport((type) => audio.canPlayType(type), navigator.userAgent)
  const choice = choosePlayback(track, tracks, support)
  if (choice.unsupported) {
    stop()
    setErr(choice.reason)
    renderList()
    return
  }
  const media = choice.element === "video" ? video : audio
  const other = media === video ? audio : video
  other.pause()
  other.removeAttribute("src")
  video.hidden = media !== video
  cover.hidden = media === video
  showCover(track)
  showLyrics(track, 0)
  const src = streamUrl(choice.id, track.size, choice.mime)
  setStatus("carregando…")
  if (navigator.serviceWorker?.controller) {
    media.src = src
    media.play().catch((error) => {
      if (explained) return
      setErr(error?.name === "NotAllowedError" ? "toca de novo para começar." : "não consegui tocar esta faixa.")
    })
  } else {
    void blobFallback(media, choice.id)
  }
  publishSession(track)
  renderList()
}

function streamUrl(id, size, mime) {
  const url = new URL(`stream/${id}`, document.baseURI)
  if (size) url.searchParams.set("size", String(size))
  if (mime) url.searchParams.set("type", mime)
  return url.href
}

async function blobFallback(media, id) {
  setStatus("baixando a faixa…")
  try {
    const response = await fetch(`${api}/drive/v3/files/${id}?alt=media`, { headers: { authorization: `Bearer ${access}` } })
    if (response.status === 401 || response.status === 403) {
      setErr("a sessão acabou. entra de novo.")
      setStatus("")
      return
    }
    if (!response.ok) throw new Error(String(response.status))
    const blob = await response.blob()
    if (blobUrl) URL.revokeObjectURL(blobUrl)
    blobUrl = URL.createObjectURL(blob)
    media.src = blobUrl
    await media.play().catch(() => setErr("toca de novo para começar."))
    setStatus("")
  } catch (error) {
    setStatus("")
    setErr(error instanceof Error && error.message ? `não consegui baixar a faixa (${error.message}).` : "não consegui baixar a faixa.")
  }
}

function showCover(track) {
  if (!track.coverId) {
    cover.removeAttribute("src")
    cover.hidden = true
    return
  }
  cover.hidden = video.hidden === false
  cover.src = navigator.serviceWorker?.controller
    ? streamUrl(track.coverId, 0, "image/jpeg")
    : ""
  if (!navigator.serviceWorker?.controller) {
    fetch(`${api}/drive/v3/files/${track.coverId}?alt=media`, { headers: { authorization: `Bearer ${access}` } })
      .then((response) => (response.ok ? response.blob() : null))
      .then((blob) => {
        if (!blob) return
        cover.src = URL.createObjectURL(blob)
        cover.hidden = false
      })
      .catch(() => {})
  }
}

function showLyrics(track, time) {
  const box = document.querySelector("#lyric")
  const lines = lyricLines(track.lyrics || "")
  const current = activeLyric(lines, time)
  if (box.dataset.track === track.id && box.dataset.line === String(current) && box.childElementCount === lines.length) return
  box.dataset.track = track.id
  box.dataset.line = String(current)
  box.innerHTML = ""
  if (!lines.length) {
    if (track.lyricsId && !track.lyrics) void loadLyricFile(track)
    else box.textContent = ""
    return
  }
  lines.forEach((line, lineIndex) => {
    const button = document.createElement("button")
    button.type = "button"
    button.textContent = line.text
    if (lineIndex === current) button.setAttribute("aria-current", "true")
    if (line.at != null) button.onclick = () => {
      const media = activeMedia()
      if (media) media.currentTime = line.at
    }
    box.append(button)
  })
  box.querySelector("[aria-current='true']")?.scrollIntoView({ block: "nearest" })
}

async function loadLyricFile(track) {
  try {
    const response = await fetch(`${api}/drive/v3/files/${track.lyricsId}?alt=media`, { headers: { authorization: `Bearer ${access}` } })
    if (!response.ok) return
    track.lyrics = await response.text()
    if (visible[index]?.id === track.id) showLyrics(track, activeMedia()?.currentTime || 0)
  } catch {
    /* a missing lyric stays blank, the player does not */
  }
}

function onTime(media) {
  if (media !== activeMedia()) return
  const seek = document.querySelector("#seek")
  const duration = Number.isFinite(media.duration) ? media.duration : visible[index]?.duration || 0
  document.querySelector("#time").textContent = `${clock(media.currentTime)} / ${clock(duration)}`
  if (duration > 0) seek.value = String(Math.round((media.currentTime / duration) * 1000))
  document.querySelector("#toggle").textContent = media.paused ? "tocar" : "pausar"
  if (visible[index]) showLyrics(visible[index], media.currentTime)
  if ("mediaSession" in navigator) navigator.mediaSession.playbackState = media.paused ? "paused" : "playing"
}

function onMediaError(media) {
  if (explained) return
  if (media !== activeMedia() || !media.getAttribute("src")) return
  if (media.error?.code === 1) return
  if (!retrying) {
    retrying = true
    void requestToken(false)
      .then(() => play(index))
      .catch(() => setErr("não consegui tocar. entra de novo se a sessão acabou."))
    return
  }
  const code = media.error?.code
  setErr(code === 4 ? "este celular não abre esse formato." : "não consegui tocar esta faixa.")
  setStatus("")
}

function onWorker(event) {
  const data = event.data || {}
  if (data.type === "unauthorized") {
    void requestToken(false).catch(() => setErr("a sessão acabou. entra de novo."))
    return
  }
  if (data.type === "stream-error") {
    const statusCode = Number(data.status) || 0
    explained = true
    setErr(statusCode === 0 ? "sem internet." : `o drive recusou a faixa (${statusCode}).`)
    setStatus("")
  }
}

function publishSession(track) {
  if (!("mediaSession" in navigator)) return
  const artwork = track.coverId ? [{ src: streamUrl(track.coverId, 0, "image/jpeg"), sizes: "512x512", type: "image/jpeg" }] : []
  navigator.mediaSession.metadata = new MediaMetadata({ title: track.title || "bob baixa", artist: track.artist || "", album: track.album || "", artwork })
  const pairs = [
    ["play", () => activeMedia()?.play()],
    ["pause", () => activeMedia()?.pause()],
    ["previoustrack", () => step(-1)],
    ["nexttrack", () => step(1)],
    ["seekto", (details) => {
      const media = activeMedia()
      if (media && details.seekTime != null) media.currentTime = details.seekTime
    }],
  ]
  for (const [action, run] of pairs) {
    try {
      navigator.mediaSession.setActionHandler(action, run)
    } catch {
      /* this browser skips the action */
    }
  }
}

function activeMedia() {
  return video.hidden ? audio : video
}

function stop() {
  audio.pause()
  video.pause()
  video.hidden = true
  cover.hidden = false
}

async function pushToken() {
  const worker = navigator.serviceWorker?.controller
  if (!worker || !access) return false
  return new Promise((resolve) => {
    const channel = new MessageChannel()
    const timer = setTimeout(() => resolve(false), 500)
    channel.port1.onmessage = () => {
      clearTimeout(timer)
      resolve(true)
    }
    worker.postMessage({ type: "token", token: access, expiresAt, api }, [channel.port2])
  })
}

function setErr(message) {
  err.textContent = message
}

function setStatus(message) {
  status.textContent = message
}

function clock(seconds) {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00"
  const whole = Math.floor(seconds)
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, "0")}`
}
