import { useEffect, useState } from "react"
import { Shuffle } from "lucide-react"
import { TrackTable } from "../components/track-table"
import { demoMode } from "../lib/demo-library"
import { createPlaylist, deletePlaylist, moveInPlaylist, removeFromPlaylist, renamePlaylist, tracksForPaths, useMusic } from "../lib/music"
import { playTracks } from "../lib/player"
import { useLibraryTracks } from "../lib/use-library"
import { weekPaths } from "../lib/week"
import { useT } from "../store"

export function Playlists() {
  const music = useMusic()
  const tracks = useLibraryTracks()
  const t = useT()
  const [openId, setOpenId] = useState<string | null>(null)
  const [draft, setDraft] = useState("")
  const weekly = tracksForPaths(weekPaths(music.events), tracks)
  const open = music.playlists.find((playlist) => playlist.id === openId) ?? null
  const rows = open ? tracksForPaths(open.paths, tracks) : []

  useEffect(() => {
    if (demoMode() === "playlist" && music.playlists[0] && !openId) setOpenId(music.playlists[0].id)
  }, [music.playlists, openId])

  if (open) {
    return (
      <div>
        <button type="button" className="text-[13px] text-muted hover:text-ink" onClick={() => setOpenId(null)}>
          {t("nav_playlists")}
        </button>
        <header className="mt-4 flex flex-wrap items-end justify-between gap-4">
          <div>
            <input
              aria-label={t("player_playlist_name")}
              value={open.name}
              onChange={(event) => renamePlaylist(open.id, event.target.value)}
              className="bg-transparent text-[28px] font-medium tracking-[-0.04em] outline-none"
            />
            <p className="mt-1 text-[13px] text-muted">{rows.length}</p>
          </div>
          <div className="flex items-center gap-2">
            <button type="button" className="h-10 rounded-full px-4 text-[14px] font-medium" style={{ background: "var(--accent)", color: "#101010" }} onClick={() => rows.length && playTracks(rows, 0)}>
              {t("play_all")}
            </button>
            <button type="button" className="inline-flex h-10 items-center gap-2 rounded-full border border-line px-4 text-[14px]" onClick={() => rows.length && playTracks(rows, Math.floor(Math.random() * rows.length))}>
              <Shuffle className="h-4 w-4" />
              {t("shuffle_play")}
            </button>
            <button type="button" className="h-10 rounded-full border border-line px-4 text-[14px]" onClick={() => { deletePlaylist(open.id); setOpenId(null) }}>
              {t("preset_delete")}
            </button>
          </div>
        </header>
        <div className="mt-6">
          <TrackTable
            tracks={rows}
            onPlay={(index) => playTracks(rows, index)}
            onRemove={(track) => removeFromPlaylist(open.id, track.path)}
            onMove={(index, delta) => {
              const item = rows[index]
              if (item && (delta === -1 || delta === 1)) moveInPlaylist(open.id, item.path, delta)
            }}
          />
        </div>
      </div>
    )
  }

  return (
    <div>
      <h1 className="text-[28px] font-medium tracking-[-0.04em]">{t("nav_playlists")}</h1>
      <form
        className="mt-6 flex flex-wrap items-center gap-2"
        onSubmit={(event) => {
          event.preventDefault()
          const id = createPlaylist(draft || t("player_new_playlist"))
          setDraft("")
          if (id) setOpenId(id)
        }}
      >
        <input
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          aria-label={t("player_playlist_name")}
          placeholder={t("player_playlist_name")}
          className="h-10 w-[220px] rounded-full border border-line bg-surface px-4 text-[14px] outline-none"
        />
        <button type="submit" className="h-10 rounded-full px-4 text-[14px] font-medium" style={{ background: "var(--accent)", color: "#101010" }}>
          {t("player_new_playlist")}
        </button>
      </form>
      <div className="mt-6 grid gap-3">
        {weekly.length ? (
          <button type="button" className="flex h-16 items-center justify-between rounded-2xl bg-soft px-4 text-left" onClick={() => playTracks(weekly, 0)}>
            <span className="text-[15px]">{t("home_weekly")}</span>
            <span className="text-[12px] text-muted">{weekly.length}</span>
          </button>
        ) : null}
        {music.playlists.map((playlist) => (
          <button key={playlist.id} type="button" className="flex h-16 items-center justify-between rounded-2xl bg-surface px-4 text-left hover:bg-soft" onClick={() => setOpenId(playlist.id)}>
            <span className="text-[15px]">{playlist.name}</span>
            <span className="text-[12px] text-muted">{playlist.paths.length}</span>
          </button>
        ))}
      </div>
    </div>
  )
}
