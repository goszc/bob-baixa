import { useSyncExternalStore } from "react"

export interface Discovery {
  title: string
  artist: string
  thumbnail?: string
  at: number
}

const KEY = "bob-baixa-discovered"
const listeners = new Set<() => void>()
let items: Discovery[] = read()

function read(): Discovery[] {
  if (typeof localStorage === "undefined") return []
  try {
    const parsed = JSON.parse(localStorage.getItem(KEY) || "[]") as Discovery[]
    return Array.isArray(parsed) ? parsed.filter((item) => item && item.title) : []
  } catch {
    return []
  }
}

function emit() {
  if (typeof localStorage !== "undefined") localStorage.setItem(KEY, JSON.stringify(items))
  listeners.forEach((listener) => listener())
}

export function rememberDiscovery(item: Omit<Discovery, "at">) {
  const title = item.title.trim()
  if (!title) return
  items = [{ ...item, title, at: Date.now() }, ...items.filter((row) => row.title !== title)].slice(0, 12)
  emit()
}

export function useDiscoveries() {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    () => items,
    () => [],
  )
}
