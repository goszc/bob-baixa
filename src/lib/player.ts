import { useSyncExternalStore } from "react"
import { invoke, isTauri } from "./api"
import { trackKey, type Track } from "./library"
import { clearSleep, flushHear, getMusic, hear, onAudioPref, rememberPosition, rememberSession, resumeFor, touchRecent } from "./music"

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
let audioAlt: HTMLAudioElement | null = null
let video: HTMLVideoElement | null = null
let park: HTMLElement | null = null
let slot: HTMLElement | null = null
let generation = 0
let frame = 0
let lastTick = 0
let slotB = false
let wired = false
let audioCtx: AudioContext | null = null
let gainA: GainNode | null = null
let gainB: GainNode | null = null
let master: GainNode | null = null
let compressor: DynamicsCompressorNode | null = null
let fading = false
let fadeToken = 0
let lastListenAt = 0
let lastTip = ""
let restored = false
let sessionStamp = 0

function emit() {
  listeners.forEach((listener) => listener())
}

function setPlayer(patch: Partial<PlayerState>) {
  player = { ...player, ...patch }
  emit()
  publishSession()
  const track = currentTrack()
  if (track) {
    const now = Date.now()
    if (now - sessionStamp > 4000) {
      sessionStamp = now
      rememberSession({
        paths: player.queue.map((item) => item.path),
        index: player.index,
        position: player.currentTime,
        shuffle: player.shuffle,
        repeat: player.repeat,
      })
    }
  }
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
  if (player.playbackKind === "audio") return slotB ? audioAlt : audio
  return null
}

function idleAudio() {
  return slotB ? audio : audioAlt
}

function applyVolume() {
  const level = player.muted ? 0 : player.volume
  if (video) video.volume = level
  if (!wired || !master || !audioCtx) {
    if (audio) audio.volume = level
    if (audioAlt) audioAlt.volume = level
    return
  }
  if (audio) audio.volume = 1
  if (audioAlt) audioAlt.volume = 1
  const now = audioCtx.currentTime
  master.gain.setValueAtTime(level, now)
  if (!fading && gainA && gainB) {
    const active = slotB ? gainB : gainA
    const idle = slotB ? gainA : gainB
    active.gain.cancelScheduledValues(now)
    idle.gain.cancelScheduledValues(now)
    active.gain.setValueAtTime(1, now)
    idle.gain.setValueAtTime(0, now)
  }
}

function ensureGraph() {
  if (wired || !audio || !audioAlt || typeof AudioContext === "undefined") return
  const ctx = new AudioContext()
  const sourceA = ctx.createMediaElementSource(audio)
  const sourceB = ctx.createMediaElementSource(audioAlt)
  gainA = ctx.createGain()
  gainB = ctx.createGain()
  master = ctx.createGain()
  compressor = ctx.createDynamicsCompressor()
  compressor.threshold.value = -20
  compressor.knee.value = 18
  compressor.ratio.value = 2
  compressor.attack.value = 0.02
  compressor.release.value = 0.25
  sourceA.connect(gainA)
  sourceB.connect(gainB)
  gainA.connect(master)
  gainB.connect(master)
  audioCtx = ctx
  wired = true
  routeMaster()
  applyVolume()
}

function routeMaster() {
  if (!wired || !master || !audioCtx || !compressor) return
  master.disconnect()
  compressor.disconnect()
  if (getMusic().normalize) {
    master.connect(compressor)
    compressor.connect(audioCtx.destination)
  } else {
    master.connect(audioCtx.destination)
  }
}

function publishSession() {
  const track = currentTrack()
  const title = displayTitle()
  const artist = displayArtist()
  if (typeof navigator !== "undefined" && "mediaSession" in navigator) {
    try {
      navigator.mediaSession.metadata = new MediaMetadata({
        title: title || "bob baixa",
        artist,
        album: player.meta?.album || track?.album || "",
      })
      navigator.mediaSession.playbackState = player.playing ? "playing" : "paused"
    } catch {
      /* the webview may not expose media session */
    }
  }
  const tip = `${title}\n${artist}`
  if (tip === lastTip || !isTauri()) return
  lastTip = tip
  void invoke("set_now_playing", { title, artist }).catch(() => undefined)
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
      const music = getMusic()
      if (music.sleepUntil && Date.now() >= music.sleepUntil) {
        element.pause()
        clearSleep()
        stopLoop()
        setPlayer({ playing: false, currentTime: element.currentTime, duration })
        return
      }
      const track = currentTrack()
      if (track && player.playbackKind === "audio") {
        const delta = lastListenAt ? now - lastListenAt : 0
        lastListenAt = now
        hear(track.path, displayTitle(), displayArtist(), delta, Date.now())
        if (element.currentTime > 5) rememberPosition(track.path, element.currentTime)
      }
      maybeCrossfade(element)
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

function handleEnded(event?: Event) {
  if (event?.currentTarget && event.currentTarget !== activeElement()) return
  if (fading) return
  if (getMusic().sleepMode === "end") {
    clearSleep()
    stopLoop()
    setPlayer({ playing: false })
    return
  }
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

export function attachMedia(
  nextAudio: HTMLAudioElement,
  nextAlt: HTMLAudioElement,
  nextVideo: HTMLVideoElement,
  nextPark: HTMLElement,
) {
  audio = nextAudio
  audioAlt = nextAlt
  video = nextVideo
  park = nextPark
  onAudioPref(() => {
    ensureGraph()
    routeMaster()
    applyVolume()
  })
  if (typeof navigator !== "undefined" && "mediaSession" in navigator) {
    const pairs: [MediaSessionAction, () => void][] = [
      ["play", () => togglePlayback()],
      ["pause", () => togglePlayback()],
      ["previoustrack", () => playPrevious()],
      ["nexttrack", () => playNext()],
    ]
    for (const [action, run] of pairs) {
      try {
        navigator.mediaSession.setActionHandler(action, run)
      } catch {
        /* unsupported action */
      }
    }
  }
  applyVolume()
  const onTime = () => pullTime()
  const onEnd = (event: Event) => handleEnded(event)
  const onErr = (event: Event) => handleError(event)
  const onMeta = () => pullTime()
  for (const element of [audio, video]) {
    element.addEventListener("timeupdate", onTime)
    element.addEventListener("ended", onEnd)
    element.addEventListener("error", onErr)
    element.addEventListener("loadedmetadata", onMeta)
  }
  if (audioAlt) {
    audioAlt.addEventListener("ended", onEnd)
    audioAlt.addEventListener("error", onErr)
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
    audioAlt?.removeEventListener("ended", onEnd)
    audioAlt?.removeEventListener("error", onErr)
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

function waitForMeta(element: HTMLMediaElement) {
  if (element.readyState >= 1) return Promise.resolve()
  return new Promise<void>((resolve) => {
    const done = () => {
      element.removeEventListener("loadedmetadata", done)
      resolve()
    }
    element.addEventListener("loadedmetadata", done)
    window.setTimeout(done, 1500)
  })
}

async function load(index: number, autoplay = true) {
  const mine = ++generation
  fadeToken += 1
  fading = false
  flushHear(Date.now())
  lastListenAt = 0
  const track = player.queue[index]
  if (!track) return
  stopLoop()
  slotB = false
  stop(audioAlt)
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
    stop(audioAlt)
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
  if (kind === "audio" && (getMusic().crossfade > 0 || getMusic().normalize)) ensureGraph()
  void audioCtx?.resume()
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
  element.src = source.src
  applyVolume()
  placeVideo()
  await waitForMeta(element)
  if (mine !== generation) return
  const resumeAt = resumeFor(track.path)
  if (resumeAt > 5 && Number.isFinite(element.duration) && element.duration - resumeAt > 8) element.currentTime = resumeAt
  const duration = Number.isFinite(element.duration) && element.duration > 0 ? element.duration : track.duration ?? 0
  if (!autoplay) {
    setPlayer({ playing: false, error: null, duration, currentTime: element.currentTime || 0 })
    void loadLyrics(mine, track, source.info)
    return
  }
  try {
    await element.play()
  } catch {
    if (mine !== generation) return
    setPlayer({ playing: false, error: "failed" })
    placeVideo()
    return
  }
  if (mine !== generation) return
  touchRecent(track.path)
  setPlayer({ playing: true, error: null, duration, currentTime: element.currentTime || 0 })
  startLoop()
  void loadLyrics(mine, track, source.info)
}

function maybeCrossfade(element: HTMLMediaElement) {
  const seconds = getMusic().crossfade
  if (fading || seconds <= 0 || player.repeat === "one" || player.playbackKind !== "audio") return
  if (!Number.isFinite(element.duration) || element.duration < seconds + 1) return
  const left = element.duration - element.currentTime
  if (left > seconds || left < 0.15) return
  const next = following()
  if (next == null) return
  const track = player.queue[next]
  if (!track || track.kind === "video") return
  void beginCrossfade(next, seconds)
}

async function beginCrossfade(nextIndex: number, seconds: number) {
  if (fading) return
  const track = player.queue[nextIndex]
  if (!track || !audio || !audioAlt) return
  ensureGraph()
  if (!audioCtx || !gainA || !gainB) return
  const token = ++fadeToken
  fading = true
  const source = await resolveSource(track)
  if (token !== fadeToken) return
  if ("error" in source) {
    fading = false
    return
  }
  const incoming = idleAudio()
  if (!incoming) {
    fading = false
    return
  }
  const inGain = incoming === audioAlt ? gainB : gainA
  const outGain = incoming === audioAlt ? gainA : gainB
  incoming.src = source.src
  incoming.loop = false
  try {
    await incoming.play()
  } catch {
    if (token === fadeToken) fading = false
    return
  }
  if (token !== fadeToken) return
  void audioCtx.resume()
  const now = audioCtx.currentTime
  outGain.gain.cancelScheduledValues(now)
  inGain.gain.cancelScheduledValues(now)
  outGain.gain.setValueAtTime(1, now)
  inGain.gain.setValueAtTime(0, now)
  outGain.gain.linearRampToValueAtTime(0, now + seconds)
  inGain.gain.linearRampToValueAtTime(1, now + seconds)
  slotB = incoming === audioAlt
  setPlayer({
    index: nextIndex,
    error: null,
    playbackKind: "audio",
    meta: source.info
      ? { title: source.info.title, artist: source.info.artist, album: source.info.album, duration: source.info.duration }
      : player.meta,
    duration: track.duration ?? 0,
    currentTime: 0,
  })
  touchRecent(track.path)
  void loadLyrics(++generation, track, source.info)
  window.setTimeout(() => {
    if (token !== fadeToken) return
    stop(incoming === audioAlt ? audio : audioAlt)
    fading = false
    applyVolume()
  }, seconds * 1000 + 40)
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
  lastListenAt = 0
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

export function queueTracks(tracks: Track[], where: "next" | "end") {
  const fresh = tracks.filter((track) => track.path)
  if (!fresh.length) return
  if (!player.queue.length) {
    playTracks(fresh, 0)
    return
  }
  const queue = [...player.queue, ...fresh]
  const added = fresh.map((_, index) => player.queue.length + index)
  const order = player.order.slice()
  const at = where === "end" ? order.length : Math.max(0, order.indexOf(player.index) + 1)
  order.splice(at, 0, ...added)
  setPlayer({ queue, order })
}

export function clearPlayback() {
  fadeToken += 1
  fading = false
  flushHear(Date.now())
  stopLoop()
  stop(audio)
  stop(audioAlt)
  stop(video)
  setPlayer({
    queue: [],
    order: [],
    index: -1,
    playing: false,
    currentTime: 0,
    duration: 0,
    error: null,
    meta: null,
    lyrics: null,
    lyricsState: "idle",
    playbackKind: null,
  })
  rememberSession(null)
}

export function forgetTracks(paths: string[]) {
  const gone = new Set(paths.map((path) => trackKey(path)))
  if (!player.queue.some((track) => gone.has(trackKey(track.path)))) return
  const current = currentTrack()
  const currentGone = Boolean(current && gone.has(trackKey(current.path)))
  const remap = new Map<number, number>()
  const queue: Track[] = []
  player.queue.forEach((track, index) => {
    if (gone.has(trackKey(track.path))) return
    remap.set(index, queue.length)
    queue.push(track)
  })
  const order = player.order.flatMap((index) => {
    const next = remap.get(index)
    return next == null ? [] : [next]
  })
  if (!queue.length) {
    clearPlayback()
    return
  }
  if (currentGone) {
    const index = order[0] ?? 0
    setPlayer({ queue, order, index })
    void load(index)
    return
  }
  setPlayer({ queue, order, index: remap.get(player.index) ?? 0 })
}

export function syncQueue(library: Track[]) {
  if (!player.queue.length) return
  const allowed = new Set(library.map((track) => trackKey(track.path)))
  const missing = player.queue.filter((track) => !allowed.has(trackKey(track.path))).map((track) => track.path)
  if (missing.length) forgetTracks(missing)
}

export function restoreSession(library: Track[], ready: boolean) {
  if (restored || player.queue.length) {
    restored = true
    return
  }
  if (!ready) return
  restored = true
  const session = getMusic().session
  if (!session?.paths.length) return
  const queue = session.paths
    .map((path) => library.find((track) => trackKey(track.path) === trackKey(path)))
    .filter((track): track is Track => Boolean(track))
  if (!queue.length) return
  const index = Math.min(Math.max(session.index, 0), queue.length - 1)
  setPlayer({
    queue,
    order: buildOrder(queue.length, index, session.shuffle),
    index,
    shuffle: session.shuffle,
    repeat: session.repeat,
    currentTime: session.position,
    playing: false,
  })
  void load(index, false)
}
