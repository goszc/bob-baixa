import { useEffect, useState } from "react"
import { isTauri } from "../lib/api"
import { initApp } from "../store"

interface MiniState {
  title: string
  artist: string
  cover: string
  line: string
  playing: boolean
  currentTime: number
  duration: number
  liked?: boolean
  buffering?: boolean
}

const empty: MiniState = { title: "", artist: "", cover: "", line: "", playing: false, currentTime: 0, duration: 0 }

const sample: MiniState = {
  title: "Noite acesa no meio da cidade",
  artist: "Banda Demo",
  cover: "",
  line: "Som que me leva",
  playing: true,
  currentTime: 12,
  duration: 180,
  liked: true,
}

type Size = "strip" | "square"

export function MiniPlayer() {
  const demo = !isTauri()
  const [state, setState] = useState<MiniState>(demo ? sample : empty)
  const [size, setSize] = useState<Size>(() => (localStorage.getItem("bob-mini-size") === "square" ? "square" : "strip"))
  const [opacity, setOpacity] = useState(() => {
    const raw = Number(localStorage.getItem("bob-mini-opacity") || "1")
    return Number.isFinite(raw) ? Math.min(1, Math.max(0.45, raw)) : 1
  })
  useEffect(() => {
    initApp()
    if (!isTauri()) return
    let stop = () => {}
    let alive = true
    void import("@tauri-apps/api/event").then(({ listen }) =>
      listen<MiniState>("mini-state", (event) => setState({ ...empty, ...event.payload })).then((unlisten) => {
        if (alive) stop = unlisten
        else unlisten()
      }),
    )
    void restoreWindow()
    return () => {
      alive = false
      stop()
    }
  }, [])
  useEffect(() => {
    localStorage.setItem("bob-mini-size", size)
    if (!isTauri()) return
    void resize(size)
  }, [size])
  const progress = state.duration > 0 ? Math.min(100, (state.currentTime / state.duration) * 100) : 0
  return (
    <div
      className="flex h-screen flex-col overflow-hidden rounded-2xl bg-canvas/90 text-ink"
      style={{ opacity }}
      data-tauri-drag-region
    >
      <div className={size === "square" ? "flex min-h-0 flex-1 flex-col gap-2 p-3" : "flex min-h-0 flex-1 items-center gap-3 px-3"} data-tauri-drag-region>
        <div className={size === "square" ? "aspect-square w-full overflow-hidden rounded-xl bg-soft" : "h-14 w-14 shrink-0 overflow-hidden rounded-xl bg-soft"}>
          {state.cover ? <img src={state.cover} alt="" className="h-full w-full object-cover" /> : <div className="h-full w-full bg-[var(--player-accent,#2a2a28)]" />}
        </div>
        <div className="min-w-0 flex-1" data-tauri-drag-region>
          <p className="marquee text-[14px] font-medium">
            <span>{state.title || "bob baixa"}</span>
          </p>
          <p className="truncate text-[12px] text-muted">{state.buffering ? "carregando…" : state.line || state.artist}</p>
        </div>
        {size === "strip" ? <Controls state={state} /> : null}
      </div>
      {size === "square" ? (
        <div className="flex items-center justify-between px-3 pb-1">
          <Controls state={state} />
        </div>
      ) : null}
      <button
        type="button"
        aria-label="posição"
        className="mx-3 mb-2 h-1 overflow-hidden rounded-full bg-soft"
        onClick={(event) => {
          const rect = event.currentTarget.getBoundingClientRect()
          const ratio = (event.clientX - rect.left) / rect.width
          void send(`seek:${Math.max(0, ratio * (state.duration || 0))}`)
        }}
      >
        <span className="block h-full bg-ink" style={{ width: `${progress}%` }} />
      </button>
      <div className="flex items-center gap-2 px-3 pb-2 text-[11px] text-muted">
        <button type="button" className="rounded-full bg-soft px-2 py-0.5" onClick={() => setSize((value) => (value === "strip" ? "square" : "strip"))}>
          {size === "strip" ? "quadrado" : "faixa"}
        </button>
        <label className="flex items-center gap-1">
          opacidade
          <input
            aria-label="opacidade"
            type="range"
            min={0.45}
            max={1}
            step={0.05}
            value={opacity}
            onChange={(event) => {
              const next = Number(event.target.value)
              setOpacity(next)
              localStorage.setItem("bob-mini-opacity", String(next))
            }}
          />
        </label>
        <button type="button" className="ml-auto" aria-label="fechar" onClick={() => void closeMini()}>
          fechar
        </button>
      </div>
      <style>{`.marquee{overflow:hidden;white-space:nowrap}.marquee span{display:inline-block;animation:bob-marquee 14s ease-in-out infinite}@keyframes bob-marquee{0%,18%{transform:translateX(0)}82%,100%{transform:translateX(-40%)}}`}</style>
    </div>
  )
}

function Controls({ state }: { state: MiniState }) {
  return (
    <div className="flex items-center gap-1">
      <MiniButton action="previous" label="anterior">
        ‹
      </MiniButton>
      <MiniButton action="toggle" label={state.playing ? "pausar" : "tocar"}>
        {state.playing ? "❚❚" : "▶"}
      </MiniButton>
      <MiniButton action="next" label="próxima">
        ›
      </MiniButton>
      <MiniButton action="like" label={state.liked ? "descurtir" : "curtir"}>
        {state.liked ? "♥" : "♡"}
      </MiniButton>
      <MiniButton action="expand" label="abrir">
        ↗
      </MiniButton>
    </div>
  )
}

function MiniButton({ action, label, children }: { action: string; label: string; children: string }) {
  return (
    <button type="button" aria-label={label} title={label} className="h-8 w-8 rounded-full text-[14px] hover:bg-soft" onClick={() => void send(action)}>
      {children}
    </button>
  )
}

async function send(action: string) {
  if (!isTauri()) return
  const { emit } = await import("@tauri-apps/api/event")
  await emit("mini-control", action)
}

async function closeMini() {
  if (!isTauri()) return
  await rememberWindow()
  const { invoke } = await import("@tauri-apps/api/core")
  await invoke("close_mini")
}

async function resize(size: Size) {
  const { getCurrentWindow } = await import("@tauri-apps/api/window")
  const { LogicalSize } = await import("@tauri-apps/api/dpi")
  const window = getCurrentWindow()
  await window.setSize(size === "square" ? new LogicalSize(300, 360) : new LogicalSize(460, 120))
}

async function rememberWindow() {
  if (!isTauri()) return
  const { getCurrentWindow } = await import("@tauri-apps/api/window")
  const window = getCurrentWindow()
  const position = await window.outerPosition()
  const bounds = await window.outerSize()
  localStorage.setItem("bob-mini-bounds", JSON.stringify({ x: position.x, y: position.y, width: bounds.width, height: bounds.height }))
}

async function restoreWindow() {
  const raw = localStorage.getItem("bob-mini-bounds")
  if (!raw) return
  try {
    const saved = JSON.parse(raw) as { x: number; y: number }
    const { getCurrentWindow } = await import("@tauri-apps/api/window")
    const { LogicalPosition } = await import("@tauri-apps/api/dpi")
    await getCurrentWindow().setPosition(new LogicalPosition(saved.x, saved.y))
  } catch {
    /* a bad saved position stays at the default */
  }
}
