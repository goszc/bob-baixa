import { useSyncExternalStore } from "react"
import { invoke, isTauri } from "./api"
import { collectTracks, dedupeTracks, isInsideLibrary, trackKey, type Track } from "./library"
import type { ConvertJob, HistoryEntry, Job } from "./types"

export interface DiskFile {
  path: string
  title: string
  artist: string
  album: string
  folder: string
  duration?: number | null
  kind: string
  size: number
  modifiedMs: number
}

let disk: DiskFile[] | null = null
let removed = new Set<string>()
const listeners = new Set<() => void>()
let request = 0

function emit() {
  listeners.forEach((listener) => listener())
}

export function useCatalog() {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    () => disk,
    () => disk,
  )
}

export function markRemoved(path: string) {
  removed.add(trackKey(path))
  emit()
}

export function unmarkRemoved(path: string) {
  removed.delete(trackKey(path))
  emit()
}

export async function refreshCatalog() {
  if (!isTauri()) {
    disk = null
    emit()
    return
  }
  const mine = ++request
  try {
    const files = await invoke<DiskFile[]>("scan_library")
    if (mine !== request) return
    disk = files
    emit()
  } catch {
    if (mine !== request) return
  }
}

export function catalogTracks(jobs: Job[], history: HistoryEntry[], convert: ConvertJob | null, outputDir: string): Track[] {
  const hidden = removed
  const fromHistory = collectTracks(jobs, history, convert, outputDir).map((track) => withPlace(track, outputDir))
  if (!disk) return dedupeTracks(fromHistory.filter((track) => !hidden.has(trackKey(track.path))))
  const meta = new Map<string, { thumbnail?: string; pageUrl: string; duration?: number }>()
  for (const track of fromHistory) {
    meta.set(trackKey(track.path), { thumbnail: track.thumbnail, pageUrl: track.pageUrl, duration: track.duration })
  }
  const scanned: Track[] = []
  for (const file of disk) {
    if (!file.path || !isInsideLibrary(file.path, outputDir) || hidden.has(trackKey(file.path))) continue
    const extra = meta.get(trackKey(file.path))
    const kind = file.kind === "video" ? "video" : "audio"
    scanned.push({
      id: trackKey(file.path),
      path: file.path,
      title: file.title || file.path,
      artist: file.artist || "",
      album: file.album || "",
      folder: file.folder || "",
      pageUrl: extra?.pageUrl ?? "",
      thumbnail: extra?.thumbnail,
      duration: file.duration ?? extra?.duration,
      kind,
      size: file.size,
    })
  }
  return dedupeTracks(scanned)
}

function withPlace(track: Track, outputDir: string): Track {
  const file = trackKey(track.path)
  const root = trackKey(outputDir)
  if (!root || !file.startsWith(`${root}/`)) return { ...track, album: track.album || "", folder: track.folder || "" }
  const rel = file.slice(root.length + 1)
  const parts = rel.split("/")
  parts.pop()
  const folder = parts.join("/")
  const album = track.album || parts[parts.length - 1] || ""
  return { ...track, folder, album }
}
