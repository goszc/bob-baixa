import { useState } from "react"
import { LibraryPanel } from "../components/library-panel"
import { Thumb } from "../components/thumb"
import { useCatalogSettled } from "../lib/catalog"
import { demoMode } from "../lib/demo-library"
import { currentTrack, usePlayer } from "../lib/player"
import { useLibraryTracks } from "../lib/use-library"
import { useT } from "../store"

export function LibraryPage() {
  const t = useT()
  const player = usePlayer()
  const tracks = useLibraryTracks()
  const settled = useCatalogSettled()
  const [query, setQuery] = useState("")
  const current = currentTrack(player)
  const pending = !demoMode() && !settled && tracks.length === 0
  return (
    <div className="flex h-full min-h-0 flex-col px-6 py-6 md:px-8">
      <h1 className="shrink-0 text-[28px] font-medium tracking-[-0.04em]">{t("nav_library")}</h1>
      <div className="mt-4 min-h-0 flex-1">
        <LibraryPanel
          tracks={tracks}
          pending={pending}
          query={query}
          onQuery={setQuery}
          renderTrack={(item, onPlay) => (
            <button type="button" className="flex h-full w-full items-center gap-3 rounded-xl px-1 text-left" onClick={onPlay}>
              <Thumb src={item.thumbnail} videoUrl={item.pageUrl || item.title} className="h-10 w-10 rounded-lg" />
              <span className="min-w-0">
                <span className="block truncate text-[14px] font-medium">{item.title}</span>
                <span className="block truncate text-[12px] text-muted">{item.artist}</span>
              </span>
              {current?.path === item.path ? <span className="sr-only">{t("player_now")}</span> : null}
            </button>
          )}
        />
      </div>
    </div>
  )
}
