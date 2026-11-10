import assert from "node:assert/strict"
import test from "node:test"
import { aiJobs, humanAiError, runAi } from "./ai-activity.ts"

test("ai errors stay readable", () => {
  assert.equal(humanAiError(new Error("sem chave do gemini")), "sem chave do gemini")
  assert.equal(humanAiError("network down"), "sem internet")
  assert.equal(humanAiError("cota do modelo gemini-3.5-flash-lite atingida. libera em 60s"), "cota do modelo gemini-3.5-flash-lite atingida. libera em 60s")
  assert.equal(humanAiError("invalid json"), "resposta inválida")
})

test("an ai job records the failure instead of vanishing", async () => {
  await assert.rejects(() => runAi("Pensando…", "gemini-3.5-flash-lite", async () => {
    throw new Error("sem internet")
  }))
  const job = aiJobs().find((item) => item.label === "Pensando…")
  assert.equal(job?.phase, "error")
  assert.equal(job?.detail, "sem internet")
  assert.equal(job?.model, "gemini-3.5-flash-lite")
  assert.ok((job?.ended ?? 0) >= (job?.started ?? 0))
})
