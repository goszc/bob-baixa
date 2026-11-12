import { useSyncExternalStore } from "react"

let current: string | null = null
const listeners = new Set<() => void>()

function emit() {
  listeners.forEach((listener) => listener())
}

export function openMenu(id: string) {
  current = current === id ? null : id
  emit()
}

export function closeMenus() {
  if (!current) return
  current = null
  emit()
}

export function useMenu(id: string) {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    () => current === id,
  )
}
