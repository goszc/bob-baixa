import assert from "node:assert/strict"
import test from "node:test"
import { startTranslation, translationFor } from "./translations.ts"

test("a translation stays after the view that started it is gone", async () => {
  startTranslation("/library/one.mp3", ["keep coming"], "pt")
  await new Promise((resolve) => setTimeout(resolve, 20))
  const saved = translationFor("/library/one.mp3")
  assert.equal(saved?.status, "done")
  assert.deepEqual(saved?.lines, ["keep coming"])
  startTranslation("/library/one.mp3", ["keep coming"], "pt")
  assert.equal(translationFor("/library/one.mp3")?.status, "done")
})
