export type RetroRange = "week" | "month" | "year" | "rolling"

export interface ListenEvent {
  path: string
  title: string
  artist: string
  at: number
  ms: number
  play: boolean
}

export interface Ranked {
  key: string
  label: string
  detail: string
  plays: number
  ms: number
}

export interface RetroSummary {
  minutes: number
  currentStreak: number
  periodStreak: number
  tracks: Ranked[]
  artists: Ranked[]
}

const DAY = 24 * 60 * 60 * 1000

export function retrospective(events: ListenEvent[], range: RetroRange, now: number): RetroSummary {
  const start = rangeStart(range, now)
  const inRange = events.filter((event) => event.at >= start && event.at <= now + 1000)
  const minutes = inRange.reduce((sum, event) => sum + Math.max(0, event.ms), 0) / 60000
  const listenedDays = new Set<number>()
  const periodDays = new Set<number>()
  for (const event of events) {
    if (!countsAsListen(event)) continue
    const day = dayKey(event.at)
    listenedDays.add(day)
    if (event.at >= start && event.at <= now + 1000) periodDays.add(day)
  }
  return {
    minutes,
    currentStreak: streakEnding(listenedDays, dayKey(now)),
    periodStreak: longestStreak(periodDays),
    tracks: rankTracks(inRange),
    artists: rankArtists(inRange),
  }
}

function countsAsListen(event: ListenEvent) {
  return event.play || event.ms >= 30_000
}

function rangeStart(range: RetroRange, now: number) {
  const date = new Date(now)
  if (range === "week") return now - 7 * DAY
  if (range === "rolling") return now - 365 * DAY
  if (range === "month") return new Date(date.getFullYear(), date.getMonth(), 1).getTime()
  return new Date(date.getFullYear(), 0, 1).getTime()
}

function dayKey(ms: number) {
  const date = new Date(ms)
  return date.getFullYear() * 10000 + (date.getMonth() + 1) * 100 + date.getDate()
}

function previousDay(key: number) {
  const year = Math.floor(key / 10000)
  const month = Math.floor((key % 10000) / 100)
  const day = key % 100
  const date = new Date(year, month - 1, day)
  date.setDate(date.getDate() - 1)
  return dayKey(date.getTime())
}

function nextDay(key: number) {
  const year = Math.floor(key / 10000)
  const month = Math.floor((key % 10000) / 100)
  const day = key % 100
  const date = new Date(year, month - 1, day)
  date.setDate(date.getDate() + 1)
  return dayKey(date.getTime())
}

function streakEnding(days: Set<number>, today: number) {
  let cursor = days.has(today) ? today : previousDay(today)
  if (!days.has(cursor)) return 0
  let count = 0
  while (days.has(cursor)) {
    count += 1
    cursor = previousDay(cursor)
  }
  return count
}

function longestStreak(days: Set<number>) {
  const sorted = [...days].sort((left, right) => left - right)
  let best = 0
  let run = 0
  let prev = 0
  for (const day of sorted) {
    run = prev && day === nextDay(prev) ? run + 1 : 1
    prev = day
    if (run > best) best = run
  }
  return best
}

function rankTracks(events: ListenEvent[]): Ranked[] {
  const map = new Map<string, Ranked>()
  for (const event of events) {
    const current = map.get(event.path) ?? {
      key: event.path,
      label: event.title || event.path,
      detail: event.artist,
      plays: 0,
      ms: 0,
    }
    current.ms += Math.max(0, event.ms)
    if (event.play) current.plays += 1
    if (event.title) current.label = event.title
    if (event.artist) current.detail = event.artist
    map.set(event.path, current)
  }
  return [...map.values()].filter((item) => item.plays > 0 || item.ms >= 15_000).sort(byPlay).slice(0, 8)
}

function rankArtists(events: ListenEvent[]): Ranked[] {
  const map = new Map<string, Ranked>()
  for (const event of events) {
    const name = event.artist.trim()
    if (!name) continue
    const key = name.toLocaleLowerCase()
    const current = map.get(key) ?? { key, label: name, detail: "", plays: 0, ms: 0 }
    current.ms += Math.max(0, event.ms)
    if (event.play) current.plays += 1
    map.set(key, current)
  }
  return [...map.values()].filter((item) => item.plays > 0 || item.ms >= 15_000).sort(byPlay).slice(0, 8)
}

function byPlay(left: Ranked, right: Ranked) {
  if (right.plays !== left.plays) return right.plays - left.plays
  return right.ms - left.ms
}

export interface ManifestPlaylist {
  name: string
  paths: string[]
}

export interface LibraryManifest {
  version: 1
  playlists: ManifestPlaylist[]
  likes: string[]
}

export function parseManifest(text: string): LibraryManifest | null {
  try {
    const value = JSON.parse(text) as Partial<LibraryManifest>
    if (value.version !== 1 || !Array.isArray(value.playlists) || !Array.isArray(value.likes)) return null
    const playlists = value.playlists
      .filter((item) => item && typeof item.name === "string" && Array.isArray(item.paths))
      .map((item) => ({
        name: item.name.trim(),
        paths: item.paths.filter((path): path is string => typeof path === "string" && path.trim().length > 0),
      }))
      .filter((item) => item.name.length > 0)
    const likes = value.likes.filter((path): path is string => typeof path === "string" && path.trim().length > 0)
    return { version: 1, playlists, likes }
  } catch {
    return null
  }
}

export function parseM3u(text: string): string[] {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith("#"))
}
