import assert from "node:assert/strict"
import test from "node:test"
import { parseManifest, parseM3u, retrospective, type ListenEvent } from "./listening.ts"
import { builtinPresets } from "./presets.ts"

function at(year: number, month: number, day: number) {
  return new Date(year, month - 1, day, 12).getTime()
}

function listen(partial: Partial<ListenEvent> & Pick<ListenEvent, "path" | "at">): ListenEvent {
  return { title: partial.path, artist: "Ana", ms: 40_000, play: true, ...partial }
}

test("retrospective covers week, month, calendar year and the last 12 months", () => {
  const now = at(2026, 3, 10)
  const events = [
    listen({ path: "a", title: "Alpha", at: at(2026, 3, 10) }),
    listen({ path: "a", title: "Alpha", at: at(2026, 3, 9) }),
    listen({ path: "b", title: "Beta", artist: "Bia", at: at(2026, 3, 8) }),
    listen({ path: "old", title: "Antiga", artist: "Cris", at: at(2025, 12, 1) }),
  ]
  const week = retrospective(events, "week", now)
  assert.equal(week.currentStreak, 3)
  assert.equal(week.periodStreak, 3)
  assert.equal(week.tracks[0].label, "Alpha")
  assert.equal(week.tracks[0].plays, 2)
  assert.equal(week.artists[0].label, "Ana")
  assert.ok(week.minutes > 1)
  assert.equal(week.tracks.some((item) => item.label === "Antiga"), false)

  const month = retrospective(events, "month", now)
  assert.equal(month.tracks.length, 2)

  const year = retrospective(events, "year", now)
  assert.equal(year.tracks.some((item) => item.label === "Antiga"), false)

  const rolling = retrospective(events, "rolling", now)
  assert.equal(rolling.tracks.some((item) => item.label === "Antiga"), true)
  assert.ok(rolling.periodStreak >= 1)
})

test("retrospective ranks albums and months without a model", () => {
  const now = at(2026, 3, 10)
  const events = [
    listen({ path: "a", title: "Alpha", artist: "Ana", album: "Disco", at: at(2026, 3, 10), ms: 120_000 }),
    listen({ path: "b", title: "Beta", artist: "Bia", album: "Disco", at: at(2026, 2, 2), ms: 90_000 }),
    listen({ path: "drive:1", title: "Nuvem", artist: "Ana", album: "Ao vivo", at: at(2026, 3, 4), ms: 60_000 }),
  ]
  const year = retrospective(events, "year", now)
  assert.equal(year.albums[0].label, "Disco")
  assert.equal(year.albums[0].plays, 2)
  assert.ok(year.months.length >= 2)
  assert.ok(year.minutes > 3)
})

test("a missed day breaks the streak but yesterday still counts", () => {
  const now = at(2026, 5, 10)
  const events = [listen({ path: "a", at: at(2026, 5, 8) }), listen({ path: "a", at: at(2026, 5, 7) })]
  const summary = retrospective(events, "month", now)
  assert.equal(summary.currentStreak, 0)
})

test("manifest and m3u stay small and local", () => {
  const parsed = parseManifest('{"version":1,"playlists":[{"name":"Noite","paths":["C:/a.mp3"]}],"likes":["C:/a.mp3"]}')
  assert.equal(parsed?.playlists[0].name, "Noite")
  assert.equal(parseManifest('{"version":2}'), null)
  assert.deepEqual(parseM3u("#EXTM3U\n#EXTINF:1,A\nC:/a.mp3\n"), ["C:/a.mp3"])
})

test("economic preset is small opus and does not replace mp3", () => {
  const economy = builtinPresets.find((preset) => preset.id === "builtin-economy")
  const mp3 = builtinPresets.find((preset) => preset.id === "builtin-mp3")
  assert.equal(economy?.audioOnly, true)
  assert.equal(economy?.audioFormat, "opus")
  assert.equal(economy?.audioBitrate, "24k")
  assert.equal(economy?.embedThumbnail, false)
  assert.equal(mp3?.audioFormat, "mp3")
  assert.equal(mp3?.audioBitrate, "192k")
  assert.equal(mp3?.audioOnly, true)
})