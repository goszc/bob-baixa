import type { Track } from "./library.ts"

export interface CopyQuality {
  path: string
  title: string
  format: string
  bitrate: number
  size: number
  cover: boolean
  score: number
}

export interface DuplicatePair {
  reason: string
  copies: CopyQuality[]
  worse: string
}

function fold(value: string) {
  return value.trim().toLocaleLowerCase().normalize("NFD").replace(/\p{M}/gu, "")
}

function formatOf(path: string) {
  const match = /\.([a-z0-9]+)$/i.exec(path)
  return match ? match[1].toLowerCase() : ""
}

function score(format: string, bitrate: number, cover: boolean) {
  const bonus = format === "flac" || format === "wav" ? 30 : format === "mp3" || format === "m4a" || format === "mp4" ? 20 : 0
  return Math.round(bitrate / 1000) + bonus + (cover ? 40 : 0)
}

/** Same normalized title and artist, close duration, even when the file names differ. */
export function duplicatePairs(tracks: Track[]): DuplicatePair[] {
  const groups = new Map<string, Track[]>()
  for (const track of tracks) {
    const title = fold(track.title)
    const artist = fold(track.artist)
    if (title.length < 2) continue
    const key = `${artist}\n${title}`
    const list = groups.get(key) ?? []
    list.push(track)
    groups.set(key, list)
  }
  const pairs: DuplicatePair[] = []
  for (const list of groups.values()) {
    const bucket: Track[] = []
    for (const track of list) {
      const close = bucket.some((other) => {
        if (!track.duration || !other.duration) return true
        return Math.abs(track.duration - other.duration) <= 3
      })
      if (close || bucket.length === 0) bucket.push(track)
    }
    const unique = [...new Map(bucket.map((track) => [track.path, track])).values()]
    if (unique.length < 2) continue
    const copies = unique.map((track) => {
      const format = formatOf(track.path)
      const bitrate = track.duration && track.size ? Math.round((track.size * 8) / track.duration) : 0
      const cover = Boolean(track.thumbnail)
      return { path: track.path, title: track.title, format, bitrate, size: track.size ?? 0, cover, score: score(format, bitrate, cover) }
    })
    const worse = [...copies].sort((left, right) => left.score - right.score || left.size - right.size)[0]?.path ?? ""
    pairs.push({ reason: "similar", copies, worse })
  }
  return pairs
}
