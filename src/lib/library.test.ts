import assert from "node:assert/strict"
import test from "node:test"
import { collectTracks, isInsideLibrary } from "./library.ts"
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
