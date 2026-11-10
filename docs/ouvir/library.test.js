import assert from "node:assert/strict"
import test from "node:test"
import { DRIVE_SCOPE, libraryFromFiles, libraryQuery } from "./library.js"

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
