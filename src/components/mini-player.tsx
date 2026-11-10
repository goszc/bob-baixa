import { useEffect, useState, type ReactNode } from "react"
import { Heart, Pause, PictureInPicture2, Play, SkipBack, SkipForward, Square } from "lucide-react"
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
  title: "I'll Keep Coming",
  artist: "Low Roar",
  cover: "",
  line: "",
  playing: true,
  currentTime: 42,
  duration: 180,
  liked: true,
}

type Size = "strip" | "square"

export function MiniPlayer() {
  const demo = !isTauri()
  const params = new URLSearchParams(window.location.search)
  const [state, setState] = useState<MiniState>(demo ? sample : empty)
  const [size, setSize] = useState<Size>(() => (params.get("size") === "square" || localStorage.getItem("bob-mini-size") === "square" ? "square" : "strip"))
  const [opacity, setOpacity] = useState(() => {
    const raw = Number(localStorage.getItem("bob-mini-opacity") || "1")
    return raw === 0.7 || raw === 0.85 ? raw : 1
  })
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null)
  useEffect(() => {
    const root = document.documentElement
    root.classList.add("dark")
    root.style.background = "transparent"
    document.body.style.background = "transparent"
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
  const title = state.title || "bob baixa"
  const subtitle = state.buffering ? "carregando…" : state.line || state.artist

  return (
    <div
      className="relative h-screen overflow-hidden rounded-2xl bg-[#161616] text-[#f3f3f1]"
      style={{ opacity }}
      onContextMenu={(event) => {
        event.preventDefault()
        setMenu({ x: event.clientX, y: event.clientY })
      }}
      onClick={() => setMenu(null)}
    >
      <div data-tauri-drag-region className="absolute inset-0" />
      <div className={size === "square" ? "pointer-events-none relative flex h-full flex-col gap-3 p-3 pb-16" : "pointer-events-none relative flex h-full items-center gap-3 px-3 pb-3"}>
        <div className={size === "square" ? "min-h-0 w-full flex-1 overflow-hidden rounded-xl bg-[#222]" : "h-12 w-12 shrink-0 overflow-hidden rounded-lg bg-[#222]"}>
          {state.cover ? <img src={state.cover} alt="" className="h-full w-full object-cover" /> : <div className="h-full w-full bg-[#2a2a28]" />}
        </div>
        <div className={size === "square" ? "min-w-0" : "min-w-0 flex-1 pr-[200px]"}>
          <p className="truncate text-[14px] font-medium">{title}</p>
          <p className="truncate text-[12px] text-[#9c9c98]">{subtitle}</p>
        </div>
      </div>
      <div className={size === "square" ? "absolute right-3 bottom-4 left-3 z-10 flex items-center justify-between" : "absolute top-1/2 right-2 z-10 flex -translate-y-1/2 items-center"}>
        <MiniButton action="previous" label="anterior">
          <SkipBack className="h-4 w-4" />
        </MiniButton>
        <MiniButton action="toggle" label={state.playing ? "pausar" : "tocar"}>
          {state.playing ? <Pause className="h-4 w-4" /> : <Play className="h-4 w-4" />}
        </MiniButton>
        <MiniButton action="next" label="próxima">
          <SkipForward className="h-4 w-4" />
        </MiniButton>
        <MiniButton action="like" label={state.liked ? "descurtir" : "curtir"}>
          <Heart className="h-4 w-4" fill={state.liked ? "currentColor" : "none"} />
        </MiniButton>
        <MiniButton action="expand" label="abrir">
          <PictureInPicture2 className="h-4 w-4" />
        </MiniButton>
        <MiniButton action="size" label={size === "strip" ? "quadrado" : "faixa"} onClick={() => setSize((value) => (value === "strip" ? "square" : "strip"))}>
          <Square className="h-4 w-4" />
        </MiniButton>
      </div>
      <button
        type="button"
        aria-label="posição"
        className="absolute right-3 bottom-1.5 left-3 z-10 h-1 overflow-hidden rounded-full bg-white/15"
        onClick={(event) => {
          event.stopPropagation()
          const rect = event.currentTarget.getBoundingClientRect()
          const ratio = (event.clientX - rect.left) / rect.width
          void send(`seek:${Math.max(0, ratio * (state.duration || 0))}`)
        }}
      >
        <span className="block h-full bg-white" style={{ width: `${progress}%` }} />
      </button>
      {menu ? (
        <div className="absolute z-20 min-w-36 rounded-xl border border-white/10 bg-[#1c1c1c] p-1 text-[13px] shadow-lg" style={{ left: Math.min(menu.x, 280), top: Math.min(menu.y, 48) }}>
          {[1, 0.85, 0.7].map((value) => (
            <button
              key={value}
              type="button"
              className="block w-full rounded-lg px-3 py-1.5 text-left hover:bg-white/10"
              onClick={(event) => {
                event.stopPropagation()
                setOpacity(value)
                localStorage.setItem("bob-mini-opacity", String(value))
                setMenu(null)
              }}
            >
              {value === 1 ? "opacidade 100%" : value === 0.85 ? "opacidade 85%" : "opacidade 70%"}
            </button>
          ))}
          <button
            type="button"
            className="block w-full rounded-lg px-3 py-1.5 text-left hover:bg-white/10"
            onClick={(event) => {
              event.stopPropagation()
              void closeMini()
            }}
          >
            fechar
          </button>
        </div>
      ) : null}
    </div>
  )
}

function MiniButton({ action, label, children, onClick }: { action: string; label: string; children: ReactNode; onClick?: () => void }) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className="grid h-8 w-8 place-items-center rounded-full text-[#f3f3f1] hover:bg-white/10"
      onClick={(event) => {
        event.stopPropagation()
        if (onClick) onClick()
        else void send(action)
      }}
    >
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
  await window.setSize(size === "square" ? new LogicalSize(280, 340) : new LogicalSize(440, 88))
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
