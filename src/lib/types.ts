export type Lang = "pt" | "en"
export type ThemeMode = "system" | "light" | "dark" | "glass-light" | "glass-dark"
export type Page =
  | "home"
  | "player"
  | "queue"
  | "search"
  | "convert"
  | "monitor"
  | "history"
  | "settings"

export type Quality = "best" | "2160" | "1440" | "1080" | "720" | "480" | "360"
export type Container = "mp4" | "mkv" | "webm"
export type AudioFormat = "mp3" | "m4a" | "opus" | "flac"
export type JobState =
  | "queued"
  | "scheduled"
  | "downloading"
  | "paused"
  | "done"
  | "failed"
  | "cancelled"
export type CookiesBrowser =
  | "auto"
  | "none"
  | "chrome"
  | "edge"
  | "firefox"
  | "brave"
  | "opera"
  | "vivaldi"
  | "chromium"
export type ConvertPreset = "mp3" | "m4a" | "opus" | "flac" | "mp4" | "webm"
export type ToolPhase = "missing" | "downloading" | "ready" | "error"

export interface Config {
  language: Lang
  theme: ThemeMode
  themeGlass: boolean
  outputDir: string
  filenameTemplate: string
  parallel: number
  globalSpeed: string
  clipboardWatch: boolean
  minimizeToTray: boolean
  notifications: boolean
  hotkey: boolean
  quality: Quality
  container: Container
  audioOnly: boolean
  audioFormat: AudioFormat
  audioBitrate: string
  subtitles: boolean
  embedSubs: boolean
  subLangs: string
  embedThumbnail: boolean
  embedMetadata: boolean
  embedChapters: boolean
  sponsorblock: boolean
  sponsorCategories: string[]
  cookiesBrowser: CookiesBrowser
  downloadKnownOnFirstWatch: boolean
  autoUpdate: boolean
  updateUrl: string
  contextMenu: boolean
  presets: DownloadPreset[]
  seenVersion: string
}

export interface DownloadPreset {
  id: string
  name: string
  quality: Quality
  container: Container
  audioOnly: boolean
  audioFormat: AudioFormat
  audioBitrate: string
  subtitles: boolean
  embedSubs: boolean
  subLangs: string
  embedThumbnail: boolean
  embedMetadata: boolean
  embedChapters: boolean
  sponsorblock: boolean
  sponsorCategories: string[]
}

export interface FormatOption {
  id: string
  label: string
  height?: number
  ext: string
  note?: string
  fps?: number
  filesize?: number
}

export interface PlaylistEntry {
  id: string
  url: string
  title: string
  duration?: number
  thumbnail?: string
}

export interface MediaItem {
  kind: "video" | "playlist"
  id: string
  url: string
  title: string
  channel?: string
  duration?: number
  thumbnail?: string
  viewCount?: number
  formats: FormatOption[]
  entries: PlaylistEntry[]
}

export interface Job {
  id: string
  url: string
  title: string
  thumbnail?: string
  duration?: number
  state: JobState
  progress: number
  speed?: number
  eta?: number
  downloaded?: number
  total?: number
  error?: string
  outputPath?: string
  createdAt: string
  scheduledAt?: string
  sectionStart?: string
  sectionEnd?: string
  speedLimit?: string
  audioOnly: boolean
  quality: string
  container: string
  audioFormat?: string
}

export interface HistoryEntry {
  id: string
  title: string
  url: string
  path?: string
  at: string
  ok: boolean
  detail?: string
}

export interface Watch {
  id: string
  url: string
  title: string
  everyMinutes: number
  enabled: boolean
  lastChecked?: string
  known: number
}

export interface ToolStatus {
  ytdlp: ToolPhase
  ffmpeg: ToolPhase
  ytdlpVersion?: string
  ffmpegVersion?: string
  message?: string
  progress?: number
  appVersion?: string
}

export interface SearchHit {
  id: string
  title: string
  url: string
  channel?: string
  duration?: number
  thumbnail?: string
}

export interface ConvertJob {
  id: string
  input: string
  output?: string
  preset: ConvertPreset
  state: "running" | "done" | "failed" | "cancelled"
  progress: number
  error?: string
}

export interface EnqueueItem {
  url: string
  title?: string
  thumbnail?: string
  duration?: number
  formatId?: string
  sectionStart?: string
  sectionEnd?: string
  speedLimit?: string
  scheduledAt?: string
}

export const defaultConfig: Config = {
  language: "pt",
  theme: "system",
  themeGlass: false,
  outputDir: "",
  filenameTemplate: "%(title)s [%(id)s].%(ext)s",
  parallel: 2,
  globalSpeed: "",
  clipboardWatch: true,
  minimizeToTray: true,
  notifications: true,
  hotkey: true,
  quality: "1080",
  container: "mp4",
  audioOnly: false,
  audioFormat: "mp3",
  audioBitrate: "192k",
  subtitles: false,
  embedSubs: true,
  subLangs: "pt.*,en.*",
  embedThumbnail: true,
  embedMetadata: true,
  embedChapters: true,
  sponsorblock: false,
  sponsorCategories: ["sponsor", "intro", "outro", "selfpromo"],
  cookiesBrowser: "auto",
  downloadKnownOnFirstWatch: false,
  autoUpdate: true,
  updateUrl: "",
  contextMenu: false,
  presets: [],
  seenVersion: "",
}
