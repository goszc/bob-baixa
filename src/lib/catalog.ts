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
let remoteTracks: Track[] = []
let settled = false
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

export function useCatalogSettled() {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    () => settled || !isTauri(),
    () => true,
  )
}

export async function refreshCatalog(dir?: string, liveDir?: () => string) {
  if (!isTauri()) {
    disk = null
    settled = true
    emit()
    return
  }
  const outputDir = (dir ?? liveDir?.() ?? "").trim()
  if (!outputDir) return
  const mine = ++request
  try {
    const files = await invoke<DiskFile[]>("scan_library", { path: outputDir })
    if (mine !== request) return
    if (liveDir && trackKey(liveDir()) !== trackKey(outputDir)) return
    disk = files
    try {
      const drive = await invoke<Array<{ id: string; path: string; title: string; artist: string; album: string; duration?: number | null; kind: string; size: number; name?: string; cover?: string }>>("drive_tracks")
      remoteTracks = drive.map((file) => ({
        id: file.id,
        path: file.path,
        title: file.title || file.name || file.path,
        artist: file.artist || "",
        album: file.album || "",
        folder: "drive",
        pageUrl: file.name || file.title || "",
        thumbnail: file.cover || undefined,
        duration: file.duration ?? undefined,
        kind: file.kind === "video" ? "video" : "audio",
        size: file.size,
      }))
    } catch {
      remoteTracks = []
    }
    settled = true
    emit()
  } catch {
    if (mine !== request) return
    settled = true
    emit()
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
  const present = new Set(scanned.map((track) => trackKey(track.path)))
  const extra = remoteTracks.filter((track) => !present.has(trackKey(track.path)) && !hidden.has(trackKey(track.path)))
  return dedupeTracks([...scanned, ...extra])
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
