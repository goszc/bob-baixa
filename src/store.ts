import { useSyncExternalStore } from "react"
import { applyWindowTheme, chooseFile, chooseFolder, errorText, invoke, isTauri, listenBob, readTextFile, revealPath } from "./lib/api"
import { markRemoved, refreshCatalog, unmarkRemoved } from "./lib/catalog"
import { extractUrls, isMediaPath, isTextPath, isTime, nextIsoFromClock } from "./lib/format"
import { trackKey, trackKind, type Track } from "./lib/library"
import { queueTracks } from "./lib/player"
import { forgetTracks } from "./lib/player"
import { presetFromConfig, presetPatch } from "./lib/presets"
import type { DownloadPreset } from "./lib/types"
import { translate, type Key } from "./lib/i18n"
import { cachedThumbs, previewAnalyzed, previewHistory, previewHits, previewJobs, previewWatches } from "./lib/preview"
import {
  defaultConfig,
  type Config,
  type ConvertJob,
  type ConvertPreset,
  type EnqueueItem,
  type HistoryEntry,
  type Job,
  type MediaItem,
  type Page,
  type SearchHit,
  type ThemeMode,
  type ToolStatus,
  type Watch,
} from "./lib/types"

export interface ConfirmState {
  title: string
  body: string
  confirm: string
  cancel: string
  run: () => void
}

export interface AppState {
  page: Page
  config: Config
  draft: string
  clipStart: string
  clipEnd: string
  itemSpeed: string
  analyzed: MediaItem[]
  formatChoice: Record<string, string>
  entryChoice: Record<string, string[]>
  jobs: Job[]
  history: HistoryEntry[]
  watches: Watch[]
  tools: ToolStatus
  searchQuery: string
  searchHits: SearchHit[]
  searchFormat: "mp3" | "mp4" | "economy"
  convertInput: string
  convertPreset: ConvertPreset
  convertBitrate: string
  convertJob: ConvertJob | null
  watchUrl: string
  watchEvery: number
  scheduleClock: string
  scheduleAt: string | null
  busy: null | "analyze" | "search" | "convert" | "tools"
  error: string | null
  toast: string | null
  clipboardUrl: string | null
  confirm: ConfirmState | null
  expandedJob: string | null
  dragging: boolean
  updateOffer: boolean
  updateVersion: string
  whatsNew: boolean
  showNotes: boolean
  releaseNotes: string
}

const previewMode =
  typeof location !== "undefined" && new URLSearchParams(location.search).get("preview") === "1"

let analyzedKey = ""
let suppressKey = ""
let analyzeToken = 0

const emptyTools: ToolStatus = { ytdlp: "missing", ffmpeg: "missing" }

function emptyState(): AppState {
  return {
    page: "home",
    config: defaultConfig,
    draft: "",
    clipStart: "",
    clipEnd: "",
    itemSpeed: "",
    analyzed: [],
    formatChoice: {},
    entryChoice: {},
    jobs: [],
    history: [],
    watches: [],
    tools: emptyTools,
    searchQuery: "",
    searchHits: [],
    searchFormat: "mp4",
    convertInput: "",
    convertPreset: "mp3",
    convertBitrate: "192k",
    convertJob: null,
    watchUrl: "",
    watchEvery: 30,
    scheduleClock: "",
    scheduleAt: null,
    busy: null,
    error: null,
    toast: null,
    clipboardUrl: null,
    confirm: null,
    expandedJob: null,
    dragging: false,
    updateOffer: false,
    updateVersion: "",
    whatsNew: false,
    showNotes: false,
    releaseNotes: "",
  }
}

function createInitial(): AppState {
  const base = emptyState()
  if (!previewMode) return base
  const next: AppState = {
    ...base,
    draft: "https://www.youtube.com/watch?v=YF4wa9gB0cA",
    analyzed: previewAnalyzed,
    jobs: previewJobs,
    history: previewHistory,
    watches: previewWatches,
    searchQuery: "lisboa",
    searchHits: previewHits,
    tools: { ytdlp: "ready", ffmpeg: "ready", ytdlpVersion: "2026.10.14", ffmpegVersion: "7.1" },
    config: {
      ...base.config,
      outputDir: "C:\\Downloads\\bob baixa",
      cookiesBrowser: "auto",
    },
    scheduleClock: "22:30",
  }
  analyzedKey = extractUrls(next.draft).join("\n")
  return next
}

let state = createInitial()
const listeners = new Set<() => void>()
const seenClipboard = new Set<string>()
const mockTimers = new Map<string, number>()
let toastTimer = 0
let booted = false
let hydrated = !isTauri()
let pendingPatch: Partial<Config> | null = null

function emit() {
  listeners.forEach((listener) => listener())
}

function setState(patch: Partial<AppState> | ((current: AppState) => AppState)) {
  state = typeof patch === "function" ? patch(state) : { ...state, ...patch }
  emit()
}

export function getState() {
  return state
}

export function useApp() {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    getState,
    getState,
  )
}

export function useT() {
  const lang = useApp().config.language
  return (key: Key, vars?: Record<string, string | number>) => translate(lang, key, vars)
}

export function resolveTheme(theme: ThemeMode, glassFamily: boolean) {
  const systemDark = window.matchMedia("(prefers-color-scheme: dark)").matches
  const dark = theme === "dark" || theme === "glass-dark" || (theme === "system" && systemDark)
  const glass = theme === "glass-light" || theme === "glass-dark" || (theme === "system" && glassFamily)
  return { dark, glass }
}

export function applyTheme(theme: ThemeMode, glassFamily = getState().config.themeGlass) {
  const { dark, glass } = resolveTheme(theme, glassFamily)
  document.documentElement.classList.toggle("dark", dark)
  document.documentElement.classList.toggle("glass", glass)
  document.documentElement.style.colorScheme = dark ? "dark" : "light"
  localStorage.setItem("bob-baixa-theme", theme)
  localStorage.setItem("bob-baixa-glass", glassFamily ? "1" : "0")
  void applyWindowTheme(theme, glassFamily)
}

function lang() {
  return getState().config.language
}

function say(key: Key, vars?: Record<string, string | number>) {
  setState({ toast: translate(lang(), key, vars) })
  window.clearTimeout(toastTimer)
  toastTimer = window.setTimeout(() => setState({ toast: null }), 2400)
}

if (typeof window !== "undefined") {
  window.addEventListener("bob-toast", (event) => {
    const message = (event as CustomEvent<string>).detail
    if (typeof message !== "string" || !message.trim()) return
    setState({ toast: message })
    window.clearTimeout(toastTimer)
    toastTimer = window.setTimeout(() => setState({ toast: null }), 3200)
  })
}

function nid() {
  return `id-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`
}

function clipError(): string | null {
  const { clipStart, clipEnd } = getState()
  const start = clipStart.trim()
  const end = clipEnd.trim()
  if (!start && !end) return null
  if (!start || !end) return translate(lang(), "need_both")
  if (!isTime(start) || !isTime(end)) return translate(lang(), "invalid_time")
  return null
}

function withClip(item: EnqueueItem): EnqueueItem {
  const current = getState()
  return {
    ...item,
    sectionStart: current.clipStart.trim() || undefined,
    sectionEnd: current.clipEnd.trim() || undefined,
    speedLimit: current.itemSpeed.trim() || undefined,
  }
}

function spotifyItem(
  media: { url: string; channel?: string; title: string },
  entry: { url: string; title: string; thumbnail?: string; duration?: number; artist?: string; album?: string; track?: number; cover?: string },
): EnqueueItem {
  const spotify = media.url.includes("open.spotify.com") || media.channel === "spotify"
  return {
    url: entry.url,
    title: entry.title,
    thumbnail: entry.thumbnail,
    duration: entry.duration,
    playlistTitle: media.title,
    audioOnly: spotify ? true : undefined,
    audioFormat: spotify ? "opus" : undefined,
    audioBitrate: spotify ? "24k" : undefined,
    tagArtist: entry.artist,
    tagAlbum: entry.album,
    tagTrack: entry.track,
    tagCover: entry.cover || entry.thumbnail,
  }
}

function itemsFromAnalyzed(): EnqueueItem[] {
  const current = getState()
  const items: EnqueueItem[] = []
  for (const media of current.analyzed) {
    if (media.kind === "playlist") {
      const picked = new Set(current.entryChoice[media.id] ?? media.entries.map((entry) => entry.id))
      for (const entry of media.entries) {
        if (!picked.has(entry.id)) continue
        items.push(withClip(spotifyItem(media, entry)))
      }
      continue
    }
    const choice = current.formatChoice[media.id]
    items.push(
      withClip({
        url: media.url,
        title: media.title,
        thumbnail: media.thumbnail,
        duration: media.duration,
        formatId: choice && choice !== "auto" ? choice : undefined,
      }),
    )
  }
  return items
}

function mockJob(item: EnqueueItem, index: number): Job {
  const config = getState().config
  const scheduledAt = getState().scheduleAt ?? undefined
  const waiting = scheduledAt ? new Date(scheduledAt).getTime() > Date.now() : false
  return {
    id: nid(),
    url: item.url,
    title: item.title || item.url,
    thumbnail: item.thumbnail || cachedThumbs[index % cachedThumbs.length],
    duration: item.duration,
    state: waiting ? "scheduled" : "queued",
    progress: 0,
    createdAt: new Date().toISOString(),
    scheduledAt: waiting ? scheduledAt : undefined,
    sectionStart: item.sectionStart,
    sectionEnd: item.sectionEnd,
    speedLimit: item.speedLimit || config.globalSpeed || undefined,
    audioOnly: item.audioOnly ?? config.audioOnly,
    audioFormat: item.audioFormat ?? config.audioFormat,
    quality: config.quality,
    container: item.container ?? config.container,
  }
}

async function mockAnalyze(urls: string[]): Promise<MediaItem[]> {
  await new Promise((resolve) => window.setTimeout(resolve, 450))
  return urls.map((url, index) => {
    if (url.includes("YF4wa9gB0cA")) return previewAnalyzed[0]
    if (/playlist|channel|\/@|list=/i.test(url)) {
      const entries = previewHits.map((hit) => ({
        id: hit.id,
        url: hit.url,
        title: hit.title,
        duration: hit.duration,
        thumbnail: hit.thumbnail,
      }))
      return {
        kind: "playlist" as const,
        id: `pl-${index}`,
        url,
        title: "lisboa",
        channel: "youtube",
        thumbnail: cachedThumbs[0],
        formats: [],
        entries,
      }
    }
    const hit = previewHits[index % previewHits.length]
    return {
      kind: "video" as const,
      id: hit.id,
      url,
      title: hit.title,
      channel: hit.channel,
      duration: hit.duration,
      thumbnail: hit.thumbnail,
      formats: previewAnalyzed[0].formats,
      entries: [],
    }
  })
}

function mockSearch(query: string): SearchHit[] {
  const needle = query.toLocaleLowerCase()
  const hits = previewHits.filter((hit) => `${hit.title} ${hit.channel ?? ""}`.toLocaleLowerCase().includes(needle))
  return hits.length ? hits : previewHits
}

function pushHistory(job: Job, ok: boolean) {
  const entry: HistoryEntry = {
    id: nid(),
    title: job.title,
    url: job.url,
    path: ok ? `${getState().config.outputDir || "Downloads\\bob baixa"}\\${job.title}.mp4` : undefined,
    at: new Date().toISOString(),
    ok,
    detail: ok ? undefined : job.error,
  }
  setState((current) => ({ ...current, history: [entry, ...current.history] }))
}

function runMock(id: string) {
  window.clearInterval(mockTimers.get(id))
  const timer = window.setInterval(() => {
    const job = getState().jobs.find((item) => item.id === id)
    if (!job || job.state !== "downloading") {
      window.clearInterval(timer)
      mockTimers.delete(id)
      return
    }
    const progress = Math.min(1, job.progress + 0.08)
    const done = progress >= 1
    const next: Job = {
      ...job,
      progress,
      state: done ? "done" : "downloading",
      speed: 4.2 * 1024 * 1024,
      eta: done ? 0 : Math.round((1 - progress) * 48),
      downloaded: progress * 80 * 1024 * 1024,
      total: 80 * 1024 * 1024,
    }
    setState((current) => ({ ...current, jobs: current.jobs.map((item) => (item.id === id ? next : item)) }))
    if (done) {
      window.clearInterval(timer)
      mockTimers.delete(id)
      pushHistory(next, true)
      say("done_toast")
      pumpMock()
    }
  }, 450)
  mockTimers.set(id, timer)
}

function pumpMock() {
  if (isTauri()) return
  const parallel = Math.min(4, Math.max(1, getState().config.parallel || 2))
  const jobs = getState().jobs
  let slots = parallel - jobs.filter((job) => job.state === "downloading").length
  if (slots <= 0) return
  const starting: string[] = []
  for (let index = jobs.length - 1; index >= 0 && slots > 0; index -= 1) {
    const job = jobs[index]
    if (job.state === "queued") {
      starting.push(job.id)
      slots -= 1
    }
  }
  if (!starting.length) return
  const chosen = new Set(starting)
  setState((current) => ({
    ...current,
    jobs: current.jobs.map((job) => (chosen.has(job.id) && job.state === "queued" ? { ...job, state: "downloading" } : job)),
  }))
  for (const id of starting) runMock(id)
}

export async function enqueueItems(items: EnqueueItem[]) {
  if (!items.length) return
  if (isTauri()) {
    await invoke("enqueue", { items })
    say(items.length > 1 ? "added_n" : "added", { n: items.length })
    return
  }
  const jobs = items.map(mockJob)
  setState((current) => ({ ...current, jobs: [...jobs, ...current.jobs] }))
  say(items.length > 1 ? "added_n" : "added", { n: items.length })
  pumpMock()
}

async function considerEnqueue(items: EnqueueItem[]) {
  if (items.length > 40) {
    setState({
      confirm: {
        title: translate(lang(), "confirm_many", { n: items.length }),
        body: translate(lang(), "confirm_many_body"),
        confirm: translate(lang(), "confirm"),
        cancel: translate(lang(), "cancel"),
        run: () => {
          void enqueueItems(items)
        },
      },
    })
    return
  }
  await enqueueItems(items)
}

function keepLatestPlaylist(items: MediaItem[]) {
  let last = -1
  items.forEach((item, index) => {
    if (item.kind === "playlist") last = index
  })
  if (last < 0) return items
  return items.filter((item, index) => item.kind !== "playlist" || index === last)
}

function choicesFor(items: MediaItem[]) {
  const entryChoice: Record<string, string[]> = {}
  for (const item of items) {
    if (item.kind === "playlist") entryChoice[item.id] = item.entries.map((entry) => entry.id)
  }
  return entryChoice
}

function withoutUrl(draft: string, url: string) {
  return draft
    .split(/\r?\n/)
    .map((line) => line.replace(url, "").trim())
    .filter((line) => line.length > 0)
    .join("\n")
}

export async function analyze() {
  const urls = extractUrls(getState().draft)
  const key = urls.join("\n")
  if (!urls.length) {
    analyzeToken += 1
    analyzedKey = ""
    suppressKey = ""
    setState({ analyzed: [], busy: null, entryChoice: {} })
    return
  }
  if (key === suppressKey) return
  if (key === analyzedKey && getState().analyzed.length > 0) return
  const token = ++analyzeToken
  setState({ busy: "analyze", error: null, analyzed: [] })
  try {
    const raw = isTauri() ? await invoke<MediaItem[]>("analyze", { urls }) : await mockAnalyze(urls)
    if (token !== analyzeToken) return
    const analyzed = keepLatestPlaylist(raw)
    analyzedKey = key
    suppressKey = ""
    setState({ analyzed, busy: null, entryChoice: choicesFor(analyzed) })
  } catch (error) {
    if (token !== analyzeToken) return
    setState({ busy: null, error: errorText(error) })
  }
}

export function dismissAnalyzed(id: string) {
  const current = getState()
  const media = current.analyzed.find((item) => item.id === id)
  const analyzed = current.analyzed.filter((item) => item.id !== id)
  const draft = media?.url ? withoutUrl(current.draft, media.url) : current.draft
  const key = extractUrls(draft).join("\n")
  analyzedKey = key
  suppressKey = analyzed.length ? "" : key
  analyzeToken += 1
  const entryChoice = { ...current.entryChoice }
  delete entryChoice[id]
  setState({ ...current, analyzed, draft, entryChoice, busy: null })
}

export function dismissAllAnalyzed() {
  const key = extractUrls(getState().draft).join("\n")
  suppressKey = key
  analyzedKey = key
  analyzeToken += 1
  setState({ analyzed: [], entryChoice: {}, busy: null })
}

export async function downloadNow() {
  const invalid = clipError()
  if (invalid) {
    setState({ error: invalid })
    return
  }
  const urls = extractUrls(getState().draft)
  if (!urls.length) {
    setState({ error: translate(lang(), "invalid_url") })
    return
  }
  if (analyzedKey !== urls.join("\n") || getState().analyzed.length === 0) await analyze()
  if (getState().error || getState().busy) return
  await considerEnqueue(itemsFromAnalyzed())
}

export async function addAnalyzed(id: string) {
  const media = getState().analyzed.find((item) => item.id === id)
  if (!media) return
  if (media.kind === "video") {
    const choice = getState().formatChoice[id]
    await considerEnqueue([
      withClip({
        url: media.url,
        title: media.title,
        thumbnail: media.thumbnail,
        duration: media.duration,
        formatId: choice && choice !== "auto" ? choice : undefined,
      }),
    ])
    return
  }
  const picked = new Set(getState().entryChoice[id] ?? [])
  const items = media.entries
    .filter((entry) => picked.has(entry.id))
    .map((entry) => withClip(spotifyItem(media, entry)))
  await considerEnqueue(items)
}

export function toggleEntry(mediaId: string, entryId: string) {
  setState((current) => {
    const all = current.analyzed.find((item) => item.id === mediaId)?.entries.map((entry) => entry.id) ?? []
    const selected = new Set(current.entryChoice[mediaId] ?? all)
    if (selected.has(entryId)) selected.delete(entryId)
    else selected.add(entryId)
    return { ...current, entryChoice: { ...current.entryChoice, [mediaId]: [...selected] } }
  })
}

export function toggleAllEntries(mediaId: string, on: boolean) {
  const all = getState().analyzed.find((item) => item.id === mediaId)?.entries.map((entry) => entry.id) ?? []
  setState((current) => ({
    ...current,
    entryChoice: { ...current.entryChoice, [mediaId]: on ? all : [] },
  }))
}

export function setFormat(mediaId: string, formatId: string) {
  setState((current) => ({ ...current, formatChoice: { ...current.formatChoice, [mediaId]: formatId } }))
}

export async function jobAction(id: string, action: "pause" | "resume" | "cancel" | "retry" | "remove") {
  if (isTauri()) {
    await invoke("job_action", { id, action })
    return
  }
  if (action === "remove") {
    window.clearInterval(mockTimers.get(id))
    setState((current) => ({ ...current, jobs: current.jobs.filter((job) => job.id !== id) }))
    pumpMock()
    return
  }
  setState((current) => ({
    ...current,
    jobs: current.jobs.map((job) => {
      if (job.id !== id) return job
      if (action === "pause") return { ...job, state: "paused" }
      if (action === "cancel") return { ...job, state: "cancelled" }
      if (action === "resume") return { ...job, state: "queued", error: undefined }
      if (action === "retry") return { ...job, state: "queued", error: undefined, progress: 0, speed: undefined, eta: undefined }
      return job
    }),
  }))
  if (action === "pause" || action === "cancel") window.clearInterval(mockTimers.get(id))
  pumpMock()
}

export async function scheduleQueue() {
  const at = nextIsoFromClock(getState().scheduleClock)
  if (!at) return
  if (isTauri()) await invoke("schedule_queue", { at })
  setState((current) => ({
    ...current,
    scheduleAt: at,
    jobs: current.jobs.map((job) => (job.state === "queued" ? { ...job, state: "scheduled", scheduledAt: at } : job)),
  }))
}

export async function clearSchedule() {
  if (isTauri()) await invoke("clear_schedule")
  setState((current) => ({
    ...current,
    scheduleAt: null,
    jobs: current.jobs.map((job) => (job.state === "scheduled" ? { ...job, state: "queued", scheduledAt: undefined } : job)),
  }))
  if (!isTauri()) pumpMock()
}

export async function runSearch() {
  const query = getState().searchQuery.trim()
  if (!query) return
  setState({ busy: "search", error: null })
  try {
    const searchHits = isTauri() ? await invoke<SearchHit[]>("search_youtube", { query }) : mockSearch(query)
    setState({ searchHits, busy: null })
  } catch (error) {
    setState({ busy: null, error: errorText(error) })
  }
}

export async function enqueueHit(hit: SearchHit) {
  const format = getState().searchFormat
  const economy = format === "economy"
  await enqueueItems([
    {
      url: hit.url,
      title: hit.title,
      thumbnail: hit.thumbnail,
      duration: hit.duration,
      audioOnly: format === "mp3" || economy,
      audioFormat: economy ? "opus" : format === "mp3" ? "mp3" : undefined,
      audioBitrate: economy ? "24k" : undefined,
      container: format === "mp4" ? "mp4" : undefined,
    },
  ])
}

export function requestDeleteTrack(path: string, title: string) {
  setState({
    confirm: {
      title: translate(lang(), "delete_track_title"),
      body: translate(lang(), "delete_track_body", { title }),
      confirm: translate(lang(), "delete_track"),
      cancel: translate(lang(), "cancel"),
      run: () => {
        void performDelete(path)
      },
    },
  })
}

async function performDelete(path: string) {
  const previous = getState()
  markRemoved(path)
  forgetTracks([path])
  setState((current) => ({
    ...current,
    history: current.history.filter((entry) => !entry.path || trackKey(entry.path) !== trackKey(path)),
    jobs: current.jobs.map((job) => (job.outputPath && trackKey(job.outputPath) === trackKey(path) ? { ...job, outputPath: undefined } : job)),
  }))
  if (!isTauri()) return
  try {
    await invoke("delete_library_track", { path })
    await refreshCatalog(getState().config.outputDir, () => getState().config.outputDir)
  } catch (error) {
    unmarkRemoved(path)
    setState({ history: previous.history, jobs: previous.jobs, error: errorText(error) })
  }
}

export async function startConvert() {
  const current = getState()
  if (!current.convertInput) return
  if (isTauri()) {
    setState({ busy: "convert", error: null })
    try {
      await invoke("convert", {
        input: current.convertInput,
        preset: current.convertPreset,
        bitrate: current.convertBitrate,
      })
      setState({ busy: null })
    } catch (error) {
      setState({ busy: null, error: errorText(error) })
    }
    return
  }
  const job: ConvertJob = {
    id: nid(),
    input: current.convertInput,
    preset: current.convertPreset,
    state: "running",
    progress: 0,
  }
  setState({ convertJob: job, error: null })
  const timer = window.setInterval(() => {
    const running = getState().convertJob
    if (!running || running.id !== job.id || running.state !== "running") {
      window.clearInterval(timer)
      return
    }
    const progress = Math.min(1, running.progress + 0.12)
    const done = progress >= 1
    setState({
      convertJob: {
        ...running,
        progress,
        state: done ? "done" : "running",
        output: done ? running.input.replace(/\.\w+$/, "") + "." + (running.preset === "mp4" ? "mp4" : running.preset) : undefined,
      },
    })
    if (done) {
      window.clearInterval(timer)
      say("convert_done")
    }
  }, 350)
}

export function cancelConvert() {
  if (isTauri()) void invoke("cancel_convert")
  setState((current) => ({
    ...current,
    convertJob: current.convertJob ? { ...current.convertJob, state: "cancelled" } : null,
  }))
}

export async function addWatch() {
  const url = extractUrls(getState().watchUrl)[0]
  if (!url) {
    setState({ error: translate(lang(), "invalid_url") })
    return
  }
  if (isTauri()) {
    try {
      await invoke("add_watch", { url, everyMinutes: getState().watchEvery })
      setState({ watchUrl: "", error: null })
      say("watch_added")
    } catch (error) {
      setState({ error: errorText(error) })
    }
    return
  }
  const watch: Watch = {
    id: nid(),
    url,
    title: url,
    everyMinutes: getState().watchEvery,
    enabled: true,
    known: 0,
  }
  setState((current) => ({ ...current, watches: [watch, ...current.watches], watchUrl: "", error: null }))
  say("watch_added")
}

export async function watchAction(id: string, action: "remove" | "check" | "toggle") {
  if (isTauri()) {
    await invoke("watch_action", { id, action })
    return
  }
  if (action === "remove") {
    setState((current) => ({ ...current, watches: current.watches.filter((watch) => watch.id !== id) }))
    return
  }
  setState((current) => ({
    ...current,
    watches: current.watches.map((watch) => {
      if (watch.id !== id) return watch
      if (action === "toggle") return { ...watch, enabled: !watch.enabled }
      return { ...watch, lastChecked: new Date().toISOString(), known: watch.known + 1 }
    }),
  }))
}

export async function clearHistory() {
  if (isTauri()) await invoke("clear_history")
  setState({ history: [], confirm: null })
}

export function askClearHistory() {
  setState({
    confirm: {
      title: translate(lang(), "clear_ask"),
      body: translate(lang(), "clear_body"),
      confirm: translate(lang(), "clear_history"),
      cancel: translate(lang(), "cancel"),
      run: () => {
        void clearHistory()
      },
    },
  })
}

export function closeConfirm() {
  setState({ confirm: null })
}

export function setConfig(patch: Partial<Config>) {
  const previous = getState().config
  let themeGlass = patch.themeGlass ?? previous.themeGlass
  if (patch.theme === "light" || patch.theme === "dark") themeGlass = false
  if (patch.theme === "glass-light" || patch.theme === "glass-dark") themeGlass = true
  const config = { ...previous, ...patch, themeGlass }
  setState({ config })
  if (patch.theme || patch.themeGlass !== undefined) applyTheme(config.theme, config.themeGlass)
  if (patch.language) localStorage.setItem("bob-baixa-lang", patch.language)
  if (patch.outputDir !== undefined && trackKey(config.outputDir) !== trackKey(previous.outputDir)) {
    void refreshCatalog(config.outputDir, () => getState().config.outputDir)
  }
  if (!isTauri()) {
    if (patch.parallel !== undefined) pumpMock()
    return
  }
  if (!hydrated) {
    pendingPatch = { ...(pendingPatch ?? {}), ...patch }
    return
  }
  void persistConfig(config, previous)
}

async function persistConfig(config: Config, previous: Config) {
  try {
    await invoke("set_config", { config })
  } catch (error) {
    setState({ config: previous, error: errorText(error) })
    applyTheme(previous.theme, previous.themeGlass)
    if (trackKey(previous.outputDir) !== trackKey(config.outputDir)) {
      void refreshCatalog(previous.outputDir, () => getState().config.outputDir)
    }
  }
}

export function applyPreset(preset: DownloadPreset) {
  setConfig(presetPatch(preset))
}

export function savePreset(name: string) {
  const trimmed = name.trim().slice(0, 40)
  if (!trimmed) {
    setState({ error: translate(lang(), "preset_need_name") })
    return
  }
  const config = getState().config
  if (config.presets.length >= 24) return
  const preset = presetFromConfig(config, nid(), trimmed)
  setState({ error: null })
  setConfig({ presets: [preset, ...config.presets] })
}

export function updatePreset(id: string) {
  const config = getState().config
  const current = config.presets.find((preset) => preset.id === id)
  if (!current) return
  setConfig({
    presets: config.presets.map((preset) => (preset.id === id ? presetFromConfig(config, preset.id, preset.name) : preset)),
  })
}

export function renamePreset(id: string, name: string) {
  const trimmed = name.trim().slice(0, 40)
  if (!trimmed) {
    setState({ error: translate(lang(), "preset_need_name") })
    return
  }
  setState({ error: null })
  setConfig({
    presets: getState().config.presets.map((preset) => (preset.id === id ? { ...preset, name: trimmed } : preset)),
  })
}

export function deletePreset(id: string) {
  setConfig({ presets: getState().config.presets.filter((preset) => preset.id !== id) })
}

export function openNotes() {
  setState({ showNotes: true })
}

export async function dismissNotes() {
  const ack = getState().whatsNew
  const version = getState().tools.appVersion || "0.6.5"
  setState((current) => ({
    ...current,
    showNotes: false,
    whatsNew: false,
    config: ack ? { ...current.config, seenVersion: version } : current.config,
  }))
  if (ack && isTauri()) await invoke("ack_whats_new")
}

export async function pickFolder() {
  if (!isTauri()) {
    setConfig({ outputDir: "C:\\Downloads\\bob baixa" })
    return
  }
  const folder = await chooseFolder()
  if (folder) setConfig({ outputDir: folder })
}

export async function pickConvertFile() {
  if (!isTauri()) {
    setState({ convertInput: "C:\\Videos\\ensaio.mp4" })
    return
  }
  const file = await chooseFile()
  if (file) setState({ convertInput: file })
}

export async function openOutput(path: string) {
  await revealPath(path)
}

export async function makeDiagnostic() {
  setState({ busy: "tools", error: null })
  try {
    if (isTauri()) await invoke("make_diagnostic")
    say("settings_diagnostic_done")
    setState({ busy: null })
  } catch (error) {
    setState({ busy: null, error: errorText(error) })
  }
}

export async function measureLibrary() {
  setState({ busy: "tools", error: null })
  try {
    const count = isTauri() ? await invoke<number>("backfill_gains", { root: getState().config.outputDir }) : 0
    say("settings_gain_done", { n: count })
    setState({ busy: null })
  } catch (error) {
    setState({ busy: null, error: errorText(error) })
  }
}

export async function checkUpdates() {
  setState({ busy: "tools", error: null })
  try {
    if (isTauri()) await invoke("check_updates")
    else say("up_to_date")
    setState({ busy: null })
  } catch (error) {
    setState({ busy: null, error: errorText(error) })
  }
}

export async function applyUpdate() {
  if (!isTauri()) return
  await invoke("apply_update")
}

export async function updateYtDlp() {
  setState({ busy: "tools", error: null })
  try {
    if (isTauri()) await invoke("update_ytdlp")
    else await new Promise((resolve) => window.setTimeout(resolve, 700))
    setState((current) => ({
      ...current,
      busy: null,
      tools: { ...current.tools, ytdlp: "ready", ytdlpVersion: "2026.11.04" },
    }))
    say("ytdlp_updated")
  } catch (error) {
    setState({ busy: null, error: errorText(error) })
  }
}

export async function reinstallTools() {
  setState({ busy: "tools", tools: { ...getState().tools, ytdlp: "downloading", ffmpeg: "downloading", progress: 0.2 } })
  try {
    if (isTauri()) await invoke("install_tools")
    else await new Promise((resolve) => window.setTimeout(resolve, 800))
    setState({
      busy: null,
      tools: { ytdlp: "ready", ffmpeg: "ready", ytdlpVersion: "2026.11.04", ffmpegVersion: "7.1", progress: 1 },
    })
  } catch (error) {
    setState({ busy: null, error: errorText(error), tools: { ...getState().tools, ytdlp: "error", ffmpeg: "error", message: errorText(error) } })
  }
}

export function go(page: Page) {
  setState({ page, error: null })
}

export function setDraft(draft: string) {
  setState({ draft })
}

export function setField<K extends keyof AppState>(key: K, value: AppState[K]) {
  setState({ [key]: value } as Partial<AppState>)
}

export function dismissClipboard() {
  const url = getState().clipboardUrl
  if (url) seenClipboard.add(url)
  setState({ clipboardUrl: null })
}

export function acceptClipboard() {
  const url = getState().clipboardUrl
  if (!url) return
  seenClipboard.add(url)
  setState((current) => ({
    ...current,
    clipboardUrl: null,
    draft: [current.draft.trim(), url].filter(Boolean).join("\n"),
    page: "home",
  }))
}

export async function appendLinks(text: string) {
  const urls = extractUrls(text)
  if (!urls.length) {
    setState({ error: translate(lang(), "invalid_url") })
    return
  }
  setState((current) => ({
    ...current,
    draft: [current.draft.trim(), urls.join("\n")].filter(Boolean).join("\n"),
    error: null,
    page: "home",
    clipboardUrl: null,
  }))
}

function onClipboard(url: string) {
  if (!getState().config.clipboardWatch) return
  if (seenClipboard.has(url) || getState().draft.includes(url)) return
  setState({ clipboardUrl: url })
}

type BobEvent =
  | { type: "jobs"; jobs: Job[] }
  | { type: "tools"; tools: ToolStatus }
  | { type: "history"; history: HistoryEntry[] }
  | { type: "watches"; watches: Watch[] }
  | { type: "clipboard"; url: string }
  | { type: "toast"; message: string }
  | { type: "convert"; job: ConvertJob }
  | { type: "schedule"; at: string | null }
  | { type: "update"; version: string }
  | { type: "ready"; path: string; title: string; artist: string }

function applyEvent(event: BobEvent) {
  if (event.type === "jobs") setState({ jobs: event.jobs })
  else if (event.type === "tools") setState({ tools: event.tools })
  else if (event.type === "history") {
    setState({ history: event.history })
    void refreshCatalog(getState().config.outputDir, () => getState().config.outputDir)
  }
  else if (event.type === "watches") setState({ watches: event.watches })
  else if (event.type === "clipboard") onClipboard(event.url)
  else if (event.type === "toast") {
    setState({ toast: event.message })
    window.clearTimeout(toastTimer)
    toastTimer = window.setTimeout(() => setState({ toast: null }), 2400)
  } else if (event.type === "convert") setState({ convertJob: event.job, busy: event.job.state === "running" ? "convert" : null })
  else if (event.type === "schedule") setState({ scheduleAt: event.at })
  else if (event.type === "update") setState({ updateOffer: true, updateVersion: event.version, busy: null })
  else if (event.type === "ready" && getState().config.queueNewDownloads) {
    const track: Track = {
      id: event.path,
      path: event.path,
      title: event.title || event.path,
      artist: event.artist,
      album: "",
      folder: "",
      pageUrl: "",
      kind: trackKind(event.path),
    }
    queueTracks([track], getState().config.queueNewWhere === "next" ? "next" : "end")
  }
}

async function bindDesktopDrag() {
  if (!isTauri()) return
  const { getCurrentWebview } = await import("@tauri-apps/api/webview")
  await getCurrentWebview().onDragDropEvent((event) => {
    if (event.payload.type === "enter" || event.payload.type === "over") {
      setState({ dragging: true })
      return
    }
    if (event.payload.type === "leave") {
      setState({ dragging: false })
      return
    }
    if (event.payload.type !== "drop") return
    setState({ dragging: false })
    const paths = event.payload.paths
    const media = paths.find((path) => isMediaPath(path))
    if (media) {
      setState({ page: "convert", convertInput: media })
      return
    }
    const text = paths.find((path) => isTextPath(path))
    if (text) {
      void readTextFile(text).then((body) => appendLinks(body)).catch((error: unknown) => setState({ error: errorText(error) }))
    }
  })
}

export function initApp() {
  if (booted) return
  booted = true
  if (!previewMode) {
    const savedTheme = localStorage.getItem("bob-baixa-theme")
    const savedLang = localStorage.getItem("bob-baixa-lang")
    if (savedTheme === "light" || savedTheme === "dark" || savedTheme === "system" || savedTheme === "glass-light" || savedTheme === "glass-dark") {
      state = {
        ...state,
        config: {
          ...state.config,
          theme: savedTheme,
          themeGlass: localStorage.getItem("bob-baixa-glass") === "1" || savedTheme === "glass-light" || savedTheme === "glass-dark",
        },
      }
    }
    if (savedLang === "pt" || savedLang === "en") {
      state = { ...state, config: { ...state.config, language: savedLang } }
    }
  }
  applyTheme(state.config.theme)
  const media = window.matchMedia("(prefers-color-scheme: dark)")
  media.addEventListener("change", () => {
    if (getState().config.theme === "system") applyTheme("system")
  })
  if (!isTauri()) return
  void listenBob<BobEvent>(applyEvent)
  void (async () => {
    type Snapshot = {
      config: Config
      jobs: Job[]
      history: HistoryEntry[]
      watches: Watch[]
      tools: ToolStatus
      scheduleAt?: string | null
      convertJob?: ConvertJob | null
      ready?: boolean
      whatsNew?: boolean
      releaseNotes?: string
    }
    const started = Date.now()
    let snapshot: Snapshot | null = null
    while (Date.now() - started < 4000) {
      snapshot = await invoke<Snapshot>("state")
      if (snapshot.ready) break
      await new Promise((resolve) => window.setTimeout(resolve, 40))
    }
    if (!snapshot) return
    const loaded = snapshot
    setState((current) => ({
      ...current,
      config: loaded.config,
      jobs: loaded.jobs,
      history: loaded.history,
      watches: loaded.watches,
      tools: loaded.tools,
      scheduleAt: loaded.scheduleAt ?? null,
      convertJob: loaded.convertJob ?? null,
      whatsNew: Boolean(loaded.whatsNew),
      showNotes: Boolean(loaded.whatsNew),
      releaseNotes: loaded.releaseNotes ?? "",
    }))
    applyTheme(loaded.config.theme, loaded.config.themeGlass)
    hydrated = true
    const patch = pendingPatch
    pendingPatch = null
    if (patch) setConfig(patch)
    void refreshCatalog(getState().config.outputDir, () => getState().config.outputDir)
  })()
  void bindDesktopDrag()
}

export const PREVIEW = previewMode
