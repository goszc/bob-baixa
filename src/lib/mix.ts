import type { ListenEvent } from "./listening"
import { trackKey, type Track } from "./library.ts"

function dayStamp(now: number) {
  const date = new Date(now)
  const month = `${date.getMonth() + 1}`.padStart(2, "0")
  const day = `${date.getDate()}`.padStart(2, "0")
  return `${date.getFullYear()}-${month}-${day}`
}

function hash(text: string) {
  let value = 2166136261
  for (let index = 0; index < text.length; index += 1) {
    value ^= text.charCodeAt(index)
    value = Math.imul(value, 16777619)
  }
  return value >>> 0
}

function artistKey(track: Track) {
  return track.artist.trim().toLowerCase()
}

/** A daily mix from local listening history. The same day and the same history return the same list. */
export function dailyMix(tracks: Track[], events: ListenEvent[], now: number, limit = 25): Track[] {
  if (!tracks.length) return []
  const stamp = dayStamp(now)
  const since = now - 90 * 24 * 60 * 60 * 1000
  const weight = new Map<string, number>()
  for (const event of events) {
    if (event.at < since || event.at > now + 1000) continue
    const artist = event.artist.trim().toLowerCase()
    if (!artist) continue
    weight.set(artist, (weight.get(artist) ?? 0) + Math.max(1, event.ms / 30000))
  }
  const ranked = [...weight.entries()].sort((left, right) => right[1] - left[1]).slice(0, 8)
  const wanted = new Set(ranked.map(([artist]) => artist))
  const pool = tracks.filter((track) => {
    const artist = artistKey(track)
    return artist && (wanted.size === 0 || wanted.has(artist))
  })
  const source = pool.length >= 5 ? pool : tracks
  const ordered = [...source].sort((left, right) => {
    const leftKey = hash(`${stamp}:${trackKey(left.path)}`)
    const rightKey = hash(`${stamp}:${trackKey(right.path)}`)
    return leftKey - rightKey
  })
  const picked: Track[] = []
  const seen = new Set<string>()
  const perArtist = new Map<string, number>()
  for (const track of ordered) {
    const key = trackKey(track.path)
    if (seen.has(key)) continue
    const artist = artistKey(track) || key
    if ((perArtist.get(artist) ?? 0) >= 4) continue
    seen.add(key)
    perArtist.set(artist, (perArtist.get(artist) ?? 0) + 1)
    picked.push(track)
    if (picked.length >= limit) break
  }
  return picked
}
