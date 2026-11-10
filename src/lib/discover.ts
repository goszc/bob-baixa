export function discoverySplit(total: number, ratio: number): { library: number; fresh: number } {
  const safe = Number.isFinite(ratio) ? Math.min(0.8, Math.max(0, ratio)) : 0.35
  const fresh = Math.round(Math.max(0, total) * safe)
  return { library: Math.max(0, total - fresh), fresh }
}

export function parseGeminiTracks(text: string): { artist: string; title: string }[] {
  const start = text.indexOf("{")
  const end = text.lastIndexOf("}")
  const slice = start >= 0 && end > start ? text.slice(start, end + 1) : text
  const value = JSON.parse(slice) as { tracks?: { artist?: string; title?: string }[] }
  return (value.tracks ?? [])
    .map((row) => ({ artist: (row.artist ?? "").trim(), title: (row.title ?? "").trim() }))
    .filter((row) => row.title)
}

export function nextPaths(order: number[], index: number, count: number): number[] {
  const at = order.indexOf(index)
  if (at < 0) return []
  return order.slice(at + 1, at + 1 + count)
}
