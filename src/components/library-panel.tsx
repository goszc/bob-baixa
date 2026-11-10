import { useEffect, useMemo, useState, type ReactNode } from "react"
import { Ellipsis, Heart, Play } from "lucide-react"
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
  setView,
  toggleLike,
  tracksForPaths,
  useMusic,
  type LibraryView,
} from "../lib/music"
import { invoke, isTauri } from "../lib/api"
import { runAi } from "../lib/ai-activity"
import { playTracks, queueTracks, rememberLibrary } from "../lib/player"
import { go, requestDeleteTrack, useT } from "../store"
import { cn } from "../lib/cn"
import { CarePanel, MixList } from "./library-care"
import { Retrospective } from "./retrospective"
import { Input } from "./ui/input"
import { VirtualList } from "./virtual-list"

const MAIN: { value: LibraryView; key: "player_tracks" | "player_artists" | "player_albums" | "player_playlists" }[] = [
  { value: "tracks", key: "player_tracks" },
  { value: "artists", key: "player_artists" },
  { value: "albums", key: "player_albums" },
  { value: "playlists", key: "player_playlists" },
]

const MORE: { value: LibraryView; key: "player_mix" | "player_folders" | "player_likes" | "player_recent" | "player_care" | "player_retro" }[] = [
  { value: "mix", key: "player_mix" },
  { value: "folders", key: "player_folders" },
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
  const [group, setGroup] = useState<string | null>(null)
  const [playlistId, setPlaylistId] = useState<string | null>(null)
  const [name, setName] = useState("")
  const [described, setDescribed] = useState<Track[] | null>(null)
  const [more, setMore] = useState(false)
  const [density, setDensity] = useState<"compact" | "comfortable">(() => (localStorage.getItem("bob-baixa-density") === "compact" ? "compact" : "comfortable"))
  const rowHeight = density === "compact" ? 48 : 64
  useEffect(() => {
    rememberLibrary(tracks)
  }, [tracks])
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
  const mainView = MAIN.some((item) => item.value === music.view)

  function play(list: Track[], index: number) {
    playTracks(list, index)
  }

  function choose(view: LibraryView) {
    setView(view)
    setGroup(null)
    setPlaylistId(null)
    setMore(false)
  }

  async function radio(track: Track) {
    if (isTauri()) {
      try {
        const paths = await runAi(t("ai_similar"), "gemini-embedding-2", () =>
          invoke<string[]>("ai_similar", {
            path: track.path,
            tracks: tracks.slice(0, 180).map((item) => ({ path: item.path, artist: item.artist, title: item.title, album: item.album, lyrics: "" })),
          }),
          (found) => (found.length > 1 ? `${found.length - 1}` : "sem chave, usei a rádio local"),
        )
        const list = paths.map((path) => tracks.find((item) => item.path === path)).filter((item): item is Track => Boolean(item))
        if (list.length > 1) {
          playTracks(list, 0, true)
          return
        }
      } catch {
        /* artist and album still build the radio */
      }
    }
    const list = relatedTracks(track, tracks)
    playTracks(list.length ? list : [track], 0, true)
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

  const trackList = music.view === "tracks" ? (described ?? filtered) : music.view === "likes" ? likes : music.view === "recent" ? recent : null
  const groupedTracks = group ? (groups.find((item) => item.name === group)?.tracks ?? []) : null

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden">
      <div className="shrink-0 space-y-2 px-3 pt-3 pb-2">
        <Input
          value={query}
          onChange={(event) => {
            setDescribed(null)
            onQuery(event.target.value)
          }}
          placeholder={t("player_filter")}
          className="h-9"
          aria-label={t("player_filter")}
        />
        <button
          type="button"
          className="text-[12px] text-muted hover:text-ink"
          onClick={() => {
            const next = density === "compact" ? "comfortable" : "compact"
            setDensity(next)
            localStorage.setItem("bob-baixa-density", next)
          }}
        >
          {t("density_toggle")}
        </button>
        <div className="flex items-center gap-2">
          <div className="flex h-8 min-w-0 flex-1 rounded-lg bg-soft p-0.5" role="tablist" aria-label={t("player_library")}>
            {MAIN.map((item) => (
              <button
                key={item.value}
                type="button"
                role="tab"
                aria-selected={music.view === item.value}
                onClick={() => choose(item.value)}
                className={cn(
                  "h-7 min-w-0 flex-1 truncate rounded-md px-1 text-[12px] transition-colors",
                  music.view === item.value ? "bg-surface text-ink" : "text-muted hover:text-ink",
                )}
              >
                {t(item.key)}
              </button>
            ))}
          </div>
          <div className="relative">
            <button
              type="button"
              aria-label={t("player_more")}
              aria-expanded={more}
              title={t("player_more")}
              onClick={() => setMore((value) => !value)}
              className={cn("grid h-8 w-8 place-items-center rounded-lg text-muted hover:bg-soft hover:text-ink", !mainView && "bg-soft text-ink")}
            >
              <Ellipsis className="h-4 w-4" />
            </button>
            {more ? (
              <div className="absolute right-0 z-20 mt-1 w-40 rounded-xl border border-line bg-surface p-1 shadow-sm">
                {MORE.map((item) => (
                  <button
                    key={item.value}
                    type="button"
                    onClick={() => choose(item.value)}
                    className={cn("block w-full rounded-lg px-3 py-2 text-left text-[13px] hover:bg-soft", music.view === item.value && "text-ink")}
                  >
                    {t(item.key)}
                  </button>
                ))}
              </div>
            ) : null}
          </div>
        </div>
        {query.trim().length > 8 && music.view === "tracks" ? (
          <button
            type="button"
            className="text-left text-[12px] text-muted hover:text-ink"
            onClick={() => {
              if (!isTauri()) return
              void runAi(
                t("ai_describe"),
                "gemini-embedding-2",
                () =>
                  invoke<string[]>("ai_describe", {
                    query,
                    tracks: tracks.slice(0, 180).map((item) => ({ path: item.path, artist: item.artist, title: item.title, album: item.album, lyrics: "" })),
                  }),
                (found) => `${found.length}`,
              )
                .then((paths) => {
                  const list = paths.map((path) => tracks.find((item) => item.path === path)).filter((item): item is Track => Boolean(item))
                  setDescribed(list)
                  setView("tracks")
                })
                .catch(() => {})
            }}
          >
            {t("player_describe")}
          </button>
        ) : null}
      </div>
      <div className="min-h-0 flex-1" data-library-list>
        {trackList ? (
          trackList.length ? (
            <VirtualList
              count={trackList.length}
              rowHeight={rowHeight}
              className="h-full overflow-auto"
              render={(index) => (
                <TrackRow
                  index={index}
                  track={trackList[index]}
                  library={tracks}
                  onPlay={() => play(trackList, index)}
                  onRadio={() => void radio(trackList[index])}
                  render={renderTrack}
                />
              )}
            />
          ) : (
            <div className="px-4 py-8">
              {pending && tracks.length === 0 ? (
                <div className="space-y-2">
                  {Array.from({ length: 6 }, (_, index) => (
                    <div key={index} className="bone" />
                  ))}
                </div>
              ) : tracks.length === 0 ? (
                <div>
                  <p className="text-[16px] leading-relaxed">{t("empty_library")}</p>
                  <button type="button" className="mt-4 h-10 rounded-full px-4 text-[14px]" style={{ background: "var(--accent)", color: "#101010" }} onClick={() => go("discover")}>
                    {t("empty_action")}
                  </button>
                </div>
              ) : (
                <p className="text-[14px] text-muted">{t("no_results")}</p>
              )}
            </div>
          )
        ) : null}
        {music.view === "retro" ? (
          <div className="h-full overflow-auto px-2">
            <Retrospective />
          </div>
        ) : null}
        {music.view === "mix" ? (
          <div className="h-full overflow-auto">
            <MixList tracks={filtered} />
          </div>
        ) : null}
        {music.view === "care" ? (
          <div className="h-full overflow-auto">
            <CarePanel tracks={tracks} />
          </div>
        ) : null}
        {(music.view === "artists" || music.view === "albums" || music.view === "folders") && !group ? (
          <div className="h-full overflow-auto">
            {groups.map((item) => (
              <button key={item.name} type="button" className="flex h-14 w-full items-center justify-between px-4 text-left hover:bg-soft" onClick={() => setGroup(item.name)}>
                <span className="truncate text-[14px]">{item.name}</span>
                <span className="text-[12px] tabular-nums text-muted">{item.tracks.length}</span>
              </button>
            ))}
          </div>
        ) : null}
        {groupedTracks ? (
          <div className="flex h-full min-h-0 flex-col">
            <button type="button" className="shrink-0 px-4 py-2 text-left text-[12px] text-muted hover:text-ink" onClick={() => setGroup(null)}>
              {t("player_back")}
            </button>
            <VirtualList
              count={groupedTracks.length}
              rowHeight={rowHeight}
              className="min-h-0 flex-1 overflow-auto"
              render={(index) => (
                <TrackRow
                  index={index}
                  track={groupedTracks[index]}
                  library={tracks}
                  onPlay={() => play(groupedTracks, index)}
                  onRadio={() => radio(groupedTracks[index])}
                  render={renderTrack}
                />
              )}
            />
          </div>
        ) : null}
        {music.view === "playlists" && !openPlaylist ? (
          <div className="h-full overflow-auto px-3">
            <form
              className="mb-2 flex gap-2"
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
              <button type="submit" aria-label={t("player_new_playlist")} title={t("player_new_playlist")} className="grid h-9 w-9 shrink-0 place-items-center rounded-full bg-fill text-[16px] text-onfill">
                +
              </button>
            </form>
            {music.playlists.length === 0 ? <p className="px-1 py-6 text-[14px] text-muted">{t("player_no_playlists")}</p> : null}
            {music.playlists.map((playlist) => (
              <button key={playlist.id} type="button" className="flex h-14 w-full items-center justify-between text-left hover:bg-soft" onClick={() => setPlaylistId(playlist.id)}>
                <span className="truncate px-1 text-[14px]">{playlist.name}</span>
                <span className="text-[12px] tabular-nums text-muted">{playlist.paths.length}</span>
              </button>
            ))}
            <div className="mt-3 flex flex-col items-start gap-2 px-1 pb-4">
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
          <div className="flex h-full min-h-0 flex-col">
            <button type="button" className="shrink-0 px-4 py-2 text-left text-[12px] text-muted hover:text-ink" onClick={() => setPlaylistId(null)}>
              {t("player_back")}
            </button>
            <input
              aria-label={t("player_playlist_name")}
              defaultValue={openPlaylist.name}
              className="mx-3 mb-2 h-9 shrink-0 bg-transparent text-[14px] outline-none"
              onBlur={(event) => renamePlaylist(openPlaylist.id, event.target.value)}
            />
            <div className="min-h-0 flex-1 overflow-auto">
              {playlistTracks.map((track, index) => (
                <div key={track.path} className="flex items-center gap-1 pr-2">
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
            </div>
            <div className="flex shrink-0 gap-3 px-4 py-2">
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
      </div>
    </div>
  )
}

function TrackRow({
  index,
  track,
  onPlay,
  onRadio,
  render,
}: {
  index: number
  track: Track
  library: Track[]
  onPlay: () => void
  onRadio: () => void
  render: (track: Track, onPlay: () => void) => ReactNode
}) {
  const t = useT()
  const music = useMusic()
  const [open, setOpen] = useState(false)
  const [box, setBox] = useState<{ left: number; top: number } | null>(null)
  const loved = music.likes.includes(trackKey(track.path))
  function place(event: { clientX: number; clientY: number; preventDefault: () => void }) {
    event.preventDefault()
    const width = 220
    const height = 248
    setBox({
      left: Math.max(8, Math.min(event.clientX, window.innerWidth - width - 8)),
      top: Math.max(8, Math.min(event.clientY, window.innerHeight - height - 8)),
    })
    setOpen(true)
  }
  function addSomewhere() {
    const id = music.playlists[0]?.id || createPlaylist("Minhas")
    if (id) addToPlaylist(id, track.path)
    setOpen(false)
  }
  return (
    <div className="group flex h-full items-center gap-1 pr-2" onContextMenu={place}>
      <button type="button" className="grid h-8 w-8 shrink-0 place-items-center text-muted" aria-label={t("player_play")} onClick={onPlay}>
        <span className="text-[12px] tabular-nums group-hover:hidden">{index + 1}</span>
        <Play className="hidden h-4 w-4 group-hover:block" />
      </button>
      <div className="min-w-0 flex-1">{render(track, onPlay)}</div>
      <button
        type="button"
        aria-pressed={loved}
        aria-label={loved ? t("player_unlike") : t("player_like")}
        className={cn("grid h-8 w-8 place-items-center rounded-full hover:bg-soft", loved ? "text-ink opacity-100" : "text-muted opacity-0 group-hover:opacity-100")}
        onClick={() => toggleLike(track.path)}
      >
        <Heart className="h-4 w-4" fill={loved ? "currentColor" : "none"} />
      </button>
      <button
        type="button"
        aria-label={t("player_more")}
        aria-expanded={open}
        className={cn("grid h-8 w-8 place-items-center rounded-full text-muted hover:bg-soft hover:text-ink", open ? "opacity-100" : "opacity-0 group-hover:opacity-100")}
        onClick={place}
      >
        <Ellipsis className="h-4 w-4" />
      </button>
      {open && box ? (
        <div className="fixed z-30 w-[220px] rounded-xl border border-line bg-surface p-1 shadow-sm" style={{ left: box.left, top: box.top }} role="menu">
          <MenuItem label={t("player_play_next")} onClick={() => { queueTracks([track], "next"); setOpen(false) }} />
          <MenuItem label={t("player_add_queue")} onClick={() => { queueTracks([track], "end"); setOpen(false) }} />
          <MenuItem label={t("player_radio_play")} onClick={() => { onRadio(); setOpen(false) }} />
          <MenuItem label={loved ? t("player_unlike") : t("player_like")} onClick={() => { toggleLike(track.path); setOpen(false) }} />
          <MenuItem label={t("player_add_to_playlist")} onClick={addSomewhere} />
          <MenuItem label={t("delete_track")} onClick={() => { requestDeleteTrack(track.path, track.title); setOpen(false) }} />
        </div>
      ) : null}
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
    <button type="button" className="text-[12px] text-muted hover:text-ink" onClick={onClick}>
      {label}
    </button>
  )
}
