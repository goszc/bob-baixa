import { useEffect, useRef } from "react"
import { invoke, isTauri } from "../lib/api"
import { applyLibrarySnapshot, librarySnapshot, onLibrarySync } from "../lib/music"
import { attachMedia, getPlayer, seek, systemControl, togglePlayback } from "../lib/player"

function typingTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) return false
  const tag = target.tagName
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || target.isContentEditable
}

export function PlayerEngine() {
  const audioRef = useRef<HTMLAudioElement>(null)
  const altRef = useRef<HTMLAudioElement>(null)
  const videoRef = useRef<HTMLVideoElement>(null)
  const parkRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const audio = audioRef.current
    const alt = altRef.current
    const video = videoRef.current
    const park = parkRef.current
    if (!audio || !alt || !video || !park) return
    return attachMedia(audio, alt, video, park)
  }, [])

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.metaKey || event.ctrlKey || event.altKey || typingTarget(event.target)) return
      if (event.target instanceof HTMLElement && event.target.closest("button, a")) return
      const player = getPlayer()
      if (event.key === " ") {
        event.preventDefault()
        togglePlayback()
      } else if (event.code === "MediaPlayPause") {
        event.preventDefault()
        systemControl("toggle")
      } else if (event.key === "ArrowRight" || event.code === "MediaTrackNext") {
        event.preventDefault()
        if (event.shiftKey || event.code === "MediaTrackNext") systemControl("next")
        else seek(player.currentTime + 5)
      } else if (event.key === "ArrowLeft" || event.code === "MediaTrackPrevious") {
        event.preventDefault()
        if (event.shiftKey || event.code === "MediaTrackPrevious") systemControl("previous")
        else seek(Math.max(0, player.currentTime - 5))
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [])

  useEffect(() => {
    if (!isTauri()) return
    let stop = () => {}
    let alive = true
    void import("@tauri-apps/api/event").then(({ listen }) =>
      listen<string>("media-control", (event) => systemControl(event.payload)).then((unlisten) => {
        if (alive) stop = unlisten
        else unlisten()
      }),
    )
    return () => {
      alive = false
      stop()
    }
  }, [])

  useEffect(() => {
    if (!isTauri()) return
    const stops: Array<() => void> = []
    let alive = true
    void import("@tauri-apps/api/event").then(async ({ listen }) => {
      const { getCurrentWindow } = await import("@tauri-apps/api/window")
      if (getCurrentWindow().label !== "main") return
      return listen<string>("mini-control", (event) => systemControl(event.payload))
    }).then((unlisten) => {
      if (!unlisten) return
      if (alive) stops.push(unlisten)
      else unlisten()
    })
    void import("@tauri-apps/api/event").then(({ listen }) =>
      listen<string>("player-hotkey", (event) => systemControl(event.payload)).then((unlisten) => {
        if (alive) stops.push(unlisten)
        else unlisten()
      }),
    )
    return () => {
      alive = false
      stops.forEach((stop) => stop())
    }
  }, [])

  useEffect(() => {
    if (!isTauri()) return
    let timer = 0
    const stop = onLibrarySync(() => {
      window.clearTimeout(timer)
      timer = window.setTimeout(() => {
        void invoke("drive_push_state", { json: JSON.stringify(librarySnapshot()) }).catch(() => {})
      }, 1500)
    })
    void invoke<string | null>("drive_pull_state")
      .then((json) => {
        if (!json) return
        try {
          const remote = JSON.parse(json) as { updatedAt?: number }
          const local = librarySnapshot()
          if ((remote.updatedAt ?? 0) > local.updatedAt) applyLibrarySnapshot(json)
        } catch {
          /* a foreign file in the app folder stays untouched */
        }
      })
      .catch(() => {})
    return () => {
      window.clearTimeout(timer)
      stop()
    }
  }, [])

  return (
    <div ref={parkRef} className="pointer-events-none fixed top-0 left-0 h-px w-px overflow-hidden opacity-0">
      <audio ref={audioRef} preload="metadata" />
      <audio ref={altRef} preload="metadata" />
      <video ref={videoRef} preload="metadata" playsInline />
    </div>
  )
}
