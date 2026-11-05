import { useEffect, useRef } from "react"
import { attachMedia, getPlayer, playNext, playPrevious, seek, togglePlayback } from "../lib/player"

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
      if (event.key === " " || event.code === "MediaPlayPause") {
        event.preventDefault()
        togglePlayback()
      } else if (event.key === "ArrowRight" || event.code === "MediaTrackNext") {
        event.preventDefault()
        if (event.shiftKey || event.code === "MediaTrackNext") playNext()
        else seek(player.currentTime + 5)
      } else if (event.key === "ArrowLeft" || event.code === "MediaTrackPrevious") {
        event.preventDefault()
        if (event.shiftKey || event.code === "MediaTrackPrevious") playPrevious()
        else seek(Math.max(0, player.currentTime - 5))
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [])

  return (
    <div ref={parkRef} className="pointer-events-none fixed top-0 left-0 h-px w-px overflow-hidden opacity-0">
      <audio ref={audioRef} preload="metadata" />
      <audio ref={altRef} preload="metadata" />
      <video ref={videoRef} preload="metadata" playsInline />
    </div>
  )
}
