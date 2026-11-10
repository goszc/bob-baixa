import { useState } from "react"
import { createPlaylist, tracksForPaths, useMusic } from "../lib/music"
import { playTracks } from "../lib/player"
import { useLibraryTracks } from "../lib/use-library"
import { weekPaths } from "../lib/week"
import { useT } from "../store"

export function Playlists() {
  const music = useMusic()
  const tracks = useLibraryTracks()
  const t = useT()
  const [name, setName] = useState("")
  const weekly = tracksForPaths(weekPaths(music.events), tracks)
  return (
    <div>
      <h1 className="text-[28px] font-medium tracking-[-0.04em]">{t("nav_playlists")}</h1>
      <button
        type="button"
        className="mt-6 flex h-16 w-full items-center justify-between rounded-2xl bg-soft px-4 text-left"
        onClick={() => weekly.length && playTracks(weekly, 0)}
      >
        <span>
          <span className="block text-[15px]">{t("home_weekly")}</span>
          <span className="text-[12px] text-muted">{weekly.length}</span>
        </span>
      </button>
      <form
        className="mt-4 flex gap-2"
        onSubmit={(event) => {
          event.preventDefault()
          if (createPlaylist(name)) setName("")
        }}
      >
        <input value={name} onChange={(event) => setName(event.target.value)} aria-label={t("nav_playlists")} className="h-10 min-w-0 flex-1 rounded-full border border-line bg-surface px-4 text-[14px] outline-none" />
      </form>
      <ul className="mt-4">
        {music.playlists.map((playlist) => {
          const list = tracksForPaths(playlist.paths, tracks)
          return (
            <li key={playlist.id}>
              <button type="button" className="flex h-14 w-full items-center justify-between rounded-xl px-2 text-left hover:bg-soft" onClick={() => list.length && playTracks(list, 0)}>
                <span className="truncate text-[14px]">{playlist.name}</span>
                <span className="text-[12px] tabular-nums text-muted">{playlist.paths.length}</span>
              </button>
            </li>
          )
        })}
      </ul>
    </div>
  )
}
