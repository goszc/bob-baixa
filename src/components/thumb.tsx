import { useState } from "react"
import { cn } from "../lib/cn"
import { thumbSources } from "../lib/thumbs"
import { Logo } from "./logo"

export function Thumb({ src, videoUrl, className }: { src?: string; videoUrl?: string; className?: string }) {
  const sources = thumbSources(src, videoUrl)
  const [index, setIndex] = useState(0)
  const current = sources[index]
  return (
    <div className={cn("relative shrink-0 overflow-hidden rounded-xl bg-soft", className)}>
      {current ? (
        <img
          src={current}
          alt=""
          className="absolute inset-0 h-full w-full object-cover"
          onError={() => setIndex((value) => value + 1)}
        />
      ) : (
        <Logo className="absolute left-1/2 top-1/2 h-4 w-6 -translate-x-1/2 -translate-y-1/2 opacity-30" />
      )}
    </div>
  )
}
