import { useSyncExternalStore } from "react"
import type { ListenEvent, LibraryManifest } from "./listening"
import { parseManifest } from "./listening"
import { fileName } from "./format.ts"
import { trackKey, type Track } from "./library"

export type LibraryView = "tracks" | "artists" | "albums" | "folders" | "playlists" | "likes" | "recent" | "retro" | "mix" | "care"
export type EqScope = "global" | "track"

export interface EqMemory {
  preset: string
  bands: number[]
}
export type SleepMode = "off" | "15" | "30" | "45" | "60" | "end"

export interface UserPlaylist {
  id: string
  name: string
  paths: string[]
}

export interface SessionSnapshot {
  paths: string[]
  index: number
  position: number
  shuffle: boolean
  repeat: "off" | "all" | "one"
}

interface MusicState {
  playlists: UserPlaylist[]
  likes: string[]
  resume: Record<string, number>
  recent: { path: string; at: number }[]
  events: ListenEvent[]
  crossfade: number
  normalize: boolean
  sleepMode: SleepMode
  sleepUntil: number | null
  session: SessionSnapshot | null
  view: LibraryView
  updatedAt: number
  eqPreset: string
  eqBands: number[]
  eqScope: EqScope
  eqByPath: Record<string, EqMemory>
}

const KEY = "bob-baixa-music"
const empty = (): MusicState => ({
  playlists: [],
  likes: [],
  resume: {},
  recent: [],
  events: [],
  crossfade: 0,
  normalize: false,
  sleepMode: "off",
  sleepUntil: null,
  session: null,
  view: "tracks",
  updatedAt: 0,
  eqPreset: "flat",
  eqBands: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  eqScope: "global",
  eqByPath: {},
})

function load(): MusicState {
  if (typeof localStorage === "undefined") return empty()
  try {
    const raw = localStorage.getItem(KEY)
    if (!raw) return empty()
    const parsed = JSON.parse(raw) as Partial<MusicState>
    const base = empty()
    return {
      ...base,
      ...parsed,
      playlists: Array.isArray(parsed.playlists) ? parsed.playlists : [],
      likes: Array.isArray(parsed.likes) ? parsed.likes : [],
      resume: parsed.resume && typeof parsed.resume === "object" ? parsed.resume : {},
      recent: Array.isArray(parsed.recent) ? parsed.recent : [],
      events: Array.isArray(parsed.events) ? parsed.events.slice(-4000) : [],
      crossfade: typeof parsed.crossfade === "number" ? Math.min(12, Math.max(0, Math.round(parsed.crossfade))) : 0,
      normalize: Boolean(parsed.normalize),
      view: parsed.view ?? "tracks",
      updatedAt: typeof parsed.updatedAt === "number" ? parsed.updatedAt : 0,
      eqPreset: typeof parsed.eqPreset === "string" ? parsed.eqPreset : "flat",
      eqBands: Array.isArray(parsed.eqBands) ? parsed.eqBands.map((band) => Number(band) || 0).slice(0, 10) : base.eqBands,
      eqScope: parsed.eqScope === "track" ? "track" : "global",
      eqByPath: parsed.eqByPath && typeof parsed.eqByPath === "object" ? parsed.eqByPath : {},
    }
  } catch {
    return empty()
  }
}

let state = load()
const listeners = new Set<() => void>()
let openListen: ListenEvent | null = null
let prefHook: (() => void) | null = null
let syncHook: (() => void) | null = null
let silentSync = false

function emit() {
  listeners.forEach((listener) => listener())
}

function persist() {
  const next = {
    ...state,
    events: state.events.slice(-4000),
    updatedAt: silentSync ? state.updatedAt : Date.now(),
  }
  state = next
  if (typeof localStorage !== "undefined") localStorage.setItem(KEY, JSON.stringify(next))
  emit()
  if (!silentSync) syncHook?.()
}

export function getMusic() {
  return state
}

export function useMusic() {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    getMusic,
    getMusic,
  )
}

export function onAudioPref(listener: () => void) {
  prefHook = listener
}

export function onLibrarySync(listener: () => void) {
  syncHook = listener
  return () => {
    if (syncHook === listener) syncHook = null
  }
}

export function activeEq(path?: string) {
  if (state.eqScope === "track" && path) {
    const saved = state.eqByPath[trackKey(path)]
    if (saved) return saved
  }
  return { preset: state.eqPreset, bands: state.eqBands }
}

export function setEq(preset: string, bands: number[], scope: EqScope, path?: string) {
  if (scope === "track" && path) {
    state = {
      ...state,
      eqScope: "track",
      eqByPath: { ...state.eqByPath, [trackKey(path)]: { preset, bands } },
    }
  } else {
    state = { ...state, eqScope: scope, eqPreset: preset, eqBands: bands }
  }
  persist()
  prefHook?.()
}

export function librarySnapshot() {
  return {
    updatedAt: state.updatedAt,
    playlists: state.playlists,
    likes: state.likes,
    events: state.events,
    recent: state.recent,
  }
}

export function applyLibrarySnapshot(raw: string) {
  try {
    const parsed = JSON.parse(raw) as Partial<MusicState> & { updatedAt?: number }
    if (!parsed || typeof parsed !== "object") return
    silentSync = true
    state = {
      ...state,
      updatedAt: typeof parsed.updatedAt === "number" ? parsed.updatedAt : state.updatedAt,
      playlists: Array.isArray(parsed.playlists) ? parsed.playlists : state.playlists,
      likes: Array.isArray(parsed.likes) ? parsed.likes : state.likes,
      events: Array.isArray(parsed.events) ? parsed.events.slice(-4000) : state.events,
      recent: Array.isArray(parsed.recent) ? parsed.recent : state.recent,
    }
    persist()
    silentSync = false
  } catch {
    silentSync = false
  }
}

export function setView(view: LibraryView) {
  state = { ...state, view }
  persist()
}

export function setCrossfade(seconds: number) {
  const crossfade = Math.min(12, Math.max(0, Math.round(seconds)))
  state = { ...state, crossfade }
  persist()
  prefHook?.()
}

export function setNormalize(normalize: boolean) {
  state = { ...state, normalize }
  persist()
  prefHook?.()
}

export function setSleep(mode: SleepMode) {
  if (mode === "off") {
    state = { ...state, sleepMode: "off", sleepUntil: null }
  } else if (mode === "end") {
    state = { ...state, sleepMode: "end", sleepUntil: null }
  } else {
    const minutes = Number(mode)
    state = { ...state, sleepMode: mode, sleepUntil: Date.now() + minutes * 60_000 }
  }
  persist()
}

export function clearSleep() {
  if (state.sleepMode === "off" && state.sleepUntil == null) return
  state = { ...state, sleepMode: "off", sleepUntil: null }
  persist()
}

export function liked(path: string) {
  return state.likes.includes(trackKey(path))
}

export function toggleLike(path: string) {
  const key = trackKey(path)
  const likes = state.likes.includes(key) ? state.likes.filter((item) => item !== key) : [key, ...state.likes]
  state = { ...state, likes }
  persist()
}

export function createPlaylist(name: string) {
  const trimmed = name.trim()
  if (!trimmed) return ""
  const playlist: UserPlaylist = { id: `pl-${Date.now().toString(36)}`, name: trimmed.slice(0, 80), paths: [] }
  state = { ...state, playlists: [playlist, ...state.playlists] }
  persist()
  return playlist.id
}

export function renamePlaylist(id: string, name: string) {
  const trimmed = name.trim()
  if (!trimmed) return
  state = {
    ...state,
    playlists: state.playlists.map((playlist) => (playlist.id === id ? { ...playlist, name: trimmed.slice(0, 80) } : playlist)),
  }
  persist()
}

export function deletePlaylist(id: string) {
  state = { ...state, playlists: state.playlists.filter((playlist) => playlist.id !== id) }
  persist()
}

export function addToPlaylist(id: string, path: string) {
  const key = trackKey(path)
  state = {
    ...state,
    playlists: state.playlists.map((playlist) =>
      playlist.id === id && !playlist.paths.some((item) => trackKey(item) === key) ? { ...playlist, paths: [...playlist.paths, path] } : playlist,
    ),
  }
  persist()
}

export function removeFromPlaylist(id: string, path: string) {
  const key = trackKey(path)
  state = {
    ...state,
    playlists: state.playlists.map((playlist) =>
      playlist.id === id ? { ...playlist, paths: playlist.paths.filter((item) => trackKey(item) !== key) } : playlist,
    ),
  }
  persist()
}

export function moveInPlaylist(id: string, path: string, direction: -1 | 1) {
  state = {
    ...state,
    playlists: state.playlists.map((playlist) => {
      if (playlist.id !== id) return playlist
      const index = playlist.paths.findIndex((item) => trackKey(item) === trackKey(path))
      const next = index + direction
      if (index < 0 || next < 0 || next >= playlist.paths.length) return playlist
      const paths = playlist.paths.slice()
      const [item] = paths.splice(index, 1)
      paths.splice(next, 0, item)
      return { ...playlist, paths }
    }),
  }
  persist()
}

export function rememberPosition(path: string, seconds: number) {
  if (!Number.isFinite(seconds) || seconds < 1) return
  const key = trackKey(path)
  const prev = state.resume[key] ?? 0
  if (Math.abs(prev - seconds) < 2) return
  state = { ...state, resume: { ...state.resume, [key]: seconds } }
  persist()
}

export function resumeFor(path: string) {
  return state.resume[trackKey(path)] ?? 0
}

export function touchRecent(path: string) {
  const key = trackKey(path)
  const recent = [{ path, at: Date.now() }, ...state.recent.filter((item) => trackKey(item.path) !== key)].slice(0, 40)
  state = { ...state, recent }
  persist()
}

export function rememberSession(session: SessionSnapshot | null) {
  state = { ...state, session }
  persist()
}

export function hear(path: string, title: string, artist: string, deltaMs: number, at: number) {
  if (!path || deltaMs < 80 || deltaMs > 2500) return
  if (!openListen || trackKey(openListen.path) !== trackKey(path)) {
    flushHear(at)
    openListen = { path, title, artist, at, ms: 0, play: false }
  }
  openListen.ms += deltaMs
  openListen.title = title || openListen.title
  openListen.artist = artist || openListen.artist
  if (!openListen.play && openListen.ms >= 30_000) openListen.play = true
}

export function flushHear(at: number) {
  if (!openListen) return
  if (openListen.ms >= 5_000) {
    state = { ...state, events: [...state.events, { ...openListen, at: openListen.at || at }].slice(-4000) }
    persist()
  }
  openListen = null
}

export function exportManifest(): string {
  const body: LibraryManifest = {
    version: 1,
    playlists: state.playlists.map((playlist) => ({ name: playlist.name, paths: playlist.paths })),
    likes: state.likes,
  }
  return JSON.stringify(body, null, 2)
}

export function importManifest(text: string) {
  const parsed = parseManifest(text)
  if (!parsed) return false
  const playlists = [
    ...parsed.playlists.map((playlist, index) => ({
      id: `imp-${Date.now().toString(36)}-${index}`,
      name: playlist.name,
      paths: playlist.paths,
    })),
    ...state.playlists.filter((playlist) => !parsed.playlists.some((item) => item.name.toLocaleLowerCase() === playlist.name.toLocaleLowerCase())),
  ]
  const likes = [...new Set([...parsed.likes.map(trackKey), ...state.likes])]
  state = { ...state, playlists, likes }
  persist()
  return true
}

export function playlistM3u(tracks: Track[]): string {
  const lines = ["#EXTM3U"]
  for (const track of tracks) {
    const seconds = track.duration ? Math.round(track.duration) : -1
    const label = track.artist ? `${track.artist} - ${track.title}` : track.title
    lines.push(`#EXTINF:${seconds},${label}`)
    lines.push(track.path)
  }
  return lines.join("\n")
}

export function matchM3u(lines: string[], tracks: Track[]): string[] {
  const byPath = new Map(tracks.map((track) => [trackKey(track.path), track.path]))
  const byName = new Map(tracks.map((track) => [fileName(track.path).toLocaleLowerCase(), track.path]))
  const found: string[] = []
  for (const line of lines) {
    const hit = byPath.get(trackKey(line)) ?? byName.get(fileName(line).toLocaleLowerCase())
    if (hit && !found.some((item) => trackKey(item) === trackKey(hit))) found.push(hit)
  }
  return found
}

export function tracksForPaths(paths: string[], library: Track[]): Track[] {
  return paths
    .map((path) => library.find((track) => trackKey(track.path) === trackKey(path)))
    .filter((track): track is Track => Boolean(track))
}
