import assert from "node:assert/strict"
import test from "node:test"
import { DRIVE_SCOPE, activeLyric, choosePlayback, libraryFromFiles, libraryQuery, lyricLines, mergeLibrary, mergePhoneState } from "./library.js"

test("the phone asks only for files this project created", () => {
  assert.equal(DRIVE_SCOPE, "https://www.googleapis.com/auth/drive.file")
  const query = libraryQuery()
  assert.match(query, /trashed = false/)
  assert.match(query, /bobKind/)
  assert.doesNotMatch(query, /drive\.readonly/)
})

test("a desktop upload in the same project is playable, with its lyric and cover", () => {
  const tracks = libraryFromFiles([
    { id: "other", name: "imposto.pdf", appProperties: {} },
    {
      id: "audio-1",
      name: "Saudade [abc].opus",
      appProperties: { bobKind: "audio", bobTitle: "Saudade", bobArtist: "Amália", bobAlbum: "Avulsas" },
    },
    { id: "lyric-1", name: "Saudade.lrc", appProperties: { bobKind: "lyrics", bobMedia: "audio-1" } },
    { id: "cover-1", name: "Saudade.cover.jpg", appProperties: { bobKind: "cover", bobMedia: "audio-1" } },
  ])
  assert.equal(tracks.length, 1)
  assert.equal(tracks[0].title, "Saudade")
  assert.equal(tracks[0].artist, "Amália")
  assert.equal(tracks[0].lyricsId, "lyric-1")
  assert.equal(tracks[0].coverId, "cover-1")
})

test("the phone index fills title, cover and synced lyrics the file list does not have", () => {
  const files = [
    { id: "audio-1", name: 'Low Roar - "I\'ll Keep Coming".opus', mimeType: "audio/ogg", size: "4000", appProperties: { bobKind: "audio" } },
    { id: "index-1", name: "index.json", appProperties: { bobKind: "index" } },
  ]
  const merged = mergeLibrary(files, {
    version: 1,
    tracks: [
      {
        id: "audio-1",
        title: "I'll Keep Coming",
        artist: "Low Roar",
        coverId: "cover-9",
        lyrics: "[00:12.00]I'll keep coming",
      },
    ],
  })
  assert.equal(merged[0].title, "I'll Keep Coming")
  assert.equal(merged[0].artist, "Low Roar")
  assert.equal(merged[0].coverId, "cover-9")
  const lines = lyricLines(merged[0].lyrics)
  assert.equal(lines[0].text, "I'll keep coming")
  assert.equal(activeLyric(lines, 12), 0)
})

test("likes and playlists merge without dropping either side", () => {
  const merged = mergePhoneState(
    { updatedAt: 10, likes: ["a"], playlists: [{ name: "Noite", paths: ["a"] }], queue: ["a"] },
    { updatedAt: 20, likes: ["b"], playlists: [{ name: "noite", paths: ["b"] }], queue: ["b"] },
  )
  assert.deepEqual(merged.likes.sort(), ["a", "b"])
  assert.deepEqual(merged.playlists[0].paths.sort(), ["a", "b"])
  assert.deepEqual(merged.queue, ["b"])
})

test("ios uses an m4a of the same song and otherwise explains opus", () => {
  const opus = { id: "a", name: "song.opus", title: "Saudade", artist: "Ana", video: false }
  const m4a = { id: "b", name: "song.m4a", title: "Saudade", artist: "Ana", video: false }
  const swapped = choosePlayback(opus, [opus, m4a], { ios: true, opus: false })
  assert.equal(swapped.id, "b")
  assert.equal(swapped.unsupported, false)
  const blocked = choosePlayback(opus, [opus], { ios: true, opus: false })
  assert.equal(blocked.unsupported, true)
  assert.match(blocked.reason, /opus/)
  const video = choosePlayback({ id: "v", name: "clip.mp4", title: "Clipe", artist: "", video: true }, [], { ios: true, opus: false })
  assert.equal(video.element, "video")
})
