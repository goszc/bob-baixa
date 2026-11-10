import assert from "node:assert/strict"
import test from "node:test"
import { discoverySplit, nextPaths, parseGeminiTracks } from "./discover.ts"

test("prefetch picks the next queue slots and discovery keeps a library majority", () => {
  assert.deepEqual(nextPaths([2, 0, 4, 1], 0, 2), [4, 1])
  assert.deepEqual(discoverySplit(10, 0.3), { library: 7, fresh: 3 })
})

test("gemini json parses inside a fence", () => {
  const rows = parseGeminiTracks('```json\n{"tracks":[{"artist":"A","title":"Manhã"}]}\n```')
  assert.deepEqual(rows, [{ artist: "A", title: "Manhã" }])
})
