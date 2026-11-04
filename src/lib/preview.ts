import type { HistoryEntry, Job, MediaItem, SearchHit, Watch } from "./types"

const lisboa = "/cache/YF4wa9gB0cA.jpg"
const coffee = "/cache/7xXWZcbGOP0.jpg"
const neighborhoods = "/cache/N9xsbTE94YI.jpg"
const anavitoria = "/cache/KS_TrJOQB-g.jpg"
const vlog = "/cache/r_S8FRy7gU8.jpg"
const sintra = "/cache/dvGOktfWWNU.jpg"

export const previewAnalyzed: MediaItem[] = [
  {
    kind: "video",
    id: "YF4wa9gB0cA",
    url: "https://www.youtube.com/watch?v=YF4wa9gB0cA",
    title: "O que fazer em Lisboa, Portugal",
    channel: "Vou na Janela",
    duration: 1337,
    thumbnail: lisboa,
    entries: [],
    formats: [
      { id: "137", label: "1080p · mp4", height: 1080, ext: "mp4", fps: 30 },
      { id: "136", label: "720p · mp4", height: 720, ext: "mp4", fps: 30 },
    ],
  },
]

export const previewJobs: Job[] = [
  {
    id: "job-1",
    url: "https://www.youtube.com/watch?v=7xXWZcbGOP0",
    title: "Morning Coffee in Lisbon",
    thumbnail: coffee,
    duration: 2804,
    state: "downloading",
    progress: 0.62,
    speed: 4.8 * 1024 * 1024,
    eta: 38,
    downloaded: 52 * 1024 * 1024,
    total: 84 * 1024 * 1024,
    createdAt: new Date().toISOString(),
    audioOnly: false,
    quality: "1080",
    container: "mp4",
  },
  {
    id: "job-2",
    url: "https://www.youtube.com/watch?v=N9xsbTE94YI",
    title: "Neighborhoods in Lisbon",
    thumbnail: neighborhoods,
    duration: 443,
    state: "scheduled",
    progress: 0,
    createdAt: new Date().toISOString(),
    scheduledAt: new Date(Date.now() + 3 * 60 * 60 * 1000).toISOString(),
    audioOnly: true,
    audioFormat: "mp3",
    quality: "best",
    container: "mp4",
  },
  {
    id: "job-3",
    url: "https://www.youtube.com/watch?v=KS_TrJOQB-g",
    title: "Lisboa (ao vivo)",
    thumbnail: anavitoria,
    duration: 267,
    state: "failed",
    progress: 0.12,
    error: "feche o navegador e tente de novo.",
    createdAt: new Date().toISOString(),
    audioOnly: false,
    quality: "1080",
    container: "mp4",
  },
]

export const previewHits: SearchHit[] = [
  {
    id: "YF4wa9gB0cA",
    title: "O que fazer em Lisboa, Portugal",
    url: "https://www.youtube.com/watch?v=YF4wa9gB0cA",
    channel: "Vou na Janela",
    duration: 1337,
    thumbnail: lisboa,
  },
  {
    id: "7xXWZcbGOP0",
    title: "Morning Coffee in Lisbon",
    url: "https://www.youtube.com/watch?v=7xXWZcbGOP0",
    channel: "Morning to Moonlight",
    duration: 2804,
    thumbnail: coffee,
  },
  {
    id: "dvGOktfWWNU",
    title: "O que fazer em Sintra, Portugal",
    url: "https://www.youtube.com/watch?v=dvGOktfWWNU",
    channel: "Vou na Janela",
    duration: 961,
    thumbnail: sintra,
  },
  {
    id: "r_S8FRy7gU8",
    title: "Minha manhã em uma cidade florida",
    url: "https://www.youtube.com/watch?v=r_S8FRy7gU8",
    channel: "Giovanna Lisboa",
    duration: 858,
    thumbnail: vlog,
  },
]

export const previewHistory: HistoryEntry[] = [
  {
    id: "h1",
    title: "O que fazer em Lisboa, Portugal",
    url: "https://www.youtube.com/watch?v=YF4wa9gB0cA",
    path: "C:\\Downloads\\bob baixa\\O que fazer em Lisboa, Portugal [YF4wa9gB0cA].mp4",
    at: new Date(Date.now() - 3600_000).toISOString(),
    ok: true,
  },
]

export const previewWatches: Watch[] = [
  {
    id: "w1",
    url: "https://www.youtube.com/@VouNaJanela",
    title: "Vou na Janela",
    everyMinutes: 30,
    enabled: true,
    lastChecked: new Date(Date.now() - 20 * 60_000).toISOString(),
    known: 24,
  },
]

export const cachedThumbs = [lisboa, coffee, neighborhoods, anavitoria, vlog, sintra]
