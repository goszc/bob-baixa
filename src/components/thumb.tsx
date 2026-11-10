import { useState } from "react"
import { cn } from "../lib/cn"
import { coverMark, thumbSources } from "../lib/thumbs"

export function Thumb({ src, videoUrl, className }: { src?: string; videoUrl?: string; className?: string }) {
  const sources = thumbSources(src, videoUrl)
  const [index, setIndex] = useState(0)
  const current = sources[index] || coverMark(videoUrl || src)
  return (
    <div className={cn("relative shrink-0 overflow-hidden rounded-xl bg-soft", className)}>
      <img
        src={current}
        alt=""
        className="absolute inset-0 h-full w-full object-cover"
        onError={() => setIndex((value) => value + 1)}
      />
    </div>
  )
}
