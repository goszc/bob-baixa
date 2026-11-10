const gate = document.querySelector("#gate")
const player = document.querySelector("#player")
const err = document.querySelector("#err")
const audio = document.querySelector("#audio")
const clientId = window.BOB_WEB?.clientId || ""

if ("serviceWorker" in navigator) {
  navigator.serviceWorker.register("./sw.js").catch(() => {})
}

document.querySelector("#signin").onclick = () => {
  if (!clientId || clientId.startsWith("COLOQUE_")) {
    err.textContent = "falta o client id em config.js. o readme diz qual redirect cadastrar."
    return
  }
  const redirect = `${location.origin}${location.pathname}`
  const url = new URL("https://accounts.google.com/o/oauth2/v2/auth")
  url.searchParams.set("client_id", clientId)
  url.searchParams.set("redirect_uri", redirect)
  url.searchParams.set("response_type", "token")
  url.searchParams.set("scope", "https://www.googleapis.com/auth/drive.file")
  url.searchParams.set("include_granted_scopes", "true")
  location.href = url.toString()
}

const hash = new URLSearchParams(location.hash.replace(/^#/, ""))
const token = hash.get("access_token") || sessionStorage.getItem("bob-access")
if (hash.get("access_token")) {
  sessionStorage.setItem("bob-access", hash.get("access_token"))
  history.replaceState(null, "", location.pathname)
}

if (token) openLibrary(token)

async function openLibrary(access) {
  gate.hidden = true
  player.hidden = false
  const query = "trashed = false"
  const response = await fetch(`https://www.googleapis.com/drive/v3/files?q=${encodeURIComponent(query)}&fields=files(id,name,mimeType)&pageSize=80`, {
    headers: { authorization: `Bearer ${access}` },
  })
  if (!response.ok) {
    err.textContent = "não consegui ler o drive."
    gate.hidden = false
    return
  }
  const body = await response.json()
  const files = body.files || []
  const media = files.filter((file) => String(file.mimeType || "").startsWith("audio/") || String(file.mimeType || "").startsWith("video/") || /\.(mp3|m4a|opus|flac|wav|mp4|webm)$/i.test(file.name))
  const list = document.querySelector("#list")
  list.innerHTML = ""
  for (const file of media) {
    const item = document.createElement("li")
    const button = document.createElement("button")
    button.type = "button"
    button.textContent = file.name
    button.onclick = () => play(access, file, files)
    item.append(button)
    list.append(item)
  }
  if (media[0]) play(access, media[0], files)
}

function play(access, file, files) {
  document.querySelector("#title").textContent = file.name.replace(/\.[a-z0-9]+$/i, "")
  document.querySelector("#artist").textContent = ""
  audio.src = `https://www.googleapis.com/drive/v3/files/${file.id}?alt=media&access_token=${encodeURIComponent(access)}`
  const stem = file.name.replace(/\.[a-z0-9]+$/i, "").toLowerCase()
  const lyric = files.find((item) => item.name.toLowerCase().startsWith(stem) && item.name.toLowerCase().endsWith(".lrc"))
  const cover = files.find((item) => item.name.toLowerCase().startsWith(stem) && /^image\//.test(item.mimeType || ""))
  document.querySelector("#lyric").textContent = ""
  document.querySelector("#art").style.backgroundImage = ""
  if (lyric) {
    fetch(`https://www.googleapis.com/drive/v3/files/${lyric.id}?alt=media`, { headers: { authorization: `Bearer ${access}` } })
      .then((response) => response.text())
      .then((text) => {
        document.querySelector("#lyric").textContent = text.replace(/\[[^\]]+\]/g, "").trim()
      })
      .catch(() => {})
  }
  if (cover) {
    document.querySelector("#art").style.backgroundImage = `url(https://www.googleapis.com/drive/v3/files/${cover.id}?alt=media&access_token=${encodeURIComponent(access)})`
  }
}
