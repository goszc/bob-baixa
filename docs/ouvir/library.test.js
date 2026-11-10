import assert from "node:assert/strict"
import test from "node:test"
import { DRIVE_SCOPE, activeLyric, choosePlayback, coverPlaceholder, isLyricText, libraryFromFiles, libraryQuery, lyricLines, mergeLibrary, mergePhoneState, normalizeTrack, trackIsLiked } from "./library.js"

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

test("uploader titles become artist and title, and a jpeg is never lyrics", () => {
  const merged = mergeLibrary(
    [
      { id: "hania", name: "Gondwana Records — Hania Rani — 'Leaving' (Official Video) [Gondwana Records].mp4", mimeType: "video/mp4", appProperties: { bobKind: "video" } },
      { id: "leaf", name: "Sub Pop — The Album Leaf - The Outer Banks [OFFICIAL VIDEO].mp4", mimeType: "video/mp4", appProperties: { bobKind: "video" } },
      { id: "caps", name: "DISAPPEARED COMPLETELY — Disappeared Completley - Once In a While (short mood video).mp3", mimeType: "audio/mpeg", appProperties: { bobKind: "audio" } },
      { id: "a", name: 'Low Roar - "I\'ll Keep Coming".opus', mimeType: "audio/ogg", appProperties: { bobKind: "audio" } },
      { id: "b", name: 'Low Roar - "I\'ll Keep Coming".mp3', mimeType: "audio/mpeg", appProperties: { bobKind: "audio" } },
      { id: "pic", name: "cover.jpg", mimeType: "image/jpeg", appProperties: { bobKind: "lyrics", bobMedia: "leaf" } },
    ],
    {
      tracks: [
        { id: "leaf", artist: "Sub Pop", title: "The Album Leaf - The Outer Banks [OFFICIAL VIDEO]", lyricsId: "pic", lyrics: "ÿØÿà JFIF cover" },
      ],
    },
  )
  const hania = merged.find((track) => track.id === "hania")
  const leaf = merged.find((track) => track.id === "leaf")
  const caps = merged.find((track) => track.id === "caps")
  assert.equal(hania.artist, "Hania Rani")
  assert.equal(hania.title, "Leaving")
  assert.equal(leaf.artist, "The Album Leaf")
  assert.equal(leaf.title, "The Outer Banks")
  assert.equal(leaf.lyrics, "")
  assert.equal(leaf.lyricsId, "")
  assert.equal(caps.artist, "Disappeared Completely")
  assert.equal(caps.title, "Once In a While")
  assert.equal(merged.filter((track) => track.title === "I'll Keep Coming").length, 1)
  assert.equal(isLyricText("ÿØÿà JFIF"), false)
  assert.equal(lyricLines("ÿØÿà JFIF binary").length, 0)
  assert.equal(normalizeTrack({ title: "Saudade", artist: "Amália", name: "Saudade [abc].opus" }).title, "Saudade")
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

test("a cover in .bob/covers is used when the index has no cover id", () => {
  const merged = mergeLibrary(
    [
      { id: "audio-1", name: "keep.opus", mimeType: "audio/ogg", appProperties: { bobKind: "audio" } },
      { id: "cover-file", name: "audio-1.jpg", mimeType: "image/jpeg", appProperties: { bobKind: "cover", bobMedia: "audio-1" } },
    ],
    { tracks: [{ id: "audio-1", title: "I'll Keep Coming", artist: "Low Roar" }] },
  )
  assert.equal(merged[0].coverId, "cover-file")
  assert.match(coverPlaceholder(merged[0]), /^data:image\/svg\+xml/)
})

test("desktop likes stored as drive paths count on the phone", () => {
  const track = { id: "audio1", name: "keep.wav", title: "I'll Keep Coming" }
  assert.equal(trackIsLiked(["drive:audio1"], track), true)
  assert.equal(trackIsLiked(["C:/Music/keep.wav"], track), true)
  assert.equal(trackIsLiked(["drive:other"], track), false)
})
