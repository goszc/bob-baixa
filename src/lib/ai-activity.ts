import { useSyncExternalStore } from "react"

export type AiPhase = "running" | "done" | "error" | "cancelled"

export interface AiJob {
  id: string
  label: string
  phase: AiPhase
  model: string
  started: number
  ended: number
  detail: string
}

let jobs: AiJob[] = []
let nextId = 1
const listeners = new Set<() => void>()
const stops = new Map<string, () => void>()

function emit() {
  listeners.forEach((listener) => listener())
}

function snapshot() {
  return jobs
}

export function aiJobs() {
  return jobs
}

export function useAiJobs() {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    snapshot,
    snapshot,
  )
}

export function cancelAi(id: string) {
  stops.get(id)?.()
  jobs = jobs.map((job) => (job.id === id && job.phase === "running" ? { ...job, phase: "cancelled", ended: Date.now(), detail: "cancelado" } : job))
  emit()
}

export function humanAiError(error: unknown) {
  const raw = error instanceof Error ? error.message : String(error ?? "")
  const text = raw.toLowerCase()
  if (!raw.trim()) return "não deu certo"
  if (text.includes("sem chave") || text.includes("cole a chave")) return "sem chave do gemini"
  if (text.includes("sem internet") || text.includes("network") || text.includes("failed to fetch") || text.includes("offline")) return "sem internet"
  if (text.includes("cota") || text.includes("429") || text.includes("quota") || text.includes("modelo")) return raw.replace(/^Error:\s*/, "")
  if (text.includes("json") || text.includes("inválid") || text.includes("invalid")) return "resposta inválida"
  if (text.includes("cancel")) return "cancelado"
  return raw.replace(/^Error:\s*/, "")
}

export async function runAi<T>(label: string, model: string, task: () => Promise<T>, summary?: (value: T) => string): Promise<T> {
  const id = String(nextId++)
  const job: AiJob = { id, label, phase: "running", model, started: Date.now(), ended: 0, detail: "" }
  let cancelled = false
  stops.set(id, () => {
    cancelled = true
  })
  jobs = [job, ...jobs].slice(0, 16)
  emit()
  try {
    const result = await task()
    if (cancelled) throw new Error("cancelado")
    const detail = summary ? summary(result) : ""
    jobs = jobs.map((item) => (item.id === id ? { ...item, phase: "done", ended: Date.now(), detail } : item))
    emit()
    return result
  } catch (error) {
    const message = humanAiError(error)
    jobs = jobs.map((item) =>
      item.id === id ? { ...item, phase: cancelled || message === "cancelado" ? "cancelled" : "error", ended: Date.now(), detail: message } : item,
    )
    emit()
    throw new Error(message)
  } finally {
    stops.delete(id)
  }
}
