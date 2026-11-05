import { useEffect, useRef } from "react"
import { attachMedia } from "../lib/player"

export function PlayerEngine() {
  const audioRef = useRef<HTMLAudioElement>(null)
  const videoRef = useRef<HTMLVideoElement>(null)
  const parkRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const audio = audioRef.current
    const video = videoRef.current
    const park = parkRef.current
    if (!audio || !video || !park) return
    return attachMedia(audio, video, park)
  }, [])

  return (
    <div ref={parkRef} className="pointer-events-none fixed top-0 left-0 h-px w-px overflow-hidden opacity-0">
      <audio ref={audioRef} preload="metadata" />
      <video ref={videoRef} preload="metadata" playsInline />
    </div>
  )
}
