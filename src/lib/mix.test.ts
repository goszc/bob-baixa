import assert from "node:assert/strict"
import test from "node:test"
import type { ListenEvent } from "./listening.ts"
import type { Track } from "./library.ts"
import { dailyMix } from "./mix.ts"

function track(path: string, artist: string): Track {
  return { id: path, path, title: path, artist, album: "", folder: "", pageUrl: "", kind: "audio" }
}

test("the daily mix stays the same during one day and prefers artists you played", () => {
  const tracks = [
    track("a.mp3", "Banda"),
    track("b.mp3", "Banda"),
    track("c.mp3", "Outra"),
    track("d.mp3", "Banda"),
  ]
  const events: ListenEvent[] = [
    { path: "a.mp3", title: "a", artist: "Banda", at: Date.parse("2026-11-01T12:00:00Z"), ms: 120_000, play: true },
  ]
  const now = Date.parse("2026-11-10T15:00:00Z")
  const first = dailyMix(tracks, events, now)
  const second = dailyMix(tracks, events, now + 60_000)
  assert.deepEqual(first.map((item) => item.path), second.map((item) => item.path))
  assert.ok(first.filter((item) => item.artist === "Banda").length >= first.filter((item) => item.artist === "Outra").length)
  const tomorrow = dailyMix(tracks, events, now + 24 * 60 * 60 * 1000)
  assert.equal(tomorrow.length, first.length)
})
