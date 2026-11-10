import type { ListenEvent } from "./listening.ts"

export interface LiveStats {
  weekMinutes: number
  artist: string
  hour: number | null
}

/** Minutes in the last 7 days, the artist with the most time this calendar month, and the hour that gets the most listening. */
export function liveStats(events: ListenEvent[], now: number): LiveStats {
  const weekStart = now - 7 * 24 * 60 * 60 * 1000
  const month = new Date(now)
  const monthStart = new Date(month.getFullYear(), month.getMonth(), 1).getTime()
  let weekMs = 0
  const artists = new Map<string, { label: string; ms: number }>()
  const hours = new Array<number>(24).fill(0)
  for (const event of events) {
    if (!event || event.at > now + 1000) continue
    const ms = Math.max(0, event.ms)
    if (event.at >= weekStart) weekMs += ms
    if (event.at >= monthStart) {
      const label = event.artist.trim()
      if (label) {
        const key = label.toLocaleLowerCase()
        const current = artists.get(key) ?? { label, ms: 0 }
        current.ms += ms
        artists.set(key, current)
      }
      hours[new Date(event.at).getHours()] += ms
    }
  }
  let artist = ""
  let best = 0
  for (const row of artists.values()) {
    if (row.ms > best) {
      best = row.ms
      artist = row.label
    }
  }
  let hour: number | null = null
  let hourBest = 0
  hours.forEach((ms, index) => {
    if (ms > hourBest) {
      hourBest = ms
      hour = index
    }
  })
  return { weekMinutes: weekMs / 60000, artist, hour }
}
