/** Files the desktop app uploaded, seen with the same project's drive.file scope. */

export const DRIVE_SCOPE = "https://www.googleapis.com/auth/drive.file"

const KINDS = ["audio", "video", "lyrics", "cover", "index", "state"]

export function libraryQuery() {
  const kinds = KINDS.map((kind) => `appProperties has { key='bobKind' and value='${kind}' }`)
  return `trashed = false and (${kinds.join(" or ")})`
}

export function libraryFromFiles(files) {
  const list = Array.isArray(files) ? files : []
  const props = (file) => file?.appProperties || {}
  return list
    .filter((file) => props(file).bobKind === "audio" || props(file).bobKind === "video")
    .map((file) => {
      const meta = props(file)
      const related = (kind) => list.find((item) => props(item).bobKind === kind && props(item).bobMedia === file.id)
      return normalizeTrack({
        id: file.id,
        name: file.name || "",
        title: meta.bobTitle || strip(file.name || ""),
        artist: meta.bobArtist || "",
        album: meta.bobAlbum || "",
        video: meta.bobKind === "video",
        lyricsId: lyricIdFor(file, list),
        coverId: related("cover")?.id || "",
        mime: file.mimeType || "",
        size: Number(file.size || 0) || 0,
        duration: 0,
        lyrics: "",
      })
    })
}

function strip(name) {
  return name.replace(/\.[a-z0-9]+$/i, "")
}

export function indexFile(files) {
  const list = Array.isArray(files) ? files : []
  return (
    list.find((file) => file?.appProperties?.bobKind === "index" || file?.name === "index.json") || null
  )
}

export function mergeLibrary(files, index) {
  const fromFiles = libraryFromFiles(files)
  const rows = Array.isArray(index?.tracks) ? index.tracks : []
  const seen = new Set(fromFiles.map((track) => track.id))
  const extra = rows.filter((row) => row?.id && !seen.has(row.id)).map(rowToTrack)
  const images = new Set(
    (Array.isArray(files) ? files : [])
      .filter((file) => String(file?.mimeType || "").startsWith("image/") || file?.appProperties?.bobKind === "cover")
      .map((file) => file.id),
  )
  return dedupeTracks(
    applyPhoneIndex([...fromFiles, ...extra], index).map((track) => {
      const next = normalizeTrack(track)
      if (images.has(next.lyricsId)) next.lyricsId = ""
      if (!next.coverId) next.coverId = coverIdFor(next, files)
      return next
    }),
  )
}

export function applyPhoneIndex(tracks, index) {
  const rows = Array.isArray(index?.tracks) ? index.tracks : []
  const byId = new Map(rows.filter((row) => row?.id).map((row) => [row.id, row]))
  return tracks.map((track) => {
    const row = byId.get(track.id)
    if (!row) return track
    const coverId = text(row.coverId) || track.coverId
    const lyricsId = safeLyricsId(text(row.lyricsId) || track.lyricsId, coverId)
    const thumb = remoteImage(row.thumbnail) || remoteImage(row.cover) || remoteImage(track.thumb)
    const lyrics = lyricBody(text(row.lyrics)) || lyricBody(track.lyrics)
    return {
      ...track,
      title: text(row.title) || track.title,
      artist: text(row.artist) || track.artist,
      album: text(row.album) || track.album,
      coverId,
      thumb,
      lyricsId,
      lyrics,
      mime: text(row.mime) || track.mime,
      size: Number(row.size || track.size || 0) || 0,
      duration: Number(row.duration || track.duration || 0) || 0,
      video: row.kind === "video" || track.video,
      name: text(row.name) || track.name,
    }
  })
}

function rowToTrack(row) {
  return {
    id: row.id,
    name: text(row.name),
    title: text(row.title) || strip(text(row.name)),
    artist: text(row.artist),
    album: text(row.album),
    video: row.kind === "video",
    lyricsId: text(row.lyricsId),
    coverId: text(row.coverId),
    mime: text(row.mime),
    size: Number(row.size || 0) || 0,
    duration: Number(row.duration || 0) || 0,
    lyrics: lyricBody(text(row.lyrics)),
  }
}

function text(value) {
  return typeof value === "string" ? value.trim() : ""
}

/** Union likes and playlists. The newer queue wins. Nothing is dropped on a tie except the older queue. */
export function mergePhoneState(local, remote) {
  const left = normalizeState(local)
  const right = normalizeState(remote)
  const likes = [...new Set([...left.likes, ...right.likes])]
  const byName = new Map()
  for (const playlist of [...left.playlists, ...right.playlists]) {
    const key = playlist.name.toLocaleLowerCase()
    const current = byName.get(key)
    if (!current) byName.set(key, { name: playlist.name, paths: [...playlist.paths] })
    else current.paths = [...new Set([...current.paths, ...playlist.paths])]
  }
  const queue = right.updatedAt >= left.updatedAt ? right.queue : left.queue
  return {
    updatedAt: Math.max(left.updatedAt, right.updatedAt),
    likes,
    playlists: [...byName.values()],
    queue,
  }
}

export function stateFile(files) {
  const list = Array.isArray(files) ? files : []
  return (
    list.find((file) => file?.name === "biblioteca.json" || file?.name === "state.json") ||
    list.find((file) => file?.appProperties?.bobKind === "state") ||
    null
  )
}

export function trackIsLiked(likes, track) {
  const id = text(track?.id).toLowerCase()
  const name = text(track?.name).toLowerCase()
  if (!id && !name) return false
  for (const key of Array.isArray(likes) ? likes : []) {
    const raw = text(key).toLowerCase()
    if (!raw) continue
    if (raw === id || raw === `drive:${id}`) return true
    const file = raw.split(/[\\/]/).pop()
    if (file && name && file === name) return true
  }
  return false
}

export function coverPlaceholder(track) {
  const label = `${text(track?.artist)} ${text(track?.title)}`.trim() || "bob"
  let hash = 0
  for (const ch of label) hash = (hash * 33 + ch.charCodeAt(0)) >>> 0
  const hue = [18, 28, 152, 188, 210, 262, 330, 48][hash % 8]
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 80"><rect width="80" height="80" fill="hsl(${hue} 34% 42%)"/><path d="M34 56V24l22-6v28" fill="none" stroke="#fff" stroke-width="3" stroke-linecap="round"/><circle cx="30" cy="56" r="6" fill="#fff"/><circle cx="52" cy="46" r="6" fill="#fff"/></svg>`
  return `data:image/svg+xml,${encodeURIComponent(svg)}`
}

function coverIdFor(track, files) {
  const id = text(track?.id)
  if (!id) return ""
  const list = Array.isArray(files) ? files : []
  for (const file of list) {
    if (!file?.id || file.id === id) continue
    const meta = file.appProperties || {}
    const name = String(file.name || "")
    const image = meta.bobKind === "cover" || String(file.mimeType || "").startsWith("image/")
    if (!image) continue
    if (meta.bobMedia === id) return file.id
    if (name === `${id}.jpg` || name === `${id}.png` || name === `${id}.webp` || name === `${id}.cover.jpg` || name === `${id}.cover.png`) return file.id
  }
  return ""
}

function remoteImage(value) {
  const raw = text(value)
  return /^https?:\/\//i.test(raw) || raw.startsWith("data:image/") ? raw : ""
}

function normalizeState(value) {
  const state = value && typeof value === "object" ? value : {}
  const paths = (list) => (Array.isArray(list) ? list.filter((item) => typeof item === "string" && item) : [])
  const playlists = Array.isArray(state.playlists)
    ? state.playlists
        .map((playlist) => ({
          name: text(playlist?.name).slice(0, 80),
          paths: paths(playlist?.paths),
        }))
        .filter((playlist) => playlist.name)
    : []
  return {
    updatedAt: Number(state.updatedAt) || 0,
    likes: paths(state.likes),
    playlists,
    queue: paths(state.queue),
  }
}

export function extensionOf(name) {
  const match = /\.([a-z0-9]+)$/i.exec(String(name || ""))
  return match ? match[1].toLowerCase() : ""
}

export function deviceSupport(canPlayType, userAgent) {
  const play = typeof canPlayType === "function" ? canPlayType : () => ""
  const ios = /iPhone|iPad|iPod/i.test(String(userAgent || ""))
  const opus = !ios && (play('audio/ogg; codecs="opus"') !== "" || play('audio/webm; codecs="opus"') !== "")
  return { ios, opus }
}

/** Opus and WebM audio do not play on iOS. An m4a of the same song is used when the index has one. */
export function choosePlayback(track, tracks, support) {
  const ext = extensionOf(track?.name)
  if (track?.video || ["mp4", "mkv", "mov", "m4v"].includes(ext) || (ext === "webm" && track?.video)) {
    return { element: "video", id: track.id, mime: mimeFor(ext, true), unsupported: false, reason: "" }
  }
  const blocked = ["opus", "ogg", "webm", "flac"].includes(ext) && (support?.ios || support?.opus === false)
  if (blocked) {
    const alt = (tracks || []).find((item) => item.id !== track.id && sameSong(item, track) && iosAudio(item))
    if (alt) return { element: "audio", id: alt.id, mime: mimeFor(extensionOf(alt.name), false), unsupported: false, reason: "" }
    return {
      element: "audio",
      id: track.id,
      mime: "",
      unsupported: true,
      reason: "este celular não toca opus nem webm. se o computador tiver uma versão m4a, ela entra no lugar.",
    }
  }
  return { element: "audio", id: track.id, mime: track.mime || mimeFor(ext, false), unsupported: false, reason: "" }
}

function iosAudio(track) {
  return ["m4a", "mp3", "aac", "mp4"].includes(extensionOf(track.name)) && !track.video
}

function sameSong(left, right) {
  const title = (value) => String(value || "").trim().toLocaleLowerCase()
  return title(left.title) && title(left.title) === title(right.title) && title(left.artist) === title(right.artist)
}

function mimeFor(ext, video) {
  if (video || ext === "mp4" || ext === "m4v" || ext === "mkv" || ext === "mov") {
    if (ext === "webm") return "video/webm"
    if (ext === "mkv") return "video/x-matroska"
    return "video/mp4"
  }
  if (ext === "mp3") return "audio/mpeg"
  if (ext === "m4a" || ext === "aac") return "audio/mp4"
  if (ext === "wav") return "audio/wav"
  if (ext === "flac") return "audio/flac"
  if (ext === "webm") return "audio/webm"
  if (ext === "opus" || ext === "ogg") return "audio/ogg"
  return "application/octet-stream"
}

export function lyricLines(value) {
  if (!isLyricText(value)) return []
  const source = String(value || "")
  const offsetMatch = source.match(/\[offset:\s*([+-]?\d+)\s*\]/i)
  const offset = offsetMatch ? Number(offsetMatch[1]) / 1000 : 0
  const lines = []
  for (const raw of source.split(/\r?\n/)) {
    const stamps = [...raw.matchAll(/\[(\d+):(\d+(?:\.\d+)?)\]/g)]
    const words = raw.replace(/\[[^\]]+\]/g, "").trim()
    if (!stamps.length) {
      if (words && !raw.trim().startsWith("[")) lines.push({ at: null, text: words })
      continue
    }
    if (!words) continue
    for (const stamp of stamps) {
      lines.push({ at: Number(stamp[1]) * 60 + Number(stamp[2]) + offset, text: words })
    }
  }
  return lines.sort((left, right) => (left.at ?? 1e9) - (right.at ?? 1e9))
}

export function activeLyric(lines, time) {
  let found = -1
  for (let index = 0; index < lines.length; index += 1) {
    if (lines[index].at == null) continue
    if (lines[index].at <= time + 0.05) found = index
  }
  return found
}

const LABELS = ["gondwana records", "sub pop", "4ad", "domino", "matador", "xl recordings", "dead oceans", "secret city"]

export function foldKey(value) {
  return String(value || "")
    .normalize("NFD")
    .replace(/\p{M}/gu, "")
    .toLocaleLowerCase()
    .replace(/[^a-z0-9]+/g, " ")
    .trim()
}

export function isLabel(value) {
  return LABELS.includes(foldKey(value))
}

export function presentLabel(value) {
  const raw = String(value || "").trim()
  if (!raw) return ""
  const letters = raw.replace(/[^\p{L}]/gu, "")
  if (letters.length > 2 && letters === letters.toLocaleUpperCase()) {
    return raw.toLocaleLowerCase().replace(/(^|[\s'’\-])(\p{L})/gu, (all, lead, ch) => lead + ch.toLocaleUpperCase())
  }
  return raw
}

export function tidyTitle(title, artist = "") {
  let text = String(title || "").trim()
  text = text.replace(/\s*\[[^\]]*\]/g, "")
  text = text.replace(/\s*[([【][^)\]】]*(official|lyric|audio|mood|video)[^)\]】]*[)\]】]/gi, "")
  text = text.replace(/\s*[-–—]\s*official video\s*$/i, "")
  text = text.replace(/^['"“”‘’]+|['"“”‘’]+$/g, "")
  text = text.replace(/\s+/g, " ").trim()
  const artistText = String(artist || "").trim()
  for (let guard = 0; artistText && guard < 3; guard += 1) {
    const parts = text.split(/\s+[-–—]\s+/)
    if (parts.length > 1 && similarName(parts[0], artistText)) {
      text = parts.slice(1).join(" - ").trim()
      continue
    }
    if (foldKey(text).startsWith(foldKey(artistText)) && text.length > artistText.length + 1) {
      text = text.slice(artistText.length).replace(/^[\s\-–—:]+/, "").trim()
      continue
    }
    break
  }
  return presentLabel(text.replace(/^['"“”‘’]+|['"“”‘’]+$/g, "").trim())
}

export function splitDisplay(name) {
  let raw = String(name || "").replace(/\.[a-z0-9]{2,5}$/i, "")
  raw = raw.replace(/\s*\[[^\]]*\]/g, "")
  raw = raw.replace(/\s*[([【][^)\]】]*(official|lyric|audio|mood|video)[^)\]】]*[)\]】]/gi, "")
  const parts = raw.split(/\s+[-–—]\s+/).map((part) => part.replace(/^['"“”‘’]+|['"“”‘’]+$/g, "").trim()).filter(Boolean)
  const kept = parts.filter((part) => !isLabel(part))
  if (kept.length >= 2) {
    const artist = presentLabel(kept[0])
    return { artist, title: tidyTitle(kept.slice(1).join(" - "), artist) }
  }
  if (kept.length === 1) return { artist: "", title: tidyTitle(kept[0]) }
  return { artist: "", title: tidyTitle(raw) }
}

export function normalizeTrack(track) {
  const source = track && typeof track === "object" ? track : {}
  const fromName = splitDisplay(source.name)
  const fromTitle = splitDisplay(source.title)
  let artist = text(source.artist)
  let title = text(source.title)
  const bare = String(source.name || "").replace(/\.[a-z0-9]{2,5}$/i, "")
  const noisy = /official|\[|\]|["“”‘’]/.test(title) || ((title.match(/ [-–—] /g) || []).length > 0 && (isLabel(artist) || !artist))
  if (!title || title === bare || title === source.name || noisy || isLabel(artist)) {
    const parsed = fromTitle.artist || fromTitle.title ? fromTitle : fromName
    if (parsed.artist) artist = parsed.artist
    if (parsed.title) title = parsed.title
    else if (fromName.title) title = fromName.title
  }
  if (!artist || isLabel(artist)) artist = fromName.artist || fromTitle.artist || (isLabel(artist) ? "" : artist)
  title = tidyTitle(title, artist)
  artist = presentLabel(artist)
  if (!title) title = fromName.title || bare || "sem título"
  return { ...source, title, artist }
}

export function dedupeTracks(tracks) {
  const map = new Map()
  for (const track of tracks || []) {
    const artist = foldKey(track.artist)
    const title = foldKey(track.title)
    const key = artist && title ? `${artist}|${title}` : track.id
    const current = map.get(key)
    if (!current || richness(track) > richness(current)) map.set(key, track)
  }
  return [...map.values()]
}

export function isLyricText(value) {
  if (typeof value !== "string") return false
  const sample = value.replace(/^\uFEFF/, "").trim()
  if (!sample || sample.length > 200_000) return false
  if (sample.includes("\u0000") || sample.includes("\uFFFD")) return false
  const head = sample.slice(0, 64)
  if (head.startsWith("ÿØÿ") || head.startsWith("GIF8") || head.startsWith("RIFF") || head.includes("JFIF") || head.includes("PNG")) return false
  let controls = 0
  for (const ch of head) {
    const code = ch.charCodeAt(0)
    if (code < 9 || (code > 13 && code < 32)) controls += 1
  }
  return controls < 2
}

export function lyricIdFor(file, list) {
  const props = (item) => item?.appProperties || {}
  const hit = (Array.isArray(list) ? list : []).find((item) => {
    const meta = props(item)
    if (meta.bobMedia !== file?.id) return false
    if (meta.bobKind === "cover") return false
    const mime = String(item?.mimeType || "")
    if (mime.startsWith("image/") || mime.startsWith("audio/") || mime.startsWith("video/")) return false
    const name = String(item?.name || "")
    return meta.bobKind === "lyrics" || /\.(lrc|txt)$/i.test(name)
  })
  return hit?.id || ""
}

function safeLyricsId(lyricsId, coverId) {
  if (!lyricsId || lyricsId === coverId) return ""
  return lyricsId
}

function lyricBody(value) {
  return isLyricText(value) ? value.trim() : ""
}

function similarName(left, right) {
  const a = foldKey(left)
  const b = foldKey(right)
  if (!a || !b) return false
  if (a === b) return true
  let index = 0
  while (index < a.length && index < b.length && a[index] === b[index]) index += 1
  return index >= 8 && Math.min(a.length, b.length) - index <= 3
}

function richness(track) {
  return (track.lyrics ? 4 : 0) + (track.coverId ? 2 : 0) + (track.duration ? 1 : 0)
}
