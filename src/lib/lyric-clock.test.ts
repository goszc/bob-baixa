import assert from "node:assert/strict"
import test from "node:test"
import { activeLine, lyricClock } from "./lyric-clock.ts"

test("a positive nudge delays the highlighted line", () => {
  const lines = [
    { at: 1, text: "a" },
    { at: 2, text: "b" },
  ]
  assert.equal(lyricClock(2, 0.5), 1.5)
  assert.equal(activeLine(lines, lyricClock(2, 0.5)), 0)
  assert.equal(activeLine(lines, lyricClock(2, 0)), 1)
  assert.equal(lyricClock(0.2, 1), 0)
})
