import { DRIVE_SCOPE, libraryFromFiles, libraryQuery } from "./library.js"

const gate = document.querySelector("#gate")
const player = document.querySelector("#player")
const err = document.querySelector("#err")
const audio = document.querySelector("#audio")
const signin = document.querySelector("#signin")
const install = document.querySelector("#install")
const clientId = window.BOB_WEB?.clientId || ""

let tokenClient = null
let access = sessionStorage.getItem("bob-access") || ""
let expiresAt = Number(sessionStorage.getItem("bob-exp") || 0)
let tracks = []
let index = -1
let deferredInstall = null

if ("serviceWorker" in navigator) {
  navigator.serviceWorker.register("./sw.js").catch(() => {})
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
    err.textContent = "falta o client id web. o readme explica o segredo BOB_GOOGLE_WEB_CLIENT_ID."
    return
  }
  const client = ensureClient()
  if (!client) {
    err.textContent = "o login do google ainda não carregou. tenta de novo."
    return
  }
  client.requestAccessToken({ prompt: access ? "" : "consent" })
}

document.querySelector("#prev").onclick = () => step(-1)
document.querySelector("#next").onclick = () => step(1)

if (access && expiresAt > Date.now()) {
  openLibrary(access)
}

function ensureClient() {
  const oauth = window.google?.accounts?.oauth2
  if (!oauth) return null
  if (!tokenClient) {
    tokenClient = oauth.initTokenClient({
      client_id: clientId,
      scope: DRIVE_SCOPE,
      callback: (response) => {
        if (response.error || !response.access_token) {
          err.textContent = "o google não autorizou."
          return
        }
        access = response.access_token
        expiresAt = Date.now() + (Number(response.expires_in) || 3600) * 1000 - 60_000
        sessionStorage.setItem("bob-access", access)
        sessionStorage.setItem("bob-exp", String(expiresAt))
        err.textContent = ""
        openLibrary(access)
      },
    })
  }
  return tokenClient
}

async function openLibrary(token) {
  gate.hidden = true
  player.hidden = false
  const found = []
  let page = ""
  try {
    do {
      const url = new URL("https://www.googleapis.com/drive/v3/files")
      url.searchParams.set("q", libraryQuery())
      url.searchParams.set("fields", "nextPageToken,files(id,name,mimeType,appProperties)")
      url.searchParams.set("pageSize", "100")
      if (page) url.searchParams.set("pageToken", page)
      const response = await fetch(url, { headers: { authorization: `Bearer ${token}` } })
      if (response.status === 401) {
        sessionStorage.removeItem("bob-access")
        access = ""
        gate.hidden = false
        player.hidden = true
        err.textContent = "a sessão acabou. entra de novo."
        return
      }
      if (!response.ok) throw new Error(String(response.status))
      const body = await response.json()
      found.push(...(body.files || []))
      page = body.nextPageToken || ""
    } while (page && found.length < 400)
  } catch {
    gate.hidden = false
    player.hidden = true
    err.textContent = "não consegui ler o drive."
    return
  }
  tracks = libraryFromFiles(found)
  const list = document.querySelector("#list")
  list.innerHTML = ""
  if (!tracks.length) {
    const item = document.createElement("li")
    item.textContent = "nenhuma faixa do bob baixa neste projeto. o cliente web precisa estar no mesmo projeto do cliente desktop."
    list.append(item)
    return
  }
  tracks.forEach((track, trackIndex) => {
    const item = document.createElement("li")
    const button = document.createElement("button")
    button.type = "button"
    button.textContent = track.artist ? `${track.artist} — ${track.title}` : track.title
    button.onclick = () => play(trackIndex)
    item.append(button)
    list.append(item)
  })
}

function step(delta) {
  if (!tracks.length) return
  play((index + delta + tracks.length) % tracks.length)
}

function play(trackIndex) {
  const track = tracks[trackIndex]
  if (!track || !access) return
  index = trackIndex
  document.querySelector("#title").textContent = track.title || "—"
  document.querySelector("#artist").textContent = track.artist
  audio.src = `https://www.googleapis.com/drive/v3/files/${track.id}?alt=media&access_token=${encodeURIComponent(access)}`
  audio.play().catch(() => {})
  const art = document.querySelector("#art")
  const lyric = document.querySelector("#lyric")
  art.style.backgroundImage = track.coverId
    ? `url(https://www.googleapis.com/drive/v3/files/${track.coverId}?alt=media&access_token=${encodeURIComponent(access)})`
    : ""
  lyric.textContent = ""
  if (!track.lyricsId) return
  fetch(`https://www.googleapis.com/drive/v3/files/${track.lyricsId}?alt=media`, {
    headers: { authorization: `Bearer ${access}` },
  })
    .then((response) => (response.ok ? response.text() : ""))
    .then((text) => {
      lyric.textContent = text.replace(/\[[^\]]+\]/g, "").trim()
    })
    .catch(() => {})
}
