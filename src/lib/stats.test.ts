import assert from "node:assert/strict"
import test from "node:test"
import { liveStats } from "./stats.ts"
import type { ListenEvent } from "./listening.ts"

const now = Date.UTC(2026, 10, 10, 15, 0, 0)

function listen(partial: Partial<ListenEvent> & Pick<ListenEvent, "at" | "ms" | "artist">): ListenEvent {
  return { path: partial.path ?? partial.artist, title: partial.title ?? "faixa", album: "", play: true, ...partial }
}

test("the home card reads the week, the month and the hour", () => {
  const stats = liveStats(
    [
      listen({ artist: "Low Roar", at: now - 2 * 24 * 60 * 60 * 1000, ms: 10 * 60 * 1000 }),
      listen({ artist: "Low Roar", at: now - 60 * 60 * 1000, ms: 20 * 60 * 1000 }),
      listen({ artist: "Nils Frahm", at: now - 30 * 60 * 1000, ms: 5 * 60 * 1000 }),
      listen({ artist: "Old", at: now - 40 * 24 * 60 * 60 * 1000, ms: 90 * 60 * 1000 }),
    ],
    now,
  )
  assert.equal(Math.round(stats.weekMinutes), 35)
  assert.equal(stats.artist, "Low Roar")
  assert.equal(stats.hour, new Date(now - 60 * 60 * 1000).getHours())
})
