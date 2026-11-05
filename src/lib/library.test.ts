import assert from "node:assert/strict"
import test from "node:test"
import { collectTracks, dedupeTracks, isInsideLibrary, relatedTracks, trackKey, type Track } from "./library.ts"
import type { ConvertJob, HistoryEntry, Job } from "./types.ts"

const folder = "C:\\Downloads\\bob baixa"

function job(partial: Partial<Job> & Pick<Job, "id" | "outputPath">): Job {
  return {
    url: "https://youtu.be/a",
    title: "Faixa",
    state: "done",
    progress: 1,
    createdAt: "2026-01-01T00:00:00Z",
    audioOnly: false,
    quality: "1080",
    container: "mp4",
    ...partial,
  }
}

test("a file is inside the settings folder only when it lives under that path", () => {
  assert.equal(isInsideLibrary(`${folder}\\faixa.mp3`, folder), true)
  assert.equal(isInsideLibrary("c:/downloads/bob baixa/aninhada/clipe.mp4", folder), true)
  assert.equal(isInsideLibrary("C:\\Downloads\\bob baixa extra\\faixa.mp3", folder), false)
  assert.equal(isInsideLibrary("D:\\Musica\\faixa.mp3", folder), false)
  assert.equal(isInsideLibrary(`${folder}\\faixa.mp3`, ""), false)
  assert.equal(isInsideLibrary(folder, folder), false)
})

test("windows verbatim paths still belong to the configured folder", () => {
  const root = "C:\\Users\\Gustavo\\Downloads\\bob baixa"
  const mp4 = "\\\\?\\C:\\Users\\Gustavo\\Downloads\\bob baixa\\Don't Be so Serious [D56XxiS_pJ0].mp4"
  const mp3 = `${root}\\playlist\\Low Roar - Easy Way Out.mp3`
  assert.equal(isInsideLibrary(mp4, root), true)
  assert.equal(isInsideLibrary(mp3, `\\\\?\\${root}`), true)
  assert.equal(isInsideLibrary("\\\\?\\C:\\Users\\Gustavo\\Downloads\\bob baixa extra\\a.mp4", root), false)
  assert.equal(isInsideLibrary("//?/C:/Users/Gustavo/Downloads/bob baixa/clipe.mp4", root), true)
  assert.equal(trackKey(mp4), trackKey(`${root}\\Don't Be so Serious [D56XxiS_pJ0].mp4`))
  assert.equal(trackKey("\\\\?\\UNC\\server\\share\\bob\\faixa.mp3"), trackKey("\\\\server\\share\\bob\\faixa.mp3"))
})

test("the library keeps playable files from the configured folder and drops the rest", () => {
  const jobs: Job[] = [
    job({ id: "in", outputPath: `${folder}\\Manha.mp3`, title: "Artista - Manha" }),
    job({ id: "out", outputPath: "D:\\Videos\\fora.mp4", title: "Fora" }),
    job({ id: "note", outputPath: `${folder}\\leia.txt`, title: "nota" }),
    job({ id: "aac", outputPath: `${folder}\\voz.aac`, title: "Voz" }),
    job({ id: "running", outputPath: `${folder}\\meio.mp4`, title: "Meio", state: "downloading" }),
  ]
  const history: HistoryEntry[] = [
    {
      id: "h-out",
      title: "Antigo",
      url: "https://youtu.be/old",
      path: "C:\\Users\\ana\\Desktop\\antigo.mkv",
      at: "2026-01-01T00:00:00Z",
      ok: true,
    },
    {
      id: "h-in",
      title: "Canal - Lisboa",
      url: "https://youtu.be/lis",
      path: `${folder}\\Lisboa.webm`,
      at: "2026-01-01T00:00:00Z",
      ok: true,
    },
  ]
  const convert: ConvertJob = {
    id: "c",
    input: "C:\\Temp\\a.wav",
    output: "C:\\Temp\\a.mp3",
    preset: "mp3",
    state: "done",
    progress: 1,
  }
  const inside = collectTracks(jobs, history, convert, folder).map((track) => track.path)
  assert.deepEqual(inside, [`${folder}\\Lisboa.webm`, `${folder}\\Manha.mp3`, `${folder}\\voz.aac`])
  const titles = collectTracks(jobs, history, convert, folder).map((track) => track.title)
  assert.deepEqual(titles, ["Lisboa", "Manha", "Voz"])
  assert.equal(collectTracks(jobs, history, convert, "").length, 0)
})

function track(partial: Partial<Track> & Pick<Track, "path" | "title">): Track {
  return {
    id: partial.path,
    artist: "",
    album: "",
    folder: "",
    pageUrl: "",
    kind: "audio",
    ...partial,
  }
}

test("duplicates collapse when the youtube id or the tags match", () => {
  const tracks = dedupeTracks([
    track({ path: `${folder}\\A [dQw4w9WgXcQ].opus`, title: "A", artist: "Band", size: 10 }),
    track({ path: `${folder}\\A [dQw4w9WgXcQ].mp3`, title: "A", artist: "Band", thumbnail: "t", size: 40 }),
    track({ path: `${folder}\\outro.mp3`, title: "Manha", artist: "Ana", duration: 180, size: 20 }),
    track({ path: `${folder}\\copia.mp3`, title: "Manha", artist: "Ana", duration: 181, size: 50 }),
    track({ path: `${folder}\\video.mp4`, title: "Manha", artist: "Ana", duration: 180, kind: "video" }),
  ])
  assert.deepEqual(
    tracks.map((item) => item.path),
    [`${folder}\\A [dQw4w9WgXcQ].mp3`, `${folder}\\copia.mp3`, `${folder}\\video.mp4`],
  )
})

test("local radio stays on the same artist, album or folder", () => {
  const seed = track({ path: `${folder}\\a.mp3`, title: "A", artist: "Ana", album: "Disco", folder: "Disco" })
  const list = relatedTracks(seed, [
    seed,
    track({ path: `${folder}\\b.mp3`, title: "B", artist: "Ana", album: "Outro", folder: "Outro" }),
    track({ path: `${folder}\\c.mp3`, title: "C", artist: "Outra", album: "Disco", folder: "Disco" }),
    track({ path: `${folder}\\d.mp3`, title: "D", artist: "Longe", album: "Longe", folder: "Longe" }),
  ])
  assert.equal(list[0].path, seed.path)
  assert.equal(list.length, 3)
  assert.ok(list.every((item) => item.path !== `${folder}\\d.mp3`))
})
