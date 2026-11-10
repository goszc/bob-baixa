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
}

const empty: MiniState = { title: "", artist: "", cover: "", line: "", playing: false, currentTime: 0, duration: 0 }

export function MiniPlayer() {
  const [state, setState] = useState<MiniState>(empty)
  useEffect(() => {
    initApp()
    if (!isTauri()) return
    let stop = () => {}
    let alive = true
    void import("@tauri-apps/api/event").then(({ listen }) =>
      listen<MiniState>("mini-state", (event) => setState(event.payload)).then((unlisten) => {
        if (alive) stop = unlisten
        else unlisten()
      }),
    )
    return () => {
      alive = false
      stop()
    }
  }, [])
  return (
    <div className="flex h-screen items-center gap-3 bg-canvas px-3 text-ink">
      <div className="h-16 w-16 shrink-0 overflow-hidden rounded-xl bg-soft">
        {state.cover ? <img src={state.cover} alt="" className="h-full w-full object-cover" /> : null}
      </div>
      <div className="min-w-0 flex-1">
        <p className="truncate text-[14px] font-medium">{state.title || "bob baixa"}</p>
        <p className="truncate text-[12px] text-muted">{state.line || state.artist}</p>
      </div>
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
        <button type="button" className="px-2 text-[12px] text-muted" aria-label="fechar" onClick={() => void closeMini()}>
          ×
        </button>
      </div>
    </div>
  )
}

function MiniButton({ action, label, children }: { action: string; label: string; children: string }) {
  return (
    <button type="button" aria-label={label} className="h-8 w-8 rounded-full text-[14px] hover:bg-soft" onClick={() => void send(action)}>
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
  const { invoke } = await import("@tauri-apps/api/core")
  await invoke("close_mini")
}
