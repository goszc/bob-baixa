import { DRIVE_SCOPE, activeLyric, choosePlayback, coverPlaceholder, deviceSupport, indexFile, isLyricText, libraryQuery, lyricLines, mergeLibrary, mergePhoneState, stateFile, trackIsLiked } from "./library.js"

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
const waiting = document.querySelector("#waiting")
const clientId = window.BOB_WEB?.clientId || ""
const api = (window.BOB_WEB?.api || "https://www.googleapis.com").replace(/\/$/, "")

const ICONS = {
  play: '<polygon points="9,6 18,12 9,18"/>',
  pause: '<line x1="9" y1="6" x2="9" y2="18"/><line x1="15" y1="6" x2="15" y2="18"/>',
  prev: '<polygon points="16,6 8,12 16,18"/><line x1="7" y1="6" x2="7" y2="18"/>',
  next: '<polygon points="8,6 16,12 8,18"/><line x1="17" y1="6" x2="17" y2="18"/>',
  shuffle: '<polyline points="16 3 21 3 21 8"/><line x1="4" y1="20" x2="21" y2="3"/><polyline points="21 16 21 21 16 21"/><line x1="15" y1="15" x2="21" y2="21"/><line x1="4" y1="4" x2="9" y2="9"/>',
  repeat: '<polyline points="17 1 21 5 17 9"/><path d="M3 11V9a4 4 0 0 1 4-4h14"/><polyline points="7 23 3 19 7 15"/><path d="M21 13v2a4 4 0 0 1-4 4H3"/>',
  heart: '<path d="M19 13.5 12 20l-7-6.5a4 4 0 0 1 6-5l1 1 1-1a4 4 0 0 1 6 5z"/>',
  home: '<path d="M4 11 12 4l8 7"/><path d="M6 10.5V20h12v-9.5"/>',
  library: '<rect x="4" y="4" width="7" height="7" rx="1.5"/><rect x="13" y="4" width="7" height="7" rx="1.5"/><rect x="4" y="13" width="7" height="7" rx="1.5"/><rect x="13" y="13" width="7" height="7" rx="1.5"/>',
  discover: '<circle cx="12" cy="12" r="8"/><polygon points="14.5,9 10,12 14.5,15"/>',
  download: '<path d="M12 4v10"/><path d="m7 10 5 5 5-5"/><path d="M5 19h14"/>',
  list: '<path d="M9 6h11"/><path d="M9 12h11"/><path d="M9 18h11"/><circle cx="4.5" cy="6" r="1"/><circle cx="4.5" cy="12" r="1"/><circle cx="4.5" cy="18" r="1"/>',
}

let tokenClient = null
let access = localStorage.getItem("bob-access") || sessionStorage.getItem("bob-access") || ""
let expiresAt = Number(localStorage.getItem("bob-exp") || sessionStorage.getItem("bob-exp") || 0)
let hint = localStorage.getItem("bob-hint") || ""
let tracks = []
let visible = []
let index = -1
let deferredInstall = null
let pending = null
let promptUsed = false
let blobUrl = ""
let retrying = false
let explained = false
let phoneState = readState()
let liked = new Set(phoneState.likes)
let queue = []
let tab = ""
let lastStream = ""
let swState = "sem worker"
let driveFiles = []
let view = "home"
let segment = "tracks"
let openedGroup = ""
let shuffle = false
let repeat = "off"
let dragAt = -1

paintIcons()
wire()
void boot()

setInterval(() => {
  if (!access) return
  if (expiresAt - Date.now() < 120_000) void requestToken(false).catch(() => {})
  else void pushToken()
}, 60_000)

function icon(name, filled) {
  return `<svg viewBox="0 0 24 24" width="22" height="22" fill="${filled ? "currentColor" : "none"}" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${ICONS[name]}</svg>`
}

function paintIcons() {
  document.querySelector("#prev").innerHTML = icon("prev")
  document.querySelector("#next").innerHTML = icon("next")
  document.querySelector("#shuffle").innerHTML = icon("shuffle")
  document.querySelector("#repeat").innerHTML = icon("repeat")
  document.querySelector("#like").innerHTML = icon("heart")
  for (const mark of document.querySelectorAll("[data-ico]")) mark.innerHTML = icon(mark.dataset.ico)
  setPlayIcon(true)
}

function setPlayIcon(paused) {
  const mark = icon(paused ? "play" : "pause", paused)
  document.querySelector("#toggle").innerHTML = mark
  document.querySelector("#mini-toggle").innerHTML = mark
  document.querySelector("#toggle").setAttribute("aria-label", paused ? "tocar" : "pausar")
}

function wire() {
  if ("serviceWorker" in navigator) {
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
      showWelcome("falta o client id web. o readme explica o segredo BOB_GOOGLE_WEB_CLIENT_ID.")
      return
    }
    showWaiting()
    withTimeout(requestToken(true), 12_000, "o google não respondeu. tenta de novo.")
      .then(() => openLibrary())
      .catch((error) => showWelcome(error instanceof Error ? error.message : "não consegui entrar."))
  }
  document.querySelector("#prev").onclick = () => step(-1)
  document.querySelector("#next").onclick = () => step(1)
  document.querySelector("#toggle").onclick = () => toggle()
  document.querySelector("#shuffle").onclick = () => {
    shuffle = !shuffle
    document.querySelector("#shuffle").setAttribute("aria-pressed", shuffle ? "true" : "false")
  }
  document.querySelector("#repeat").onclick = () => {
    repeat = repeat === "off" ? "all" : repeat === "all" ? "one" : "off"
    document.querySelector("#repeat").setAttribute("aria-pressed", repeat === "off" ? "false" : "true")
    document.querySelector("#repeat").setAttribute("aria-label", repeat === "one" ? "repetir uma" : "repetir")
  }
  search.oninput = () => renderLibrary()
  for (const media of [audio, video]) {
    media.addEventListener("timeupdate", () => onTime(media))
    media.addEventListener("loadedmetadata", () => onTime(media))
    media.addEventListener("ended", () => step(1))
    media.addEventListener("waiting", () => setStatus(""))
    media.addEventListener("playing", () => setStatus(""))
    media.addEventListener("error", () => onMediaError(media))
  }
  document.querySelector("#seek").oninput = (event) => {
    const media = activeMedia()
    if (!media || !Number.isFinite(media.duration)) return
    media.currentTime = (Number(event.target.value) / 1000) * media.duration
    onTime(media)
  }
  document.querySelector("#version")?.addEventListener("click", () => {
    const panel = document.querySelector("#debug")
    panel.hidden = !panel.hidden
    paintDebug()
  })
  document.querySelector("#like")?.addEventListener("click", () => toggleLike(currentTrack()))
  document.querySelector("#mini")?.addEventListener("click", (event) => {
    if (event.target.closest("button")) return
    if (index < 0 && tracks[0]) preview(tracks[0])
    openSheet()
  })
  document.querySelector("#mini-toggle")?.addEventListener("click", () => {
    pulse()
    toggle()
  })
  let phoneFormat = localStorage.getItem("bob-format") === "mp4" ? "mp4" : "mp3"
  const paintFormat = () => {
    document.querySelector("#fmt-mp3")?.setAttribute("aria-pressed", phoneFormat === "mp3" ? "true" : "false")
    document.querySelector("#fmt-mp4")?.setAttribute("aria-pressed", phoneFormat === "mp4" ? "true" : "false")
  }
  paintFormat()
  document.querySelector("#fmt-mp3")?.addEventListener("click", () => {
    phoneFormat = "mp3"
    localStorage.setItem("bob-format", phoneFormat)
    paintFormat()
  })
  document.querySelector("#fmt-mp4")?.addEventListener("click", () => {
    phoneFormat = "mp4"
    localStorage.setItem("bob-format", phoneFormat)
    paintFormat()
  })
  document.querySelector("#link-go")?.addEventListener("click", () => {
    const link = document.querySelector("#linkbox")?.value?.trim()
    const note = document.querySelector("#link-note")
    if (note) note.textContent = link ? `baixando em ${phoneFormat.toUpperCase()}.` : "cole um link primeiro."
  })
  document.querySelector("#discover-one")?.addEventListener("click", () => {
    const note = document.querySelector("#discover-note")
    if (note) note.textContent = "baixando 1 faixa no formato padrão."
    if (tracks.length) play(0)
  })
  document.querySelector("#discover-mix")?.addEventListener("click", () => {
    const note = document.querySelector("#discover-note")
    if (note) note.textContent = "mix de 10 vídeos em 480p."
    if (tracks.length) play(0)
  })
  document.querySelector("#close")?.addEventListener("click", () => closeSheet())
  document.querySelector("#grab")?.addEventListener("pointerdown", startDrag)
  document.querySelector("#stage")?.addEventListener("pointerdown", startDrag)
  window.addEventListener("pointerup", endDrag)
  cover.addEventListener("load", () => tintFrom(cover))
  for (const button of document.querySelectorAll("[data-view]")) {
    button.addEventListener("click", () => {
      view = button.dataset.view
      for (const item of document.querySelectorAll("[data-view]")) item.removeAttribute("aria-current")
      button.setAttribute("aria-current", "page")
      const libraryView = view === "library" || view === "playlists"
      document.querySelector("#home").hidden = view !== "home"
      document.querySelector("#library").hidden = !libraryView
      document.querySelector("#likes").hidden = view !== "likes"
      document.querySelector("#discover").hidden = view !== "discover"
      document.querySelector("#download").hidden = view !== "download"
      if (view === "playlists") segment = "playlists"
      if (view === "library") segment = "tracks"
      for (const item of document.querySelectorAll("[data-seg]")) item.setAttribute("aria-selected", item.dataset.seg === segment ? "true" : "false")
      if (view === "home") renderHome()
      else if (view === "likes") renderLikes()
      else if (libraryView) renderLibrary()
    })
  }
  for (const button of document.querySelectorAll("[data-seg]")) {
    button.addEventListener("click", () => {
      segment = button.dataset.seg
      openedGroup = ""
      for (const item of document.querySelectorAll("[data-seg]")) item.setAttribute("aria-selected", item === button ? "true" : "false")
      renderLibrary()
    })
  }
  for (const button of document.querySelectorAll("[data-tab]")) {
    button.addEventListener("click", () => selectTab(button.dataset.tab))
  }
  const scroller = document.querySelector("main")
  let pullY = 0
  scroller.addEventListener("touchstart", (event) => {
    if (scroller.scrollTop <= 0) pullY = event.touches[0].clientY
  }, { passive: true })
  scroller.addEventListener("touchend", (event) => {
    if (!pullY) return
    const delta = event.changedTouches[0].clientY - pullY
    pullY = 0
    if (delta > 72 && access) {
      document.querySelector("#pull").textContent = "atualizando…"
      void openLibrary()
    }
  }, { passive: true })
}

function selectTab(next) {
  tab = next
  for (const item of document.querySelectorAll("[data-tab]")) item.setAttribute("aria-selected", item.dataset.tab === next ? "true" : "false")
  document.querySelector("#lyric").hidden = next !== "lyrics"
  document.querySelector("#queue").hidden = next !== "queue"
  document.querySelector("#sheet")?.classList.toggle("lyrics", next === "lyrics")
  if (next === "queue") renderQueue()
  if (next === "lyrics" && visible[index]) showLyrics(visible[index], activeMedia()?.currentTime || 0)
}

function startDrag(event) {
  event.currentTarget.dataset.x = String(event.clientX)
  event.currentTarget.dataset.y = String(event.clientY)
}

function endDrag(event) {
  const stage = document.querySelector("#stage")
  const grab = document.querySelector("#grab")
  const source = event.target?.closest ? event.target.closest("#stage, #grab") : null
  const host = source || (stage.dataset.x ? stage : grab.dataset.y ? grab : null)
  if (!host?.dataset.x && !host?.dataset.y) return
  const x = Number(host.dataset.x || 0)
  const y = Number(host.dataset.y || 0)
  delete host.dataset.x
  delete host.dataset.y
  const dx = event.clientX - x
  const dy = event.clientY - y
  if (host.id === "grab" && dy > 64) closeSheet()
  if (host.id === "stage" && Math.abs(dx) > 48 && Math.abs(dx) > Math.abs(dy)) step(dx < 0 ? 1 : -1)
}

function openSheet() {
  document.body.classList.add("sheet")
}

function closeSheet() {
  document.body.classList.remove("sheet")
}

function showWelcome(message) {
  document.documentElement.classList.remove("has-session")
  document.documentElement.classList.remove("is-waiting")
  gate.hidden = false
  player.hidden = true
  waiting.hidden = true
  signin.hidden = false
  closeSheet()
  setErr(message || "")
  setStatus("")
}

function showWaiting() {
  document.documentElement.classList.remove("has-session")
  gate.hidden = false
  player.hidden = true
  waiting.hidden = false
  signin.hidden = true
  gate.setAttribute("aria-busy", "true")
  setStatus("")
}

function showApp() {
  document.documentElement.classList.add("has-session")
  gate.hidden = true
  player.hidden = false
  waiting.hidden = true
  signin.hidden = false
  gate.removeAttribute("aria-busy")
}

function setLoading(on) {
  const bones = document.querySelector("#bones")
  bones.hidden = !on
  document.querySelector("#list").hidden = on
  if (on) {
    view = "library"
    document.querySelector("#home").hidden = true
    document.querySelector("#library").hidden = false
    document.querySelector("#likes").hidden = true
    document.querySelector("#discover").hidden = true
    document.querySelector("#download").hidden = true
    document.querySelector('[data-view="library"]')?.setAttribute("aria-current", "page")
    document.querySelector('[data-view="home"]')?.removeAttribute("aria-current")
  }
}

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
        if (response.email) hint = response.email
        rememberSession()
        setErr("")
        void pushToken()
        const wait = pending
        pending = null
        wait?.resolve(access)
        if (!wait && player.hidden) void openLibrary()
      },
    })
  }
  return tokenClient
}

function rememberSession() {
  localStorage.setItem("bob-access", access)
  localStorage.setItem("bob-exp", String(expiresAt))
  if (hint) localStorage.setItem("bob-hint", hint)
  sessionStorage.setItem("bob-access", access)
  sessionStorage.setItem("bob-exp", String(expiresAt))
  const maxAge = Math.max(60, Math.round((expiresAt - Date.now()) / 1000))
  document.cookie = `bob-access=${encodeURIComponent(access)}; path=/; max-age=${maxAge}; samesite=lax`
}

function requestToken(force) {
  const client = ensureClient()
  if (!client) return Promise.reject(new Error(clientId ? "o google não carregou." : "falta o client id web."))
  promptUsed = force
  return new Promise((resolve, reject) => {
    pending = { resolve, reject }
    const options = { prompt: force ? "consent" : "" }
    if (hint) options.login_hint = hint
    try {
      client.requestAccessToken(options)
    } catch (error) {
      pending = null
      reject(error instanceof Error ? error : new Error("o google não abriu."))
    }
  })
}

function withTimeout(promise, ms, message) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      pending = null
      reject(new Error(message))
    }, ms)
    promise.then(
      (value) => {
        clearTimeout(timer)
        resolve(value)
      },
      (error) => {
        clearTimeout(timer)
        reject(error)
      },
    )
  })
}

async function boot() {
  const valid = Boolean(access && expiresAt > Date.now() + 15_000)
  if (valid) {
    showApp()
    setLoading(true)
  } else if (access || hint) showWaiting()
  else showWelcome("")
  await ensureWorker()
  if (valid) {
    rememberSession()
    await openLibrary()
    return
  }
  if (!(access || hint)) return
  try {
    await withTimeout(
      (async () => {
        await waitForGoogle(2000)
        await requestToken(false)
      })(),
      3000,
      "não deu para entrar sozinho.",
    )
    await openLibrary()
  } catch (error) {
    showWelcome(error instanceof Error ? error.message : "entra com o google.")
  }
}

function waitForGoogle(limit) {
  return new Promise((resolve, reject) => {
    const started = Date.now()
    const tick = () => {
      if (window.google?.accounts?.oauth2) resolve()
      else if (Date.now() - started > limit) reject(new Error("o google não carregou."))
      else setTimeout(tick, 80)
    }
    tick()
  })
}

async function ensureWorker() {
  if (!("serviceWorker" in navigator)) {
    swState = "sem worker"
    return
  }
  try {
    await navigator.serviceWorker.register("./sw.js")
    await navigator.serviceWorker.ready
    if (!navigator.serviceWorker.controller) {
      if (sessionStorage.getItem("bob-sw-reload") !== "1") {
        sessionStorage.setItem("bob-sw-reload", "1")
        location.reload()
        return
      }
      swState = "registrado, sem controle"
      return
    }
    swState = "controlado"
    await pushToken()
  } catch (error) {
    swState = error instanceof Error ? error.message : "falhou"
  }
}

async function openLibrary(retried = false) {
  showApp()
  setLoading(true)
  setErr("")
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
            await withTimeout(requestToken(false), 3000, "a sessão acabou.")
            return openLibrary(true)
          } catch {
            /* fall through to the sign-in gate */
          }
        }
        access = ""
        localStorage.removeItem("bob-access")
        localStorage.removeItem("bob-exp")
        sessionStorage.removeItem("bob-access")
        showWelcome("a sessão acabou. entra de novo.")
        return
      }
      if (!response.ok) throw new Error(String(response.status))
      const body = await response.json()
      found.push(...(body.files || []))
      page = body.nextPageToken || ""
    } while (page && found.length < 2000)
  } catch {
    setLoading(false)
    showWelcome("não consegui ler o drive.")
    return
  }
  let phoneIndex = null
  const indexHit = indexFile(found)
  if (indexHit?.id) {
    try {
      const response = await fetch(`${api}/drive/v3/files/${indexHit.id}?alt=media`, { headers: { authorization: `Bearer ${access}` } })
      if (response.ok) phoneIndex = await response.json()
    } catch {
      /* the file list still plays */
    }
  }
  tracks = mergeLibrary(found, phoneIndex)
  driveFiles = found
  const stateHit = stateFile(found)
  if (stateHit?.id) {
    try {
      const response = await fetch(`${api}/drive/v3/files/${stateHit.id}?alt=media`, { headers: { authorization: `Bearer ${access}` } })
      if (response.ok) phoneState = mergePhoneState(phoneState, await response.json())
    } catch {
      /* local likes still work */
    }
  }
  liked = new Set(phoneState.likes)
  queue = phoneState.queue.map((id) => tracks.find((track) => track.id === id)).filter(Boolean)
  await pushToken()
  setLoading(false)
  document.querySelector("#pull").textContent = ""
  visible = tracks
  const last = tracks.find((track) => track.id === localStorage.getItem("bob-last")) || tracks[0]
  if (last) preview(last)
  else {
    document.querySelector("#mini-title").textContent = "nada por aqui"
    document.querySelector("#mini-artist").textContent = ""
  }
  showHome()
  if (!tracks.length) setErr("nenhuma faixa do bob baixa neste projeto. o cliente web precisa estar no mesmo projeto do cliente desktop.")
}

function showHome() {
  view = "home"
  document.querySelector("#home").hidden = false
  document.querySelector("#library").hidden = true
  document.querySelector("#likes").hidden = true
  document.querySelector("#discover").hidden = true
  document.querySelector("#download").hidden = true
  document.querySelector('[data-view="home"]')?.setAttribute("aria-current", "page")
  document.querySelector('[data-view="library"]')?.removeAttribute("aria-current")
  renderHome()
}

function lovedTracks() {
  return tracks.filter((track) => trackIsLiked([...liked], track))
}

function paintLikeCount() {
  const node = document.querySelector("#like-count")
  if (node) node.textContent = String(lovedTracks().length)
}

function renderLikes() {
  const box = document.querySelector("#likes")
  box.innerHTML = ""
  const loved = lovedTracks()
  if (!loved.length) {
    const empty = document.createElement("p")
    empty.className = "lead"
    empty.textContent = "nenhuma curtida ainda."
    const action = document.createElement("button")
    action.type = "button"
    action.className = "go"
    action.textContent = "Descobrir música"
    action.addEventListener("click", () => document.querySelector('[data-view="discover"]')?.dispatchEvent(new Event("click")))
    box.append(empty, action)
    return
  }
  for (const track of loved) {
    const button = document.createElement("button")
    button.type = "button"
    button.className = "row"
    button.textContent = `${track.artist} — ${track.title}`
    button.addEventListener("click", () => play(tracks.indexOf(track)))
    box.append(button)
  }
}

function renderHome() {
  paintLikeCount()
  const home = document.querySelector("#home")
  home.innerHTML = ""
  if (!tracks.length) {
    const empty = document.createElement("p")
    empty.className = "lead"
    empty.textContent = "nenhuma faixa ainda."
    home.append(empty)
    return
  }
  const last = tracks.find((track) => track.id === localStorage.getItem("bob-last")) || tracks[0]
  const mix = mixTracks(tracks).filter((track) => track.id !== last.id)
  home.append(sectionLabel("continuar ouvindo"))
  home.append(gridOf([last, ...mix.slice(0, 5)]))
  if (mix.length) {
    home.append(sectionLabel("mix do dia"))
    home.append(gridOf(mix.slice(0, 6)))
  }
  const loved = lovedTracks()
  home.append(sectionLabel("curtidas"))
  if (loved.length) home.append(gridOf(loved))
  else home.append(note("curta uma faixa e ela fica aqui."))
  const recentIds = readIds("bob-recent")
  const recent = recentIds.map((id) => tracks.find((track) => track.id === id)).filter((track) => track && track.id !== last.id)
  if (recent.length) {
    home.append(sectionLabel("recentes"))
    home.append(rail(recent))
  }
}

function renderLibrary() {
  const query = search.value.trim().toLocaleLowerCase()
  const list = document.querySelector("#list")
  const groups = document.querySelector("#groups")
  list.innerHTML = ""
  groups.innerHTML = ""
  list.hidden = false
  const pool = query
    ? tracks.filter((track) => `${track.artist} ${track.title} ${track.album}`.toLocaleLowerCase().includes(query))
    : tracks
  if (query || segment === "tracks" || openedGroup) {
    const rows = openedGroup && !query ? openedGroup.tracks : pool
    visible = rows
    if (!rows.length) {
      list.append(plainItem(tracks.length ? "nada com esse nome." : "nenhuma faixa."))
      return
    }
    rows.forEach((track, trackIndex) => {
      const item = document.createElement("li")
      const button = trackButton(track)
      if (track.id === visible[index]?.id || trackIndex === index) button.setAttribute("aria-current", "true")
      button.onclick = () => {
        visible = rows
        play(trackIndex)
      }
      item.append(button)
      list.append(item)
    })
    return
  }
  if (segment === "artists" || segment === "albums") {
    const grouped = groupBy(tracks, segment === "artists" ? "artist" : "album")
    const grid = document.createElement("div")
    grid.className = "grid"
    for (const group of grouped) {
      const button = document.createElement("button")
      button.type = "button"
      button.append(coverImage(group.tracks[0], "grid"))
      const title = document.createElement("strong")
      title.textContent = group.name
      const count = document.createElement("small")
      count.textContent = `${group.tracks.length} faixas`
      button.append(title, count)
      button.onclick = () => {
        openedGroup = group
        renderLibrary()
      }
      grid.append(button)
    }
    groups.append(grid)
    return
  }
  const curtidas = document.createElement("li")
  const loved = lovedTracks()
  const button = document.createElement("button")
  button.type = "button"
  button.textContent = `Curtidas · ${loved.length}`
  button.onclick = () => {
    if (!loved.length) return
    visible = loved
    play(0)
  }
  curtidas.append(button)
  list.append(curtidas)
  for (const playlist of phoneState.playlists) {
    const item = document.createElement("li")
    const row = document.createElement("button")
    row.type = "button"
    row.textContent = `${playlist.name} · ${playlist.paths.length}`
    row.onclick = () => {
      const rows = playlist.paths.map((id) => tracks.find((track) => track.id === id)).filter(Boolean)
      if (!rows.length) return
      visible = rows
      play(0)
    }
    item.append(row)
    list.append(item)
  }
}

function renderQueue() {
  const box = document.querySelector("#queue")
  box.innerHTML = ""
  if (!queue.length) {
    box.textContent = "a fila começa quando você escolhe uma faixa."
    return
  }
  queue.forEach((track, trackIndex) => {
    const button = trackButton(track)
    const handle = document.createElement("span")
    handle.className = "handle"
    handle.textContent = "≡"
    handle.setAttribute("aria-hidden", "true")
    button.append(handle)
    button.draggable = true
    button.onclick = () => {
      visible = queue.slice()
      play(trackIndex)
    }
    button.addEventListener("dragstart", () => {
      dragAt = trackIndex
    })
    button.addEventListener("dragover", (event) => event.preventDefault())
    button.addEventListener("drop", () => moveQueue(dragAt, trackIndex))
    handle.addEventListener("pointerdown", (event) => {
      dragAt = trackIndex
      event.stopPropagation()
    })
    box.append(button)
  })
}

function moveQueue(from, to) {
  if (from < 0 || to < 0 || from === to) return
  const [row] = queue.splice(from, 1)
  queue.splice(to, 0, row)
  phoneState = { ...phoneState, queue: queue.map((track) => track.id), updatedAt: Date.now() }
  localStorage.setItem("bob-phone-state", JSON.stringify(phoneState))
  renderQueue()
  void saveState()
}

function sectionLabel(text) {
  const node = document.createElement("p")
  node.className = "kicker"
  node.textContent = text
  return node
}

function note(text) {
  const node = document.createElement("p")
  node.className = "lead"
  node.textContent = text
  return node
}

function heroCard(track) {
  const button = document.createElement("button")
  button.type = "button"
  button.className = "hero"
  button.setAttribute("aria-label", labelFor(track))
  const art = coverImage(track, "art")
  const title = document.createElement("strong")
  title.textContent = track.title
  const artist = document.createElement("small")
  artist.textContent = track.artist
  button.append(art, title, artist)
  button.onclick = () => {
    visible = tracks
    play(Math.max(0, tracks.indexOf(track)))
  }
  return button
}

function gridOf(rows) {
  const box = document.createElement("div")
  box.className = "home-grid"
  for (const track of rows) {
    const button = document.createElement("button")
    button.type = "button"
    button.setAttribute("aria-label", labelFor(track))
    const title = document.createElement("strong")
    title.textContent = track.title
    const artist = document.createElement("small")
    artist.textContent = track.artist
    button.append(coverImage(track, ""), title, artist)
    button.onclick = () => {
      visible = rows
      play(rows.indexOf(track))
    }
    box.append(button)
  }
  return box
}

function rail(rows) {
  const strip = document.createElement("div")
  strip.className = "rail"
  for (const track of rows) {
    const button = document.createElement("button")
    button.type = "button"
    button.setAttribute("aria-label", labelFor(track))
    const title = document.createElement("strong")
    title.textContent = track.title
    const artist = document.createElement("small")
    artist.textContent = track.artist
    button.append(coverImage(track, ""), title, artist)
    button.onclick = () => {
      visible = rows
      play(rows.indexOf(track))
    }
    strip.append(button)
  }
  return strip
}

function trackButton(track) {
  const button = document.createElement("button")
  button.type = "button"
  button.setAttribute("aria-label", labelFor(track))
  button.append(coverImage(track, "thumb"), names(track))
  return button
}

function names(track) {
  const box = document.createElement("span")
  box.className = "meta"
  const title = document.createElement("strong")
  title.textContent = track.title
  const artist = document.createElement("small")
  artist.textContent = [track.artist, albumOf(track)].filter(Boolean).join(" · ")
  box.append(title, artist)
  return box
}

function coverImage(track, className) {
  const name = className || "thumb"
  const image = document.createElement("img")
  const fallback = coverPlaceholder(track)
  image.alt = ""
  image.className = name
  const remote = coverSrc(track)
  image.src = fallback
  if (remote) {
    const probe = new Image()
    probe.onload = () => {
      image.src = remote
    }
    probe.onerror = () => {
      void directCover(track).then((url) => {
        if (url) image.src = url
      })
    }
    probe.src = remote
  }
  return image
}

function coverSrc(track) {
  if (track?.thumb && /^(https?:|data:image\/)/.test(track.thumb)) return track.thumb
  if (track?.coverId) return streamUrl(track.coverId, 0, "image/jpeg")
  return ""
}

async function directCover(track) {
  if (!track?.coverId || !access) return ""
  try {
    const response = await fetch(`${api}/drive/v3/files/${track.coverId}?alt=media`, { headers: { authorization: `Bearer ${access}` } })
    if (!response.ok) return ""
    const blob = await response.blob()
    if (blob.size < 32) return ""
    return URL.createObjectURL(blob)
  } catch {
    return ""
  }
}

function plainItem(text) {
  const item = document.createElement("li")
  item.textContent = text
  return item
}

function labelFor(track) {
  return track.artist ? `${track.artist} — ${track.title}` : track.title
}

function albumOf(track) {
  const album = String(track.album || "").trim()
  if (!album || album === "0") return ""
  return album
}

function groupBy(rows, key) {
  const map = new Map()
  for (const track of rows) {
    const name = String(track[key] || "").trim() || (key === "artist" ? "sem artista" : "sem álbum")
    const id = name.toLocaleLowerCase()
    if (!map.has(id)) map.set(id, { name, tracks: [] })
    map.get(id).tracks.push(track)
  }
  return [...map.values()]
}

function mixTracks(rows) {
  const day = new Date().toDateString()
  return [...rows].sort((a, b) => hash(`${day}${a.id}`) - hash(`${day}${b.id}`)).slice(0, 8)
}

function hash(text) {
  let value = 0
  for (const ch of text) value = (value * 33 + ch.charCodeAt(0)) >>> 0
  return value
}

function preview(track) {
  const at = tracks.indexOf(track)
  if (at >= 0) {
    visible = tracks
    index = at
  }
  document.querySelector("#title").textContent = track.title || ""
  document.querySelector("#artist").textContent = [track.artist, albumOf(track)].filter(Boolean).join(" · ")
  document.querySelector("#mini-title").textContent = track.title || "bob baixa"
  document.querySelector("#mini-artist").textContent = track.artist || ""
  document.querySelector("#like").setAttribute("aria-pressed", trackIsLiked([...liked], track) ? "true" : "false")
  document.querySelector("#like").innerHTML = icon("heart", trackIsLiked([...liked], track))
  showCover(track)
  publishSession(track)
}

function step(delta) {
  if (!visible.length) return
  if (repeat === "one" && delta > 0 && activeMedia()?.src) {
    activeMedia().currentTime = 0
    void activeMedia().play()
    return
  }
  let next = index < 0 ? 0 : index + delta
  if (shuffle && visible.length > 1) next = Math.floor(Math.random() * visible.length)
  if (next < 0 || next >= visible.length) {
    if (repeat === "all") next = (next + visible.length) % visible.length
    else return
  }
  play(next)
}

function toggle() {
  pulse()
  const media = activeMedia()
  if (!media?.src) {
    if (visible.length) play(index < 0 ? 0 : index)
    return
  }
  if (media.paused) void media.play().catch(() => setErr("toca de novo para começar."))
  else media.pause()
}

function currentTrack() {
  return visible[index] || null
}

function toggleLike(track) {
  if (!track) return
  if (trackIsLiked([...liked], track)) {
    liked.delete(track.id)
    liked.delete(`drive:${track.id}`)
  } else liked.add(track.id)
  phoneState = { ...phoneState, likes: [...liked], updatedAt: Date.now() }
  localStorage.setItem("bob-phone-state", JSON.stringify(phoneState))
  document.querySelector("#like").setAttribute("aria-pressed", trackIsLiked([...liked], track) ? "true" : "false")
  document.querySelector("#like").innerHTML = icon("heart", trackIsLiked([...liked], track))
  paintLikeCount()
  if (view === "home") renderHome()
  void saveState()
}

async function play(trackIndex) {
  const track = visible[trackIndex]
  if (!track) return
  index = trackIndex
  retrying = false
  explained = false
  preview(track)
  openSheet()
  rememberRecent(track.id)
  setErr("")
  const support = deviceSupport((type) => audio.canPlayType(type), navigator.userAgent)
  const choice = choosePlayback(track, tracks, support)
  if (choice.unsupported) {
    stop()
    setErr(choice.reason)
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
  document.querySelector("#mini-title").textContent = track.title || "bob baixa"
  document.querySelector("#mini-artist").textContent = track.artist || ""
  if (!queue.find((item) => item.id === track.id)) queue = [track, ...queue.filter((item) => item.id !== track.id)].slice(0, 100)
  phoneState = { ...phoneState, queue: queue.map((item) => item.id), updatedAt: Date.now() }
  if (navigator.serviceWorker?.controller) {
    await pushToken()
    media.src = src
    media.play().catch((error) => {
      if (explained) return
      setErr(error?.name === "NotAllowedError" ? "toca de novo para começar." : "não consegui tocar esta faixa.")
    })
  } else void blobFallback(media, choice.id)
  publishSession(track)
  void rememberOffline(choice.id, choice.mime, track.size)
  void saveState()
}

function streamUrl(id, size, mime) {
  const url = new URL(`stream/${id}`, document.baseURI)
  if (size) url.searchParams.set("size", String(size))
  if (mime) url.searchParams.set("type", mime)
  if (api) url.searchParams.set("api", api)
  return url.href
}

async function blobFallback(media, id) {
  try {
    const response = await fetch(`${api}/drive/v3/files/${id}?alt=media`, { headers: { authorization: `Bearer ${access}` } })
    if (response.status === 401 || response.status === 403) {
      setErr("a sessão acabou. entra de novo.")
      return
    }
    if (!response.ok) throw new Error(String(response.status))
    const blob = await response.blob()
    if (blobUrl) URL.revokeObjectURL(blobUrl)
    blobUrl = URL.createObjectURL(blob)
    media.src = blobUrl
    await media.play().catch(() => setErr("toca de novo para começar."))
  } catch (error) {
    setErr(error instanceof Error && error.message ? `não consegui baixar a faixa (${error.message}).` : "não consegui baixar a faixa.")
  }
}

function showCover(track) {
  const mini = document.querySelector("#mini-cover")
  const wash = document.querySelector("#wash-img")
  const fallback = coverPlaceholder(track)
  const remote = coverSrc(track)
  const paint = (src) => {
    const art = src || fallback
    mini.src = art
    wash.src = art
    if (video.hidden === false) {
      cover.hidden = true
      return
    }
    cover.hidden = false
    cover.src = art
  }
  paint(remote)
  if (!track?.coverId) return
  const applyBlob = () => {
    fetch(`${api}/drive/v3/files/${track.coverId}?alt=media`, { headers: { authorization: `Bearer ${access}` } })
      .then((response) => (response.ok ? response.blob() : null))
      .then((blob) => {
        if (!blob || blob.size < 32) return
        paint(URL.createObjectURL(blob))
      })
      .catch(() => paint(fallback))
  }
  if (!navigator.serviceWorker?.controller) applyBlob()
  else {
    cover.onerror = () => applyBlob()
    mini.onerror = () => applyBlob()
  }
}

function showLyrics(track, time) {
  const box = document.querySelector("#lyric")
  const lines = lyricLines(track?.lyrics || "")
  const current = activeLyric(lines, time)
  if (box.dataset.track === track?.id && box.dataset.line === String(current) && box.childElementCount === lines.length && lines.length) return
  box.dataset.track = track?.id || ""
  box.dataset.line = String(current)
  box.innerHTML = ""
  if (!lines.length) {
    if (track?.lyricsId && track.lyricsId !== track.coverId && !track.lyrics) void loadLyricFile(track)
    return
  }
  lines.forEach((line, lineIndex) => {
    const button = document.createElement("button")
    button.type = "button"
    const next = lines[lineIndex + 1]?.at
    if (lineIndex === current && line.at != null && next != null) {
      const span = Math.max(0.4, next - line.at)
      const ratio = Math.min(1, Math.max(0, (time - line.at) / span))
      const cut = Math.round(ratio * line.text.length)
      const said = document.createElement("span")
      said.className = "said"
      said.textContent = line.text.slice(0, cut)
      button.append(said, document.createTextNode(line.text.slice(cut)))
    } else button.textContent = line.text
    if (lineIndex === current) button.setAttribute("aria-current", "true")
    if (line.at != null) {
      button.onclick = () => {
        const media = activeMedia()
        if (media) media.currentTime = line.at
      }
    }
    box.append(button)
  })
  if (!matchMedia("(prefers-reduced-motion: reduce)").matches) {
    box.querySelector("[aria-current='true']")?.scrollIntoView({ block: "center", behavior: "smooth" })
  }
}

async function loadLyricFile(track) {
  if (!track.lyricsId || track.lyricsId === track.coverId) return
  try {
    const response = await fetch(`${api}/drive/v3/files/${track.lyricsId}?alt=media`, { headers: { authorization: `Bearer ${access}` } })
    if (!response.ok) return
    const type = (response.headers.get("content-type") || "").toLowerCase()
    if (type.startsWith("image/") || type.startsWith("audio/") || type.startsWith("video/")) return
    const body = await response.text()
    if (!isLyricText(body)) return
    track.lyrics = body
    if (visible[index]?.id === track.id) showLyrics(track, activeMedia()?.currentTime || 0)
  } catch {
    /* a missing lyric stays blank */
  }
}

function onTime(media) {
  if (media !== activeMedia()) return
  const seek = document.querySelector("#seek")
  const duration = Number.isFinite(media.duration) ? media.duration : visible[index]?.duration || 0
  const bits = document.querySelectorAll("#time span")
  if (bits.length === 2) {
    bits[0].textContent = clock(media.currentTime)
    bits[1].textContent = clock(duration)
  }
  if (duration > 0) seek.value = String(Math.round((media.currentTime / duration) * 1000))
  setPlayIcon(media.paused)
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
  if (data.type === "stream") {
    lastStream = `${data.status || 0}`
    paintDebug()
    return
  }
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
  if (!("mediaSession" in navigator) || !track) return
  const artwork = track.coverId ? [{ src: streamUrl(track.coverId, 0, "image/jpeg"), sizes: "512x512", type: "image/jpeg" }] : []
  navigator.mediaSession.metadata = new MediaMetadata({ title: track.title || "bob baixa", artist: track.artist || "", album: albumOf(track), artwork })
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

async function saveState() {
  const hit = stateFile(driveFiles)
  if (!access || !hit?.id) return
  await fetch(`${api}/upload/drive/v3/files/${hit.id}?uploadType=media`, {
    method: "PATCH",
    headers: { authorization: `Bearer ${access}`, "content-type": "application/json" },
    body: JSON.stringify(phoneState),
  }).catch(() => {})
}

async function rememberOffline(id, mime, size) {
  if (!id || !size || size > 8_000_000 || !("caches" in window)) return
  try {
    const cache = await caches.open("bob-ouvir-media")
    const key = new URL(`offline/${id}`, document.baseURI).href
    if (await cache.match(key)) return
    const response = await fetch(streamUrl(id, size, mime))
    if (!response.ok && response.status !== 206) return
    const blob = await response.blob()
    await cache.put(key, new Response(blob, { headers: { "content-type": mime || blob.type || "application/octet-stream" } }))
  } catch {
    /* online playback does not depend on the cache */
  }
}

function tintFrom(image) {
  try {
    const canvas = document.createElement("canvas")
    canvas.width = 8
    canvas.height = 8
    const context = canvas.getContext("2d", { willReadFrequently: true })
    context.drawImage(image, 0, 0, 8, 8)
    const data = context.getImageData(0, 0, 8, 8).data
    let r = 0
    let g = 0
    let b = 0
    const count = data.length / 4
    for (let i = 0; i < data.length; i += 4) {
      r += data[i]
      g += data[i + 1]
      b += data[i + 2]
    }
    document.querySelector("#wash").style.background = `rgb(${Math.round(r / count)}, ${Math.round(g / count)}, ${Math.round(b / count)})`
  } catch {
    /* a cross-origin cover keeps the neutral wash */
  }
}

function paintDebug() {
  const panel = document.querySelector("#debug")
  if (!panel || panel.hidden) return
  const tokenLine = access ? (expiresAt > Date.now() ? "token válido" : "token vencido") : "sem token"
  panel.textContent = `worker: ${swState}\n${tokenLine}\núltimo stream: ${lastStream || "—"}\nerro: ${err.textContent || "—"}`
}

function setErr(message) {
  err.hidden = !message
  err.textContent = message || ""
  paintDebug()
}

function setStatus(message) {
  status.textContent = message || ""
}

function clock(seconds) {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00"
  const whole = Math.floor(seconds)
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, "0")}`
}

function pulse() {
  if (navigator.vibrate) navigator.vibrate(8)
}

function readState() {
  try {
    return mergePhoneState({ updatedAt: 0, likes: [], playlists: [], queue: [] }, JSON.parse(localStorage.getItem("bob-phone-state") || "null"))
  } catch {
    return { updatedAt: 0, likes: [], playlists: [], queue: [] }
  }
}

function readIds(key) {
  try {
    const parsed = JSON.parse(localStorage.getItem(key) || "[]")
    return Array.isArray(parsed) ? parsed.filter((item) => typeof item === "string") : []
  } catch {
    return []
  }
}

function rememberRecent(id) {
  const ids = [id, ...readIds("bob-recent").filter((item) => item !== id)].slice(0, 12)
  localStorage.setItem("bob-recent", JSON.stringify(ids))
  localStorage.setItem("bob-last", id)
}
