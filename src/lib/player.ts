import { useSyncExternalStore } from "react"
import { invoke, isTauri } from "./api"
import type { Track } from "./library"

export type RepeatMode = "off" | "all" | "one"
export type PlayerPanel = "library" | "queue" | "lyrics"
export type PlaybackKind = "audio" | "video"

export interface LyricLine {
  at: number
  text: string
}

export interface TrackLyrics {
  source: string
  synced: boolean
  instrumental: boolean
  lines: LyricLine[]
  plain: string
}

export interface MediaInspect {
  exists: boolean
  kind: string
  title: string
  artist: string
  album: string
  duration?: number | null
  src: string
}

export interface PlayerMeta {
  title: string
  artist: string
  album: string
  duration?: number | null
}

export interface PlayerState {
  queue: Track[]
  order: number[]
  index: number
  playing: boolean
  currentTime: number
  duration: number
  volume: number
  muted: boolean
  shuffle: boolean
  repeat: RepeatMode
  panel: PlayerPanel
  lyrics: TrackLyrics | null
  lyricsState: "idle" | "loading" | "ready"
  error: "missing" | "failed" | null
  meta: PlayerMeta | null
  playbackKind: PlaybackKind | null
}

const sampleLyrics: TrackLyrics = {
  source: "file",
  synced: true,
  instrumental: false,
  plain: "Noite acesa\nSom que me leva\nPasso por passo\nTudo vibra agora",
  lines: [
    { at: 0.4, text: "Noite acesa" },
    { at: 2, text: "Som que me leva" },
    { at: 4, text: "Passo por passo" },
    { at: 6, text: "Tudo vibra agora" },
  ],
}

function emptyLyrics(): TrackLyrics {
  return { source: "none", synced: false, instrumental: false, lines: [], plain: "" }
}

function readVolume() {
  if (typeof localStorage === "undefined") return 0.9
  const raw = localStorage.getItem("bob-baixa-volume")
  if (raw == null || raw.trim() === "") return 0.9
  const value = Number(raw)
  if (!Number.isFinite(value)) return 0.9
  return Math.min(1, Math.max(0, value))
}

function readMuted(volume: number) {
  if (typeof localStorage === "undefined") return volume === 0
  if (localStorage.getItem("bob-baixa-muted") === "1") return true
  return volume === 0
}

function persistLevel(volume: number, muted: boolean) {
  localStorage.setItem("bob-baixa-volume", String(volume))
  localStorage.setItem("bob-baixa-muted", muted ? "1" : "0")
}

function initial(): PlayerState {
  const volume = readVolume()
  return {
    queue: [],
    order: [],
    index: -1,
    playing: false,
    currentTime: 0,
    duration: 0,
    volume,
    muted: readMuted(volume),
    shuffle: false,
    repeat: "off",
    panel: "library",
    lyrics: null,
    lyricsState: "idle",
    error: null,
    meta: null,
    playbackKind: null,
  }
}

let player = initial()
const listeners = new Set<() => void>()
let audio: HTMLAudioElement | null = null
let video: HTMLVideoElement | null = null
let park: HTMLElement | null = null
let slot: HTMLElement | null = null
let generation = 0
let frame = 0
let lastTick = 0

function emit() {
  listeners.forEach((listener) => listener())
}

function setPlayer(patch: Partial<PlayerState>) {
  player = { ...player, ...patch }
  emit()
}

export function getPlayer() {
  return player
}

export function usePlayer() {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    getPlayer,
    getPlayer,
  )
}

export function currentTrack(state: PlayerState = player) {
  return state.queue[state.index] ?? null
}

export function displayTitle(state: PlayerState = player) {
  const fromTags = state.meta?.title.trim()
  if (fromTags) return fromTags
  return currentTrack(state)?.title ?? ""
}

export function displayArtist(state: PlayerState = player) {
  const fromTags = state.meta?.artist.trim()
  if (fromTags) return fromTags
  return currentTrack(state)?.artist ?? ""
}

export function activeLine(lines: LyricLine[], position: number) {
  let found = -1
  for (let index = 0; index < lines.length; index += 1) {
    if (lines[index].at <= position + 0.05) found = index
    else break
  }
  return found
}

export function orderedTracks(state: PlayerState = player) {
  return state.order.map((index) => state.queue[index]).filter((track): track is Track => Boolean(track))
}

function buildOrder(length: number, index: number, shuffle: boolean) {
  const order = Array.from({ length }, (_, item) => item)
  if (!shuffle || length < 2 || index < 0) return order
  const rest = order.filter((item) => item !== index)
  for (let cursor = rest.length - 1; cursor > 0; cursor -= 1) {
    const swap = Math.floor(Math.random() * (cursor + 1))
    const held = rest[cursor]
    rest[cursor] = rest[swap]
    rest[swap] = held
  }
  return [index, ...rest]
}

function activeElement() {
  if (player.playbackKind === "video") return video
  if (player.playbackKind === "audio") return audio
  return null
}

function applyVolume() {
  const level = player.muted ? 0 : player.volume
  if (audio) audio.volume = level
  if (video) video.volume = level
}

function stop(element: HTMLMediaElement | null) {
  if (!element) return
  element.pause()
  element.removeAttribute("src")
  element.load()
}

export function placeVideo() {
  if (!video || !park) return
  const host = slot && player.playbackKind === "video" && !player.error ? slot : park
  if (video.parentElement !== host) host.appendChild(video)
  video.className = "h-full w-full bg-black object-contain"
}

export function setVideoSlot(element: HTMLElement | null) {
  slot = element
  placeVideo()
}

function stopLoop() {
  if (frame) cancelAnimationFrame(frame)
  frame = 0
}

function startLoop() {
  stopLoop()
  const tick = (now: number) => {
    const element = activeElement()
    if (!element || element.paused) return
    if (now - lastTick > 80) {
      lastTick = now
      const duration = Number.isFinite(element.duration) && element.duration > 0 ? element.duration : player.duration
      setPlayer({ currentTime: element.currentTime, duration, playing: true })
    }
    frame = requestAnimationFrame(tick)
  }
  frame = requestAnimationFrame(tick)
}

function pullTime() {
  const element = activeElement()
  if (!element) return
  const duration = Number.isFinite(element.duration) && element.duration > 0 ? element.duration : player.duration
  setPlayer({ currentTime: element.currentTime || 0, duration })
}

function handleEnded() {
  if (player.repeat === "one") return
  const next = following()
  if (next == null) {
    stopLoop()
    setPlayer({ playing: false })
    return
  }
  void load(next)
}

function handleError(event: Event) {
  const element = event.currentTarget
  if (!(element instanceof HTMLMediaElement)) return
  if (element !== activeElement()) return
  if (!element.getAttribute("src")) return
  stopLoop()
  setPlayer({ playing: false, error: "failed" })
  placeVideo()
}

export function attachMedia(nextAudio: HTMLAudioElement, nextVideo: HTMLVideoElement, nextPark: HTMLElement) {
  audio = nextAudio
  video = nextVideo
  park = nextPark
  applyVolume()
  const onTime = () => pullTime()
  const onEnd = () => handleEnded()
  const onErr = (event: Event) => handleError(event)
  const onMeta = () => pullTime()
  for (const element of [audio, video]) {
    element.addEventListener("timeupdate", onTime)
    element.addEventListener("ended", onEnd)
    element.addEventListener("error", onErr)
    element.addEventListener("loadedmetadata", onMeta)
  }
  placeVideo()
  return () => {
    stopLoop()
    for (const element of [audio, video]) {
      if (!element) continue
      element.removeEventListener("timeupdate", onTime)
      element.removeEventListener("ended", onEnd)
      element.removeEventListener("error", onErr)
      element.removeEventListener("loadedmetadata", onMeta)
    }
  }
}

async function resolveSource(track: Track): Promise<{ src: string; info?: MediaInspect } | { error: "missing" | "failed" }> {
  if (!isTauri()) {
    if (track.path.startsWith("/") || track.path.startsWith("http")) return { src: track.path }
    return { error: "missing" }
  }
  try {
    const info = await invoke<MediaInspect>("inspect_media", { path: track.path })
    if (!info.exists) return { error: "missing" }
    if (!info.src) return { error: "failed" }
    return { src: info.src, info }
  } catch {
    return { error: "failed" }
  }
}

async function loadLyrics(mine: number, track: Track, info?: MediaInspect) {
  setPlayer({ lyricsState: "loading", lyrics: null })
  if (!isTauri()) {
    if (mine !== generation) return
    setPlayer({
      lyrics: track.id === "preview-1" ? sampleLyrics : emptyLyrics(),
      lyricsState: "ready",
    })
    return
  }
  try {
    const lyrics = await invoke<TrackLyrics>("load_lyrics", {
      path: track.path,
      title: info?.title || track.title,
      artist: info?.artist || track.artist,
      duration: info?.duration ?? track.duration ?? null,
    })
    if (mine !== generation) return
    setPlayer({ lyrics, lyricsState: "ready" })
  } catch {
    if (mine !== generation) return
    setPlayer({ lyrics: emptyLyrics(), lyricsState: "ready" })
  }
}

async function load(index: number) {
  const mine = ++generation
  const track = player.queue[index]
  if (!track) return
  stopLoop()
  setPlayer({
    index,
    error: null,
    lyrics: null,
    lyricsState: "idle",
    meta: null,
    currentTime: 0,
    playing: false,
    duration: track.duration ?? 0,
  })
  const source = await resolveSource(track)
  if (mine !== generation) return
  if ("error" in source) {
    stop(audio)
    stop(video)
    setPlayer({
      error: source.error,
      playing: false,
      playbackKind: track.kind,
      lyrics: emptyLyrics(),
      lyricsState: "ready",
    })
    placeVideo()
    return
  }
  const kind: PlaybackKind = source.info?.kind === "video" || source.info?.kind === "audio" ? source.info.kind : track.kind
  const element = kind === "video" ? video : audio
  const other = kind === "video" ? audio : video
  if (!element || !other) {
    setPlayer({ error: "failed", playbackKind: kind })
    return
  }
  if (source.info) {
    setPlayer({
      meta: {
        title: source.info.title,
        artist: source.info.artist,
        album: source.info.album,
        duration: source.info.duration,
      },
    })
  }
  stop(other)
  setPlayer({ playbackKind: kind, error: null })
  element.loop = player.repeat === "one"
  if (element.getAttribute("src") === source.src) element.currentTime = 0
  else element.src = source.src
  applyVolume()
  placeVideo()
  try {
    await element.play()
  } catch {
    if (mine !== generation) return
    setPlayer({ playing: false, error: "failed" })
    placeVideo()
    return
  }
  if (mine !== generation) return
  const duration = Number.isFinite(element.duration) && element.duration > 0 ? element.duration : track.duration ?? 0
  setPlayer({ playing: true, error: null, duration, currentTime: element.currentTime || 0 })
  startLoop()
  void loadLyrics(mine, track, source.info)
}

export function playTracks(queue: Track[], index: number) {
  if (!queue.length) return
  const safe = Math.min(Math.max(index, 0), queue.length - 1)
  const order = buildOrder(queue.length, safe, player.shuffle)
  setPlayer({ queue, order })
  void load(safe)
}

function following() {
  const pos = player.order.indexOf(player.index)
  if (pos < 0) return null
  if (pos + 1 < player.order.length) return player.order[pos + 1]
  if (player.repeat === "all") return player.order[0] ?? null
  return null
}

export function playOrderSlot(slot: number) {
  const index = player.order[slot]
  if (index == null) return
  void load(index)
}

export function playNext() {
  const pos = player.order.indexOf(player.index)
  if (pos < 0) return
  if (pos + 1 < player.order.length) void load(player.order[pos + 1])
  else if (player.repeat === "all" && player.order.length) void load(player.order[0])
}

export function playPrevious() {
  const element = activeElement()
  if (element && element.currentTime > 3) {
    seek(0)
    return
  }
  const pos = player.order.indexOf(player.index)
  if (pos > 0) void load(player.order[pos - 1])
  else if (player.repeat === "all" && player.order.length) void load(player.order[player.order.length - 1])
  else seek(0)
}

export function togglePlayback() {
  const element = activeElement()
  if (!element || !element.getAttribute("src") || player.error) return
  if (element.paused) {
    void element
      .play()
      .then(() => {
        setPlayer({ playing: true, error: null })
        startLoop()
      })
      .catch(() => setPlayer({ playing: false, error: "failed" }))
    return
  }
  element.pause()
  stopLoop()
  setPlayer({ playing: false, currentTime: element.currentTime })
}

export function seek(seconds: number) {
  const element = activeElement()
  const duration = element && Number.isFinite(element.duration) ? element.duration : player.duration
  const next = Math.max(0, duration > 0 ? Math.min(seconds, duration) : seconds)
  if (element && element.getAttribute("src")) element.currentTime = next
  setPlayer({ currentTime: next })
}

export function setVolume(volume: number) {
  const next = Math.min(1, Math.max(0, volume))
  const muted = next === 0
  setPlayer({ volume: next, muted })
  applyVolume()
  persistLevel(next, muted)
}

export function toggleMute() {
  const muted = !player.muted
  setPlayer({ muted })
  applyVolume()
  persistLevel(player.volume, muted)
}

export function toggleShuffle() {
  const shuffle = !player.shuffle
  setPlayer({ shuffle, order: buildOrder(player.queue.length, player.index, shuffle) })
}

export function cycleRepeat() {
  const repeat: RepeatMode = player.repeat === "off" ? "all" : player.repeat === "all" ? "one" : "off"
  setPlayer({ repeat })
  const element = activeElement()
  if (element) element.loop = repeat === "one"
}

export function setPanel(panel: PlayerPanel) {
  setPlayer({ panel })
}
