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
      return {
        id: file.id,
        name: file.name || "",
        title: meta.bobTitle || strip(file.name || ""),
        artist: meta.bobArtist || "",
        album: meta.bobAlbum || "",
        video: meta.bobKind === "video",
        lyricsId: related("lyrics")?.id || "",
        coverId: related("cover")?.id || "",
        mime: file.mimeType || "",
        size: Number(file.size || 0) || 0,
        duration: 0,
        lyrics: "",
      }
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
  return applyPhoneIndex([...fromFiles, ...extra], index)
}

export function applyPhoneIndex(tracks, index) {
  const rows = Array.isArray(index?.tracks) ? index.tracks : []
  const byId = new Map(rows.filter((row) => row?.id).map((row) => [row.id, row]))
  return tracks.map((track) => {
    const row = byId.get(track.id)
    if (!row) return track
    return {
      ...track,
      title: text(row.title) || track.title,
      artist: text(row.artist) || track.artist,
      album: text(row.album) || track.album,
      coverId: text(row.coverId) || track.coverId,
      lyricsId: text(row.lyricsId) || track.lyricsId,
      lyrics: text(row.lyrics) || track.lyrics,
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
    lyrics: text(row.lyrics),
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
  return list.find((file) => file?.appProperties?.bobKind === "state" || file?.name === "state.json") || null
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

export function lyricLines(text) {
  const source = String(text || "")
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
