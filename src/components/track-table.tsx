import { useMemo, useState } from "react"
import { Heart, Play } from "lucide-react"
import { formatClock } from "../lib/format"
import { trackKey, type Track } from "../lib/library"
import { toggleLike, useMusic } from "../lib/music"
import { playTracks } from "../lib/player"
import { cn } from "../lib/cn"
import { useT } from "../store"
import { Thumb } from "./thumb"

type SortKey = "index" | "title" | "album" | "duration"

export function TrackTable({
  tracks,
  onPlay,
  onRemove,
  onMove,
}: {
  tracks: Track[]
  onPlay?: (index: number) => void
  onRemove?: (track: Track) => void
  onMove?: (index: number, delta: number) => void
}) {
  const t = useT()
  const music = useMusic()
  const [sort, setSort] = useState<SortKey>("index")
  const [dir, setDir] = useState<1 | -1>(1)
  const rows = useMemo(() => {
    const copy = tracks.map((track, index) => ({ track, index }))
    if (sort === "index") return dir === 1 ? copy : [...copy].reverse()
    return copy.sort((left, right) => {
      const a = sort === "title" ? left.track.title : sort === "album" ? left.track.album : String(left.track.duration ?? 0)
      const b = sort === "title" ? right.track.title : sort === "album" ? right.track.album : String(right.track.duration ?? 0)
      return a.localeCompare(b, undefined, { numeric: true }) * dir
    })
  }, [tracks, sort, dir])

  function choose(key: SortKey) {
    if (sort === key) setDir((value) => (value === 1 ? -1 : 1))
    else {
      setSort(key)
      setDir(1)
    }
  }

  return (
    <table className="w-full border-separate border-spacing-0 text-left">
      <thead>
        <tr className="text-[12px] text-muted">
          <th className="w-12 px-2 py-2 font-medium">
            <button type="button" onClick={() => choose("index")}>#</button>
          </th>
          <th className="px-2 py-2 font-medium">
            <button type="button" onClick={() => choose("title")}>{t("col_title")}</button>
          </th>
          <th className="hidden px-2 py-2 font-medium md:table-cell">
            <button type="button" onClick={() => choose("album")}>{t("col_album")}</button>
          </th>
          <th className="w-20 px-2 py-2 text-right font-medium">
            <button type="button" onClick={() => choose("duration")}>{t("col_time")}</button>
          </th>
          <th className="w-12 px-2 py-2" />
        </tr>
      </thead>
      <tbody>
        {rows.map(({ track, index }, row) => {
          const loved = music.likes.includes(trackKey(track.path))
          return (
            <tr key={track.path} className="group border-t border-line hover:bg-soft">
              <td className="px-2 py-2 text-[13px] tabular-nums text-muted">
                <button
                  type="button"
                  aria-label={t("player_play")}
                  className="grid h-8 w-8 place-items-center"
                  onClick={() => (onPlay ? onPlay(index) : playTracks(tracks, index))}
                >
                  <span className="group-hover:hidden">{row + 1}</span>
                  <Play className="hidden h-4 w-4 group-hover:block" />
                </button>
              </td>
              <td className="px-2 py-2">
                <button type="button" className="flex min-w-0 items-center gap-3 text-left" onClick={() => (onPlay ? onPlay(index) : playTracks(tracks, index))}>
                  <Thumb src={track.thumbnail} videoUrl={track.pageUrl || track.title} className="h-10 w-10 rounded-md" />
                  <span className="min-w-0">
                    <span className="block truncate text-[14px]">{track.title}</span>
                    <span className="block truncate text-[12px] text-muted">{track.artist}</span>
                  </span>
                </button>
              </td>
              <td className="hidden truncate px-2 py-2 text-[13px] text-muted md:table-cell">{track.album}</td>
              <td className="px-2 py-2 text-right text-[13px] tabular-nums text-muted">{formatClock(track.duration)}</td>
              <td className="px-2 py-2">
                <div className="flex items-center justify-end gap-1">
                  {onMove ? (
                    <>
                      <button type="button" aria-label={t("player_more")} className="h-8 w-8 text-[12px] text-muted opacity-0 group-hover:opacity-100" onClick={() => onMove(index, -1)}>
                        ↑
                      </button>
                      <button type="button" aria-label={t("player_more")} className="h-8 w-8 text-[12px] text-muted opacity-0 group-hover:opacity-100" onClick={() => onMove(index, 1)}>
                        ↓
                      </button>
                    </>
                  ) : null}
                  {onRemove ? (
                    <button type="button" className="text-[12px] text-muted opacity-0 group-hover:opacity-100" onClick={() => onRemove(track)}>
                      {t("player_remove_from_playlist")}
                    </button>
                  ) : null}
                  <button
                    type="button"
                    aria-pressed={loved}
                    aria-label={loved ? t("player_unlike") : t("player_like")}
                    className={cn("grid h-8 w-8 place-items-center", loved ? "text-ink" : "text-muted")}
                    onClick={() => toggleLike(track.path)}
                  >
                    <Heart className="h-4 w-4" fill={loved ? "currentColor" : "none"} />
                  </button>
                </div>
              </td>
            </tr>
          )
        })}
      </tbody>
    </table>
  )
}
