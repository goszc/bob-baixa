import assert from "node:assert/strict"
import test from "node:test"
import { duplicatePairs } from "./duplicates.ts"
import type { Track } from "./library.ts"

function track(partial: Partial<Track> & Pick<Track, "path" | "title">): Track {
  return { id: partial.path, artist: "", album: "", folder: "", pageUrl: "", kind: "audio", ...partial }
}

test("the same song with two names is a pair and the smaller one is worse", () => {
  const pairs = duplicatePairs([
    track({ path: "drive:good", title: "I'll Keep Coming", artist: "Low Roar", duration: 180, size: 4_000_000, thumbnail: "cover" }),
    track({ path: "C:/Music/rip.opus", title: "I'll Keep Coming", artist: "low roar", duration: 181, size: 400_000 }),
    track({ path: "C:/Music/other.mp3", title: "Holocene", artist: "Bon Iver", duration: 300, size: 8_000_000 }),
  ])
  assert.equal(pairs.length, 1)
  assert.equal(pairs[0].worse, "C:/Music/rip.opus")
  assert.equal(pairs[0].copies.length, 2)
})
