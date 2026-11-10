import { useEffect, useMemo, useState, type ReactNode } from "react"
import { groupTracks, relatedTracks, trackKey, type Track } from "../lib/library"
import { parseM3u } from "../lib/listening"
import {
  addToPlaylist,
  createPlaylist,
  deletePlaylist,
  exportManifest,
  importManifest,
  matchM3u,
  moveInPlaylist,
  playlistM3u,
  removeFromPlaylist,
  renamePlaylist,
  setCrossfade,
  setNormalize,
  setSleep,
  setView,
  toggleLike,
  tracksForPaths,
  useMusic,
  type LibraryView,
  type SleepMode,
} from "../lib/music"
import { invoke, isTauri } from "../lib/api"
import { discoverySplit } from "../lib/discover"
import { clearPlayback, playNext, playTracks, queueTracks, rememberLibrary } from "../lib/player"
import { enqueueItems, requestDeleteTrack, useApp, useT } from "../store"
import { cn } from "../lib/cn"
import { CarePanel, MixList } from "./library-care"
import { Retrospective } from "./retrospective"
import { Input } from "./ui/input"

const VIEWS: { value: LibraryView; key: "player_tracks" | "player_artists" | "player_albums" | "player_folders" | "player_playlists" | "player_likes" | "player_recent" | "player_retro" | "player_mix" | "player_care" }[] = [
  { value: "tracks", key: "player_tracks" },
  { value: "mix", key: "player_mix" },
  { value: "artists", key: "player_artists" },
  { value: "albums", key: "player_albums" },
  { value: "folders", key: "player_folders" },
  { value: "playlists", key: "player_playlists" },
  { value: "likes", key: "player_likes" },
  { value: "recent", key: "player_recent" },
  { value: "care", key: "player_care" },
  { value: "retro", key: "player_retro" },
]

export function LibraryPanel({
  tracks,
  pending = false,
  query,
  onQuery,
  renderTrack,
}: {
  tracks: Track[]
  pending?: boolean
  query: string
  onQuery: (value: string) => void
  renderTrack: (track: Track, onPlay: () => void) => ReactNode
}) {
  const music = useMusic()
  const t = useT()
  useEffect(() => {
    rememberLibrary(tracks)
  }, [tracks])
  const [group, setGroup] = useState<string | null>(null)
  const [playlistId, setPlaylistId] = useState<string | null>(null)
  const [name, setName] = useState("")
  const filtered = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase()
    if (!needle) return tracks
    return tracks.filter((track) => `${track.title} ${track.artist} ${track.album} ${track.folder}`.toLocaleLowerCase().includes(needle))
  }, [tracks, query])

  const likes = filtered.filter((track) => music.likes.includes(trackKey(track.path)))
  const recent = music.recent
    .map((item) => filtered.find((track) => trackKey(track.path) === trackKey(item.path)))
    .filter((track): track is Track => Boolean(track))
  const openPlaylist = music.playlists.find((playlist) => playlist.id === playlistId) ?? null
  const playlistTracks = openPlaylist ? tracksForPaths(openPlaylist.paths, tracks) : []

  function play(list: Track[], index: number) {
    playTracks(list, index)
  }

  function radio(track: Track) {
    const list = relatedTracks(track, tracks)
    if (list.length < 2) return
    playTracks(list, 0, true)
  }

  async function readFile(accept: string) {
    const input = document.createElement("input")
    input.type = "file"
    input.accept = accept
    const file = await new Promise<File | null>((resolve) => {
      input.onchange = () => resolve(input.files?.[0] ?? null)
      input.click()
    })
    if (!file) return null
    return file.text()
  }

  function download(filename: string, body: string) {
    const blob = new Blob([body], { type: "text/plain" })
    const url = URL.createObjectURL(blob)
    const link = document.createElement("a")
    link.href = url
    link.download = filename
    link.click()
    URL.revokeObjectURL(url)
  }

  const groups =
    music.view === "artists"
      ? groupTracks(filtered, "artist", t("player_unknown_artist"))
      : music.view === "albums"
        ? groupTracks(filtered, "album", t("player_unknown_album"))
        : music.view === "folders"
          ? groupTracks(filtered, "folder", t("player_root"))
          : []

  return (
    <div className="mt-4 flex min-h-0 flex-1 flex-col">
      <div className="mb-3 flex flex-wrap gap-1">
        {VIEWS.map((item) => (
          <button
            key={item.value}
            type="button"
            aria-pressed={music.view === item.value}
            onClick={() => {
              setView(item.value)
              setGroup(null)
              setPlaylistId(null)
            }}
            className={cn(
              "h-8 shrink-0 rounded-full px-3 text-[13px]",
              music.view === item.value ? "bg-fill text-onfill" : "bg-soft text-muted hover:text-ink",
            )}
          >
            {t(item.key)}
          </button>
        ))}
      </div>
      {music.view !== "retro" && music.view !== "playlists" ? (
        <Input value={query} onChange={(event) => onQuery(event.target.value)} placeholder={t("player_filter")} className="mb-3 h-9" />
      ) : null}
      <div className="min-h-0 flex-1 space-y-1 overflow-auto pb-4">
        {music.view === "retro" ? <Retrospective /> : null}
        {music.view === "mix" ? <MixList tracks={filtered} /> : null}
        {music.view === "care" ? <CarePanel tracks={tracks} /> : null}
        {music.view === "tracks" ? (
          filtered.length ? (
            filtered.map((track, index) => (
              <TrackRow key={track.path} track={track} library={tracks} onPlay={() => play(filtered, index)} onRadio={() => radio(track)} render={renderTrack} />
            ))
          ) : (
            <div className="px-2 py-8">
              {pending && tracks.length === 0 ? (
                <p className="text-[15px] text-muted">{t("searching")}</p>
              ) : (
                <>
                  <p className="text-[15px]">{tracks.length === 0 ? t("player_empty") : t("no_results")}</p>
                  {tracks.length === 0 ? <p className="mt-2 text-[13px] text-muted">{t("player_empty_hint")}</p> : null}
                </>
              )}
            </div>
          )
        ) : null}
        {music.view === "likes" ? (
          likes.length ? (
            likes.map((track, index) => (
              <TrackRow key={track.path} track={track} library={tracks} onPlay={() => play(likes, index)} onRadio={() => radio(track)} render={renderTrack} />
            ))
          ) : (
            <p className="px-2 py-8 text-[14px] text-muted">{t("player_no_likes")}</p>
          )
        ) : null}
        {music.view === "recent" ? (
          recent.length ? (
            recent.map((track, index) => (
              <TrackRow key={track.path} track={track} library={tracks} onPlay={() => play(recent, index)} onRadio={() => radio(track)} render={renderTrack} />
            ))
          ) : (
            <p className="px-2 py-8 text-[14px] text-muted">{t("player_no_recent")}</p>
          )
        ) : null}
        {(music.view === "artists" || music.view === "albums" || music.view === "folders") && !group
          ? groups.map((item) => (
              <button key={item.name} type="button" className="flex w-full items-center justify-between rounded-xl px-2 py-3 text-left hover:bg-soft" onClick={() => setGroup(item.name)}>
                <span className="truncate text-[14px] font-medium">{item.name}</span>
                <span className="text-[12px] tabular-nums text-muted">{item.tracks.length}</span>
              </button>
            ))
          : null}
        {group && (music.view === "artists" || music.view === "albums" || music.view === "folders") ? (
          <div>
            <button type="button" className="mb-2 text-[13px] text-muted hover:text-ink" onClick={() => setGroup(null)}>
              {t("player_back")}
            </button>
            {(groups.find((item) => item.name === group)?.tracks ?? []).map((track, index, list) => (
              <TrackRow key={track.path} track={track} library={tracks} onPlay={() => play(list, index)} onRadio={() => radio(track)} render={renderTrack} />
            ))}
          </div>
        ) : null}
        {music.view === "playlists" && !openPlaylist ? (
          <div>
            <form
              className="mb-3 flex gap-2"
              onSubmit={(event) => {
                event.preventDefault()
                const id = createPlaylist(name)
                if (id) {
                  setName("")
                  setPlaylistId(id)
                }
              }}
            >
              <Input value={name} onChange={(event) => setName(event.target.value)} placeholder={t("player_playlist_name")} className="h-9" />
              <button type="submit" className="shrink-0 rounded-full bg-fill px-3 text-[13px] text-onfill">
                {t("player_new_playlist")}
              </button>
            </form>
            {music.playlists.length === 0 ? <p className="px-2 py-6 text-[14px] text-muted">{t("player_no_playlists")}</p> : null}
            {music.playlists.map((playlist) => (
              <button key={playlist.id} type="button" className="flex w-full items-center justify-between rounded-xl px-2 py-3 text-left hover:bg-soft" onClick={() => setPlaylistId(playlist.id)}>
                <span className="truncate text-[14px] font-medium">{playlist.name}</span>
                <span className="text-[12px] tabular-nums text-muted">{playlist.paths.length}</span>
              </button>
            ))}
            <div className="mt-4 flex flex-wrap gap-2 px-1">
              <TextButton
                label={t("player_import_m3u")}
                onClick={() => {
                  void readFile(".m3u,.m3u8,audio/x-mpegurl").then((text) => {
                    if (!text) return
                    const id = createPlaylist("importada")
                    if (!id) return
                    for (const path of matchM3u(parseM3u(text), tracks)) addToPlaylist(id, path)
                    setPlaylistId(id)
                  })
                }}
              />
              <TextButton
                label={t("player_import_manifest")}
                onClick={() => {
                  void readFile(".json,application/json").then((text) => {
                    if (!text) return
                    importManifest(text)
                  })
                }}
              />
              <TextButton label={t("player_export_manifest")} onClick={() => download("bob-baixa-biblioteca.json", exportManifest())} />
            </div>
          </div>
        ) : null}
        {music.view === "playlists" && openPlaylist ? (
          <div>
            <button type="button" className="mb-2 text-[13px] text-muted hover:text-ink" onClick={() => setPlaylistId(null)}>
              {t("player_back")}
            </button>
            <input
              aria-label={t("player_playlist_name")}
              defaultValue={openPlaylist.name}
              className="mb-3 h-9 w-full rounded-xl bg-transparent px-2 text-[16px] font-medium outline-none"
              onBlur={(event) => renamePlaylist(openPlaylist.id, event.target.value)}
            />
            {playlistTracks.map((track, index) => (
              <div key={track.path} className="flex items-center gap-1">
                <div className="min-w-0 flex-1">{renderTrack(track, () => play(playlistTracks, index))}</div>
                <button type="button" className="text-[12px] text-muted" aria-label={t("player_move_up")} onClick={() => moveInPlaylist(openPlaylist.id, track.path, -1)}>
                  ↑
                </button>
                <button type="button" className="text-[12px] text-muted" aria-label={t("player_move_down")} onClick={() => moveInPlaylist(openPlaylist.id, track.path, 1)}>
                  ↓
                </button>
                <button type="button" className="text-[12px] text-muted" onClick={() => removeFromPlaylist(openPlaylist.id, track.path)}>
                  {t("player_remove_from_playlist")}
                </button>
              </div>
            ))}
            <div className="mt-3 flex flex-wrap gap-2">
              <TextButton label={t("player_export_m3u")} onClick={() => download(`${openPlaylist.name}.m3u`, playlistM3u(playlistTracks))} />
              <TextButton
                label={t("preset_delete")}
                onClick={() => {
                  deletePlaylist(openPlaylist.id)
                  setPlaylistId(null)
                }}
              />
            </div>
          </div>
        ) : null}
        {filtered.length === 0 && music.view === "tracks" ? null : null}
      </div>
      <Studio tracks={tracks} />
      <ListeningTools />
      {music.view === "tracks" && filtered.length === 0 ? null : null}
    </div>
  )
}

function TrackRow({
  track,
  library,
  onPlay,
  onRadio,
  render,
}: {
  track: Track
  library: Track[]
  onPlay: () => void
  onRadio: () => void
  render: (track: Track, onPlay: () => void) => ReactNode
}) {
  const t = useT()
  const music = useMusic()
  const [open, setOpen] = useState(false)
  const loved = music.likes.includes(trackKey(track.path))
  const related = relatedTracks(track, library)
  return (
    <div className="flex items-center gap-1">
      <div className="min-w-0 flex-1">{render(track, onPlay)}</div>
      <button type="button" aria-pressed={loved} aria-label={loved ? t("player_unlike") : t("player_like")} className={cn("px-1 text-[14px]", loved ? "text-ink" : "text-muted")} onClick={() => toggleLike(track.path)}>
        ♥
      </button>
      <div className="relative">
        <button type="button" aria-label={t("player_more")} aria-expanded={open} className="px-2 text-[16px] text-muted" onClick={() => setOpen((value) => !value)}>
          ···
        </button>
        {open ? (
          <div className="absolute right-0 z-20 w-48 rounded-xl border border-line bg-surface p-1 shadow-sm">
            <MenuItem label={t("player_play_next")} onClick={() => { queueTracks([track], "next"); setOpen(false) }} />
            <MenuItem label={t("player_add_queue")} onClick={() => { queueTracks([track], "end"); setOpen(false) }} />
            <MenuItem label={t("player_offline")} onClick={() => { void invoke("set_offline", { path: track.path, on: true }); setOpen(false) }} />
            <MenuItem
              label={t("player_skip")}
              onClick={() => {
                const raw = JSON.parse(localStorage.getItem("bob-dislike") || "[]") as string[]
                localStorage.setItem("bob-dislike", JSON.stringify([...raw, track.path]))
                playNext()
                setOpen(false)
              }}
            />
            <MenuItem
              label={t("player_radio")}
              onClick={() => {
                if (related.length < 2) return
                onRadio()
                setOpen(false)
              }}
            />
            {music.playlists.map((playlist) => (
              <MenuItem key={playlist.id} label={`${t("player_add_to_playlist")}: ${playlist.name}`} onClick={() => { addToPlaylist(playlist.id, track.path); setOpen(false) }} />
            ))}
            <MenuItem label={t("delete_track")} onClick={() => { requestDeleteTrack(track.path, track.title); setOpen(false) }} />
          </div>
        ) : null}
      </div>
    </div>
  )
}

function MenuItem({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button type="button" className="block w-full rounded-lg px-3 py-2 text-left text-[13px] hover:bg-soft" onClick={onClick}>
      {label}
    </button>
  )
}

function TextButton({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button type="button" className="text-[13px] text-muted hover:text-ink" onClick={onClick}>
      {label}
    </button>
  )
}

function Studio({ tracks }: { tracks: Track[] }) {
  const t = useT()
  const app = useApp()
  const [ask, setAsk] = useState("")
  const [note, setNote] = useState("")
  const rows = tracks.map((track) => ({ path: track.path, artist: track.artist, title: track.title, album: track.album, plays: 1 }))

  function playPaths(paths: string[]) {
    const list = paths.map((path) => tracks.find((track) => track.path === path)).filter((track): track is Track => Boolean(track))
    if (list.length) playTracks(list, 0)
  }

  async function mood(name: string) {
    if (!isTauri()) return
    const paths = await invoke<string[]>("mood_mix", { mood: name, library: rows })
    playPaths(paths)
  }

  async function askQueue() {
    if (!isTauri() || !ask.trim()) return
    const paths = await invoke<string[]>("natural_queue", { text: ask, library: rows })
    playPaths(paths)
    setNote(paths.length ? "" : t("lyrics_empty"))
  }

  async function discover() {
    const plan = discoverySplit(Math.min(tracks.length, 12), app.config.discoverRatio)
    const known = tracks.slice(0, plan.library)
    if (known.length) playTracks(known, 0)
    if (!isTauri()) return
    const artists = [...new Set(tracks.map((track) => track.artist).filter(Boolean))].slice(0, 8)
    const queries = await invoke<string[]>("discover_queries", { artists })
    await enqueueItems(
      queries.slice(0, plan.fresh || 2).map((query) => ({
        url: `ytsearch1:${query}`,
        title: query,
        audioOnly: true,
        audioFormat: "opus",
        audioBitrate: "24k",
      })),
    )
    setNote(t("player_discover"))
  }

  return (
    <div className="mt-3 space-y-2">
      <div className="flex flex-wrap gap-2">
        <button type="button" className="h-7 rounded-full bg-fill px-3 text-[12px] text-onfill" onClick={() => void discover()}>
          {t("player_discover")}
        </button>
        <button type="button" className="h-7 rounded-full bg-soft px-3 text-[12px] text-muted" onClick={() => void mood(t("player_mood_study"))}>
          {t("player_mood_study")}
        </button>
        <button type="button" className="h-7 rounded-full bg-soft px-3 text-[12px] text-muted" onClick={() => void mood(t("player_mood_train"))}>
          {t("player_mood_train")}
        </button>
      </div>
      <form
        className="flex gap-2"
        onSubmit={(event) => {
          event.preventDefault()
          void askQueue()
        }}
      >
        <input value={ask} onChange={(event) => setAsk(event.target.value)} placeholder={t("player_ask")} className="h-9 min-w-0 flex-1 rounded-xl border border-line bg-surface px-3 text-[13px]" />
      </form>
      {note ? <p className="text-[12px] text-muted">{note}</p> : null}
    </div>
  )
}

function ListeningTools() {
  const t = useT()
  const music = useMusic()
  const sleep = music.sleepMode
  return (
    <div className="mt-2 space-y-2 border-t border-line pt-3">
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-[12px] text-muted">{t("player_crossfade")}</span>
        {[0, 2, 3].map((seconds) => (
          <button
            key={seconds}
            type="button"
            aria-pressed={music.crossfade === seconds}
            className={cn("h-7 rounded-full px-2 text-[12px]", music.crossfade === seconds ? "bg-fill text-onfill" : "bg-soft text-muted")}
            onClick={() => setCrossfade(seconds)}
          >
            {seconds === 0 ? t("player_off") : `${seconds}s`}
          </button>
        ))}
      </div>
      <label className="flex items-center justify-between gap-3 text-[13px]">
        <span>
          {t("player_normalize")}
          <span className="mt-0.5 block text-[12px] text-muted">{t("player_normalize_hint")}</span>
        </span>
        <input type="checkbox" checked={music.normalize} onChange={(event) => setNormalize(event.target.checked)} />
      </label>
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-[12px] text-muted">{t("player_sleep")}</span>
        {(["off", "15", "30", "45", "60", "end"] as SleepMode[]).map((mode) => (
          <button
            key={mode}
            type="button"
            aria-pressed={sleep === mode}
            className={cn("h-7 rounded-full px-2 text-[12px]", sleep === mode ? "bg-fill text-onfill" : "bg-soft text-muted")}
            onClick={() => setSleep(mode)}
          >
            {mode === "off" ? t("player_off") : mode === "end" ? t("player_sleep_end") : t("player_sleep_min", { n: mode })}
          </button>
        ))}
      </div>
      <button type="button" className="text-[12px] text-muted hover:text-ink" onClick={clearPlayback}>
        {t("player_clear_queue")}
      </button>
    </div>
  )
}
