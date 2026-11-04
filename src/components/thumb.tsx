import { useEffect, useState } from "react"
import { cn } from "../lib/cn"
import { resolveThumb } from "../lib/thumbs"
import { Logo } from "./logo"

export function Thumb({ src, className }: { src?: string; className?: string }) {
  const [shown, setShown] = useState<string | undefined>(src && !src.startsWith("http") ? src : undefined)

  useEffect(() => {
    let alive = true
    if (!src) {
      setShown(undefined)
      return
    }
    if (!src.startsWith("http")) {
      setShown(src)
      return
    }
    void resolveThumb(src)
      .then((next) => {
        if (alive) setShown(next)
      })
      .catch(() => {
        if (alive) setShown(undefined)
      })
    return () => {
      alive = false
    }
  }, [src])

  return (
    <div className={cn("relative shrink-0 overflow-hidden rounded-xl bg-soft", className)}>
      {shown ? (
        <img src={shown} alt="" className="absolute inset-0 h-full w-full object-cover" />
      ) : (
        <Logo className="absolute left-1/2 top-1/2 h-4 w-6 -translate-x-1/2 -translate-y-1/2 opacity-30" />
      )}
    </div>
  )
}
