import type { ListenEvent } from "./listening"

export function weekPaths(events: ListenEvent[], limit = 10) {
  const since = Date.now() - 7 * 86_400_000
  const counts = new Map<string, number>()
  for (const event of events) {
    if (event.at < since) continue
    counts.set(event.path, (counts.get(event.path) || 0) + (event.play ? 1 : 0))
  }
  return [...counts.entries()]
    .filter((entry) => entry[1] > 0)
    .sort((left, right) => right[1] - left[1])
    .slice(0, limit)
    .map(([path]) => path)
}
