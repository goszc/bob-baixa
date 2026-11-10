import { fileName, isMediaPath } from "./format.ts"
import type { ConvertJob, HistoryEntry, Job } from "./types"

export interface Track {
  id: string
  path: string
  title: string
  artist: string
  album: string
  folder: string
  pageUrl: string
  thumbnail?: string
  duration?: number
  kind: "audio" | "video"
  size?: number
}

const VIDEO = /\.(mp4|mkv|webm|mov|avi|m4v)$/i

export function trackKind(path: string): "audio" | "video" {
  return VIDEO.test(path) ? "video" : "audio"
}

export function splitArtistTitle(raw: string): { artist: string; title: string } {
  const text = raw.trim()
  for (const sep of [" - ", " – ", " — ", " | "]) {
    const index = text.indexOf(sep)
    if (index > 0) {
      const artist = text.slice(0, index).trim()
      const title = text.slice(index + sep.length).trim()
      if (artist && title) return { artist, title }
    }
  }
  return { artist: "", title: text }
}

/** `\\?\C:\...` and `\\?\UNC\...` are the same folder as the path without that prefix. */
function stripVerbatim(value: string) {
  const text = value.trim()
  const lower = text.toLowerCase()
  if (lower.startsWith("\\\\?\\unc\\")) return `\\\\${text.slice(8)}`
  if (lower.startsWith("//?/unc/")) return `//${text.slice(8)}`
  if (lower.startsWith("\\\\?\\")) return text.slice(4)
  if (lower.startsWith("//?/")) return text.slice(4)
  return text
}

function canonical(value: string) {
  const stripped = stripVerbatim(value)
  if (!stripped) return ""
  return stripped.replaceAll("\\", "/").replace(/\/+/g, "/").replace(/\/+$/, "").toLowerCase()
}

export function trackKey(path: string) {
  return canonical(path)
}

function norm(path: string) {
  return trackKey(path)
}

/** Settings stores this as `outputDir`. A file counts only when it is inside that folder. */
export function isInsideLibrary(path: string, outputDir: string): boolean {
  const root = canonical(outputDir)
  const file = canonical(path)
  if (!root || !file || file === root) return false
  return file.startsWith(`${root}/`)
}

function stemTitle(path: string) {
  return fileName(path).replace(/\.[^.]+$/, "")
}

export function collectTracks(jobs: Job[], history: HistoryEntry[], convert: ConvertJob | null, outputDir: string): Track[] {
  const seen = new Set<string>()
  const tracks: Track[] = []
  const add = (track: Track) => {
    const remote = track.path.startsWith("drive:")
    if (!track.path || (!remote && (!isMediaPath(track.path) || !isInsideLibrary(track.path, outputDir)))) return
    const key = norm(track.path)
    if (seen.has(key)) return
    seen.add(key)
    tracks.push(track)
  }
  for (const entry of history) {
    if (!entry.ok || !entry.path) continue
    const job = jobs.find((item) => item.outputPath && norm(item.outputPath) === norm(entry.path!))
    const named = splitArtistTitle(entry.title || stemTitle(entry.path))
    add({
      id: entry.id,
      path: entry.path,
      title: named.title || entry.title,
      artist: named.artist,
      album: "",
      folder: "",
      pageUrl: entry.url,
      thumbnail: job?.thumbnail,
      duration: job?.duration,
      kind: trackKind(entry.path),
    })
  }
  for (const job of jobs) {
    if (job.state !== "done" || !job.outputPath) continue
    const named = splitArtistTitle(job.title || stemTitle(job.outputPath))
    add({
      id: job.id,
      path: job.outputPath,
      title: named.title || job.title,
      artist: named.artist,
      album: "",
      folder: "",
      pageUrl: job.url,
      thumbnail: job.thumbnail,
      duration: job.duration,
      kind: trackKind(job.outputPath),
    })
  }
  if (convert?.state === "done" && convert.output) {
    const named = splitArtistTitle(stemTitle(convert.output))
    add({
      id: convert.id,
      path: convert.output,
      title: named.title,
      artist: named.artist,
      album: "",
      folder: "",
      pageUrl: "",
      kind: trackKind(convert.output),
    })
  }
  return tracks
}

export function youtubeId(path: string): string | null {
  const name = fileName(path)
  const match = /\[([A-Za-z0-9_-]{11})\](?:\.[^.]+)?$/.exec(name)
  return match?.[1] ?? null
}

function identityKey(track: Track): string | null {
  const id = youtubeId(track.path)
  if (id) return `${track.kind}:yt:${id}`
  const artist = track.artist.trim().toLocaleLowerCase()
  const title = track.title.trim().toLocaleLowerCase()
  if (!artist || !title || track.duration == null || !Number.isFinite(track.duration)) return null
  const bucket = Math.floor(track.duration / 3)
  return `${track.kind}:meta:${artist}:${title}:${bucket}`
}

function richness(track: Track) {
  return (
    (track.thumbnail ? 4 : 0) +
    (track.artist ? 2 : 0) +
    (track.album ? 1 : 0) +
    (track.duration ? 1 : 0) +
    Math.min(track.size ?? 0, 1_000_000_000) / 1_000_000_000
  )
}

/** Same path, same YouTube id, or the same artist/title/duration. Audio and video stay apart. */
export function dedupeTracks(tracks: Track[]): Track[] {
  const byPath = new Map<string, Track>()
  for (const track of tracks) {
    const key = trackKey(track.path)
    const prev = byPath.get(key)
    if (!prev || richness(track) > richness(prev)) byPath.set(key, track)
  }
  const seen = new Map<string, Track>()
  const order: string[] = []
  for (const track of byPath.values()) {
    const key = identityKey(track) ?? trackKey(track.path)
    const prev = seen.get(key)
    if (!prev) {
      seen.set(key, track)
      order.push(key)
      continue
    }
    if (richness(track) > richness(prev)) seen.set(key, track)
  }
  return order.map((key) => seen.get(key)).filter((track): track is Track => Boolean(track))
}

export interface TrackGroup {
  name: string
  tracks: Track[]
}

export function groupTracks(tracks: Track[], field: "artist" | "album" | "folder", empty: string): TrackGroup[] {
  const groups = new Map<string, Track[]>()
  for (const track of tracks) {
    const raw = field === "artist" ? track.artist : field === "album" ? track.album : track.folder
    const name = raw.trim() || empty
    const list = groups.get(name) ?? []
    list.push(track)
    groups.set(name, list)
  }
  return [...groups.entries()]
    .map(([name, items]) => ({ name, tracks: items }))
    .sort((left, right) => left.name.localeCompare(right.name, undefined, { sensitivity: "base" }))
}

function sameText(left: string, right: string) {
  const a = left.trim().toLocaleLowerCase()
  const b = right.trim().toLocaleLowerCase()
  return a.length > 0 && a === b
}

/** Shuffled local tracks that share artist, album or folder with the seed. The seed stays first. */
export function relatedTracks(seed: Track, library: Track[]): Track[] {
  const others = library.filter((track) => trackKey(track.path) !== trackKey(seed.path))
  const artist = others.filter((track) => sameText(track.artist, seed.artist))
  const album = others.filter((track) => sameText(track.album, seed.album) && !artist.some((item) => trackKey(item.path) === trackKey(track.path)))
  const folder = others.filter((track) => {
    if (!sameText(track.folder, seed.folder)) return false
    const key = trackKey(track.path)
    return !artist.some((item) => trackKey(item.path) === key) && !album.some((item) => trackKey(item.path) === key)
  })
  const pool = [...artist, ...album, ...folder]
  for (let index = pool.length - 1; index > 0; index -= 1) {
    const swap = Math.floor(Math.random() * (index + 1))
    const held = pool[index]
    pool[index] = pool[swap]
    pool[swap] = held
  }
  return [seed, ...pool]
}
