import { useSyncExternalStore } from "react"

export type PanelTab = "queue" | "lyrics" | "library"

type Chrome = {
  open: boolean
  width: number
  tab: PanelTab
  lyrics: boolean
  karaoke: boolean
  find: boolean
}

const KEY_OPEN = "bob-baixa-panel-open"
const KEY_WIDTH = "bob-baixa-panel"
const KEY_TAB = "bob-baixa-panel-tab"

function stored(): Chrome {
  if (typeof localStorage === "undefined") return { open: true, width: 320, tab: "queue", lyrics: false, karaoke: false, find: false }
  const open = localStorage.getItem(KEY_OPEN)
  const width = Number(localStorage.getItem(KEY_WIDTH)) || 320
  const tab = localStorage.getItem(KEY_TAB)
  const demo = typeof location !== "undefined" ? new URLSearchParams(location.search).get("demo") : ""
  const base: Chrome = {
    open: open == null ? true : open === "1",
    width: Math.min(520, Math.max(240, width)),
    tab: tab === "lyrics" || tab === "library" || tab === "queue" ? tab : "queue",
    lyrics: false,
    karaoke: false,
    find: false,
  }
  if (demo === "letra" || demo === "lyrics") base.tab = "lyrics"
  if (demo === "lyrics") base.lyrics = true
  if (demo === "biblioteca") base.tab = "library"
  if (demo === "fila") base.tab = "queue"
  if (demo === "home" || demo === "letra" || demo === "lyrics" || demo === "biblioteca" || demo === "fila") base.open = true
  return base
}

let state = stored()
const listeners = new Set<() => void>()

function emit() {
  listeners.forEach((listener) => listener())
}

function save() {
  if (typeof localStorage === "undefined") return
  localStorage.setItem(KEY_OPEN, state.open ? "1" : "0")
  localStorage.setItem(KEY_WIDTH, String(state.width))
  localStorage.setItem(KEY_TAB, state.tab)
}

export function useChrome() {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    () => state,
  )
}

export function setChrome(patch: Partial<Chrome>) {
  state = { ...state, ...patch }
  save()
  emit()
}

export function togglePanel() {
  setChrome({ open: !state.open })
}

export function pressLyrics() {
  if (state.lyrics) {
    setChrome({ lyrics: false })
    return
  }
  if (!state.open || state.tab !== "lyrics") {
    setChrome({ open: true, tab: "lyrics" })
    return
  }
  setChrome({ lyrics: true })
}

export function setPanelWidth(width: number) {
  setChrome({ width: Math.min(520, Math.max(240, width)) })
}
