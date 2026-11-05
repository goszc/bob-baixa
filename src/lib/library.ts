import { fileName, isMediaPath } from "./format"
import type { ConvertJob, HistoryEntry, Job } from "./types"

export interface Track {
  id: string
  path: string
  title: string
  artist: string
  pageUrl: string
  thumbnail?: string
  duration?: number
  kind: "audio" | "video"
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

function norm(path: string) {
  return path.replaceAll("\\", "/").replace(/\/+$/, "").toLocaleLowerCase()
}

/** Settings stores this as `outputDir`. A file counts only when it is inside that folder. */
export function isInsideLibrary(path: string, outputDir: string): boolean {
  const root = canonical(outputDir)
  const file = canonical(path)
  if (!root || !file || file === root) return false
  return file.startsWith(`${root}/`)
}

function canonical(value: string) {
  const trimmed = value.trim()
  if (!trimmed) return ""
  return trimmed.replaceAll("\\", "/").replace(/\/+/g, "/").replace(/\/+$/, "").toLowerCase()
}

function stemTitle(path: string) {
  return fileName(path).replace(/\.[^.]+$/, "")
}

export function collectTracks(jobs: Job[], history: HistoryEntry[], convert: ConvertJob | null, outputDir: string): Track[] {
  const seen = new Set<string>()
  const tracks: Track[] = []
  const add = (track: Track) => {
    if (!track.path || !isMediaPath(track.path) || !isInsideLibrary(track.path, outputDir)) return
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
      pageUrl: "",
      kind: trackKind(convert.output),
    })
  }
  return tracks
}
