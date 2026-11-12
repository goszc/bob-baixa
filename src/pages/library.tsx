import { useMemo, useState } from "react"
import { Shuffle } from "lucide-react"
import { TrackTable } from "../components/track-table"
import { Thumb } from "../components/thumb"
import { type Track } from "../lib/library"
import { playTracks } from "../lib/player"
import { useLibraryTracks } from "../lib/use-library"
import { cn } from "../lib/cn"
import { useT } from "../store"

type View = "tracks" | "artists" | "albums"

export function LibraryPage() {
  const tracks = useLibraryTracks()
  const t = useT()
  const [view, setView] = useState<View>("tracks")
  const [group, setGroup] = useState<string | null>(null)
  const groups = useMemo(() => groupOf(tracks, view === "artists" ? "artist" : "album"), [tracks, view])
  const shown = group ? tracks.filter((track) => (view === "artists" ? track.artist : track.album) === group) : tracks

  return (
    <div className="flex h-full min-h-0 flex-col">
      <header className="flex flex-wrap items-end justify-between gap-4 px-6 pt-8 pb-4 md:px-8">
        <div>
          <h1 className="text-[28px] font-medium tracking-[-0.04em]">{t("nav_library")}</h1>
          <p className="mt-1 text-[13px] text-muted">{tracks.length}</p>
        </div>
        <div className="flex items-center gap-2">
          <button type="button" className="h-10 rounded-full px-4 text-[14px] font-medium" style={{ background: "var(--accent)", color: "#101010" }} onClick={() => tracks.length && playTracks(tracks, 0)}>
            {t("play_all")}
          </button>
          <button
            type="button"
            className="inline-flex h-10 items-center gap-2 rounded-full border border-line px-4 text-[14px]"
            onClick={() => {
              if (!tracks.length) return
              const index = Math.floor(Math.random() * tracks.length)
              playTracks(tracks, index)
            }}
          >
            <Shuffle className="h-4 w-4" />
            {t("shuffle_play")}
          </button>
        </div>
      </header>
      <div className="flex gap-1 px-6 md:px-8" role="tablist">
        {(["tracks", "artists", "albums"] as View[]).map((item) => (
          <button
            key={item}
            type="button"
            role="tab"
            aria-selected={view === item}
            className={cn("h-8 rounded-full px-3 text-[13px]", view === item ? "bg-soft text-ink" : "text-muted")}
            onClick={() => {
              setView(item)
              setGroup(null)
            }}
          >
            {t(item === "tracks" ? "player_tracks" : item === "artists" ? "player_artists" : "player_albums")}
          </button>
        ))}
      </div>
      <div className="min-h-0 flex-1 overflow-auto px-4 py-4 md:px-6">
        {view === "tracks" || group ? (
          <TrackTable tracks={shown} />
        ) : (
          <div className="grid gap-4" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(160px, 1fr))" }}>
            {groups.map((item) => (
              <button key={item.name} type="button" className="rounded-2xl bg-soft p-3 text-left" onClick={() => setGroup(item.name)}>
                <Thumb src={item.cover} videoUrl={item.name} className="aspect-square w-full rounded-xl" />
                <span className="mt-3 block truncate text-[14px]">{item.name}</span>
                <span className="block text-[12px] text-muted">{item.count}</span>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

function groupOf(tracks: Track[], key: "artist" | "album") {
  const map = new Map<string, { name: string; count: number; cover?: string }>()
  for (const track of tracks) {
    const name = track[key] || "—"
    const current = map.get(name)
    if (current) current.count += 1
    else map.set(name, { name, count: 1, cover: track.thumbnail })
  }
  return [...map.values()]
}
