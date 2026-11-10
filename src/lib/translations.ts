import { useSyncExternalStore } from "react"

export interface TranslationEntry {
  lines: string[]
  target: string
  status: "running" | "done" | "error"
  error: string
  progress: number
}

const KEY = "bob-baixa-translations"
const listeners = new Set<() => void>()
const entries = new Map<string, TranslationEntry>()

function load() {
  if (typeof localStorage === "undefined") return
  try {
    const parsed = JSON.parse(localStorage.getItem(KEY) || "{}") as Record<string, TranslationEntry>
    for (const [path, entry] of Object.entries(parsed)) {
      if (entry?.status === "done" && Array.isArray(entry.lines)) entries.set(path, { ...entry, status: "done", progress: 1 })
    }
  } catch {
    /* a bad cache starts empty */
  }
}

load()

function emit() {
  const saved: Record<string, TranslationEntry> = {}
  for (const [path, entry] of entries) {
    if (entry.status === "done") saved[path] = entry
  }
  if (typeof localStorage !== "undefined") localStorage.setItem(KEY, JSON.stringify(saved))
  listeners.forEach((listener) => listener())
}

export function translationFor(path: string) {
  return entries.get(path) || null
}

export function useTranslation(path: string) {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    () => entries.get(path) || null,
    () => null,
  )
}

export function translationProgress() {
  for (const entry of entries.values()) {
    if (entry.status === "running") return entry.progress
  }
  return 0
}

export function useTranslationProgress() {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    translationProgress,
    () => 0,
  )
}

export function startTranslation(path: string, lines: string[], target: string) {
  const current = entries.get(path)
  if (!path || !lines.length) return
  if (current?.status === "done" && current.target === target) return
  if (current?.status === "running" && current.target === target) return
  entries.set(path, { lines: current?.lines || [], target, status: "running", error: "", progress: 0.08 })
  emit()
  const timer = setInterval(() => {
    const row = entries.get(path)
    if (!row || row.status !== "running") {
      clearInterval(timer)
      return
    }
    row.progress = Math.min(0.9, row.progress + 0.07)
    emit()
  }, 400)
  const desktop = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window
  const run = desktop
    ? import("./api").then(({ invoke }) => invoke<string[]>("translate_lines", { lines, target }))
    : Promise.resolve(lines.map((line) => line))
  void run
    .then((next) => {
      clearInterval(timer)
      entries.set(path, { lines: next, target, status: "done", error: "", progress: 1 })
      emit()
    })
    .catch((error: Error) => {
      clearInterval(timer)
      entries.set(path, { lines: [], target, status: "error", error: error.message || "erro", progress: 0 })
      emit()
    })
}
