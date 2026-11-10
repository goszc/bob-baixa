import { useEffect, useMemo, useRef, useState, type ReactNode } from "react"
import {
  Heart,
  Maximize2,
  Pause,
  Play,
  Repeat,
  Repeat1,
  Settings,
  Shuffle,
  SkipBack,
  SkipForward,
  Sparkles,
  Volume2,
  VolumeX,
} from "lucide-react"
import { Segmented } from "../components/segmented"
import { Scrubber } from "../components/scrubber"
import { Thumb } from "../components/thumb"
import { Equalizer } from "../components/equalizer"
import { LibraryPanel } from "../components/library-panel"
import { FindLyrics, Karaoke } from "../components/lyrics-tools"
import { invoke, isTauri } from "../lib/api"
import { runAi } from "../lib/ai-activity"
import { catalogTracks, refreshCatalog, useCatalog, useCatalogSettled } from "../lib/catalog"
import { discoverySplit } from "../lib/discover"
import { formatClock } from "../lib/format"
import type { Key } from "../lib/i18n"
import { type Track } from "../lib/library"
import { cn } from "../lib/cn"
import { liked, lyricNudgeFor, nudgeLyrics, setCinema, setCrossfade, setNormalize, setSleep, toggleLike, useMusic, type SleepMode } from "../lib/music"
import {
  activeLine,
  currentTrack,
  cycleRepeat,
  displayArtist,
  displayTitle,
  lyricClock,
  orderedTracks,
  playNext,
  playPrevious,
  playTracks,
  reloadLyrics,
  playOrderSlot,
  reorderQueue,
  restoreSession,
  syncQueue,
  seek,
  setPanel,
  setVideoSlot,
  setVolume,
  toggleMute,
  togglePlayback,
  toggleShuffle,
  usePlayer,
} from "../lib/player"
import { enqueueItems, getState, openOutput, useApp, useT } from "../store"
import { VirtualList } from "../components/virtual-list"

const SOURCE_KEY: Record<string, Key> = {
  embedded: "lyrics_source_embedded",
  file: "lyrics_source_file",
  lrcmux: "lyrics_source_lrcmux",
  karalyr: "lyrics_source_karalyr",
  lrclib: "lyrics_source_lrclib",
  ovh: "lyrics_source_ovh",
  ai: "lyrics_ai_mark",
}

const DEMO_ARTISTS = ["Low Roar", "Nils Frahm", "Agnes Obel", "Ólafur Arnalds", "Bon Iver", "Fleet Foxes", "José González", "Feist"]
const DEMO_TITLES = ["I'll Keep Coming", "Says", "Familiar", "Near Light", "Holocene", "Mykonos", "Heartbeats", "Mushaboom", "Open", "Softly"]
const DEMO_ALBUMS = ["Once", "Spaces", "Aventine", "re:member", "For Emma", "Fleet Foxes", "Veneer", "The Reminder"]

function makeDemoLibrary(): Track[] {
  return Array.from({ length: 200 }, (_, index) => ({
    id: index === 0 ? "preview-1" : `demo-${index + 1}`,
    path: index === 0 ? "/preview/clip.mp4" : `/library/demo-${index + 1}.mp3`,
    title: index < DEMO_TITLES.length ? DEMO_TITLES[index] : `${DEMO_TITLES[index % DEMO_TITLES.length]} ${index + 1}`,
    artist: DEMO_ARTISTS[index % DEMO_ARTISTS.length],
    album: DEMO_ALBUMS[index % DEMO_ALBUMS.length],
    folder: "Demo",
    pageUrl: "",
    kind: index === 0 ? "video" : "audio",
    duration: 150 + (index % 90),
  }))
}

function TransportButton({
  label,
  pressed,
  onClick,
  disabled,
  children,
  large,
}: {
  label: string
  pressed?: boolean
  onClick: () => void
  disabled?: boolean
  children: ReactNode
  large?: boolean
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      aria-pressed={pressed}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "inline-flex items-center justify-center rounded-full transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink disabled:opacity-30",
        large
          ? "h-11 w-11 bg-fill text-onfill hover:opacity-90"
          : pressed
            ? "h-8 w-8 bg-soft text-ink"
            : "h-8 w-8 text-muted hover:bg-soft hover:text-ink",
      )}
    >
      {children}
    </button>
  )
}

function IconButton({ label, pressed, onClick, children }: { label: string; pressed?: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      aria-pressed={pressed}
      onClick={onClick}
      className={cn(
        "grid h-8 w-8 place-items-center rounded-full text-muted transition-colors hover:bg-soft hover:text-ink focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink",
        pressed && "text-ink",
      )}
    >
      {children}
    </button>
  )
}

function TrackButton({
  track,
  active,
  playing,
  onClick,
  draggable,
  onDragStart,
  onDrop,
}: {
  track: Track
  active: boolean
  playing: boolean
  onClick: () => void
  draggable?: boolean
  onDragStart?: () => void
  onDrop?: () => void
}) {
  return (
    <button
      type="button"
      draggable={draggable}
      onDragStart={onDragStart}
      onDragOver={(event) => {
        if (draggable) event.preventDefault()
      }}
      onDrop={(event) => {
        event.preventDefault()
        onDrop?.()
      }}
      onClick={onClick}
      className={cn("flex h-full w-full items-center gap-3 rounded-xl px-3 text-left focus-visible:outline focus-visible:outline-2 focus-visible:outline-ink", active ? "bg-soft" : "hover:bg-soft/70")}
    >
      <Thumb src={track.thumbnail} videoUrl={track.pageUrl} className="h-10 w-10 rounded-lg" />
      <span className="min-w-0 flex-1">
        <span className="flex items-center gap-2">
          {active && playing ? (
            <span className="eq text-ink" aria-hidden="true">
              <span />
              <span />
              <span />
            </span>
          ) : null}
          <span className="truncate text-[14px] font-medium">{track.title}</span>
        </span>
        <span className="mt-0.5 block truncate text-[12px] text-muted">{track.artist}</span>
      </span>
    </button>
  )
}

function Lyrics({ time, onFind, onKaraoke }: { time: number; onFind: () => void; onKaraoke: () => void }) {
  const player = usePlayer()
  const app = useApp()
  const music = useMusic()
  const t = useT()
  const listRef = useRef<HTMLDivElement>(null)
  const [translated, setTranslated] = useState<string[] | null>(null)
  const [note, setNote] = useState("")
  const lyrics = player.lyrics
  const track = currentTrack(player)
  const nudge = track ? lyricNudgeFor(track.path) : 0
  const synced = Boolean(lyrics?.synced)
  const clock = lyricClock(time, nudge)
  const current = synced && lyrics ? activeLine(lyrics.lines, clock) : -1
  const lines = lyrics?.lines ?? []

  useEffect(() => {
    const list = listRef.current
    const node = list?.querySelector<HTMLElement>("[data-lyric-active='true']")
    if (!list || !node) return
    const top = node.getBoundingClientRect().top - list.getBoundingClientRect().top + list.scrollTop
    list.scrollTo({ top: top - list.clientHeight / 2 + node.clientHeight / 2, behavior: "smooth" })
  }, [current])

  const source = lyrics ? SOURCE_KEY[lyrics.source] : undefined
  const runTranslate = () => {
    if (!lines.length) {
      setNote(t("lyrics_translate_fail"))
      return
    }
    setNote(t("ai_translate"))
    void runAi(t("ai_translate"), "gemma-4-31b-it", () =>
      invoke<string[]>("translate_lines", { lines: lines.map((line) => line.text), target: app.config.translateLang || "pt" }),
    )
      .then((next) => {
        setTranslated(next)
        setNote("")
      })
      .catch((error: Error) => setNote(error.message || t("lyrics_translate_fail")))
  }
  return (
    <div className="flex h-full min-h-0 flex-1 flex-col">
      <div className="mb-2 flex flex-wrap items-center gap-2 px-1">
        <button type="button" className="rounded-full bg-soft px-3 py-1 text-[12px] text-muted hover:text-ink" onClick={onFind}>
          {t("lyrics_find")}
        </button>
        <button type="button" className="rounded-full bg-soft px-3 py-1 text-[12px] text-muted hover:text-ink" onClick={runTranslate}>
          {t("lyrics_translate")}
        </button>
        <button
          type="button"
          title={t("player_nudge_forward")}
          className="rounded-full bg-soft px-2 py-1 text-[12px] tabular-nums text-muted hover:text-ink"
          onClick={() => track && nudgeLyrics(track.path, -0.5)}
        >
          −0,5s
        </button>
        <span className="text-[12px] tabular-nums text-muted">{nudge > 0 ? `+${nudge.toFixed(1)}s` : `${nudge.toFixed(1)}s`}</span>
        <button
          type="button"
          title={t("player_nudge_back")}
          className="rounded-full bg-soft px-2 py-1 text-[12px] tabular-nums text-muted hover:text-ink"
          onClick={() => track && nudgeLyrics(track.path, 0.5)}
        >
          +0,5s
        </button>
        <button type="button" className="rounded-full bg-soft px-3 py-1 text-[12px] text-muted hover:text-ink" onClick={onKaraoke} disabled={!lines.length}>
          {t("lyrics_karaoke")}
        </button>
      </div>
      {player.lyricsState === "loading" ? <p className="px-3 py-6 text-[14px] text-muted">{t("lyrics_loading")}</p> : null}
      {player.buffering ? <p className="px-3 text-[12px] text-muted">{t("player_buffering")}</p> : null}
      {!player.error && player.lyricsState !== "loading" && (!lyrics || lyrics.instrumental || !lines.length) ? (
        <div className="px-3 py-6">
          <p className="text-[14px] text-muted">{t(lyrics?.instrumental ? "lyrics_instrumental" : "player_lyrics_missing")}</p>
          {track && isTauri() ? (
            <button
              type="button"
              className="mt-3 text-[13px] text-muted hover:text-ink"
              onClick={() => {
                setNote(t("ai_transcribe"))
                void runAi(t("ai_transcribe"), app.config.geminiTranscribe || "gemini-3.5-transcribe", () => invoke("ai_transcribe", { path: track.path }))
                  .then(() => {
                    reloadLyrics()
                    setNote("")
                  })
                  .catch((error: Error) => setNote(error.message))
              }}
            >
              {t("lyrics_ai")}
            </button>
          ) : null}
        </div>
      ) : null}
      {source ? <p className="px-3 pb-1 text-[12px] text-muted">{t(source)}</p> : null}
      {note ? <p className="px-3 pb-2 text-[12px] text-muted">{note}</p> : null}
      <div ref={listRef} className="relative min-h-0 flex-1 space-y-1 overflow-auto pb-6">
        {lines.map((line, index) => {
          const on = synced && index === current
          return (
            <button
              key={`${line.at}-${index}`}
              type="button"
              data-lyric-active={on ? "true" : "false"}
              onClick={() => {
                if (synced) seek(line.at + nudge)
              }}
              className={cn(
                "block w-full rounded-xl px-3 py-2.5 text-left text-[16px] leading-snug transition-colors",
                on ? "bg-soft font-medium text-ink" : "text-muted hover:text-ink",
              )}
            >
              <span>{line.text}</span>
              {translated?.[index] ? <span className="mt-1 block text-[13px] text-muted">{translated[index]}</span> : null}
            </button>
          )
        })}
      </div>
      <span className="sr-only">{music.cinema ? "cinema" : "normal"}</span>
    </div>
  )
}

export function Player() {
  const app = useApp()
  const player = usePlayer()
  const music = useMusic()
  const t = useT()
  const rootRef = useRef<HTMLDivElement>(null)
  const [query, setQuery] = useState("")
  const [eqOpen, setEqOpen] = useState(false)
  const [findOpen, setFindOpen] = useState(false)
  const [karaoke, setKaraoke] = useState(false)
  const [gear, setGear] = useState(false)
  const [askOpen, setAskOpen] = useState(false)
  const [viewOpen, setViewOpen] = useState(false)
  const [ask, setAsk] = useState("")
  const [note, setNote] = useState("")
  const [ratio, setRatio] = useState(16 / 9)
  const [dragFrom, setDragFrom] = useState<number | null>(null)
  const demoOnce = useRef(false)
  const [demoLibrary, setDemoLibrary] = useState<Track[] | null>(null)
  const disk = useCatalog()
  const settled = useCatalogSettled()
  const catalog = useMemo(
    () => catalogTracks(app.jobs, app.history, app.convertJob, app.config.outputDir),
    [app.jobs, app.history, app.convertJob, app.config.outputDir, disk],
  )
  const library = demoLibrary ?? catalog
  const pending = !demoLibrary && !settled && library.length === 0
  useEffect(() => {
    if (demoLibrary) return
    void refreshCatalog(app.config.outputDir, () => getState().config.outputDir)
  }, [app.config.outputDir, app.history.length, app.jobs.length, demoLibrary])
  useEffect(() => {
    const demo = new URLSearchParams(window.location.search).get("demo")
    if (demo === "cinema" || demo === "player") {
      if (demoOnce.current) return
      demoOnce.current = true
      if (demo === "cinema") setCinema(true)
      const tracks = makeDemoLibrary()
      setDemoLibrary(tracks)
      playTracks(tracks, 0)
      return
    }
    restoreSession(library, disk !== null || !isTauri())
    if (disk) syncQueue(library)
  }, [library, disk])
  const queue = orderedTracks(player)
  const track = currentTrack(player)
  const title = displayTitle(player)
  const artist = displayArtist(player)
  const album = player.meta?.album || track?.album || ""
  const showVideo = player.playbackKind === "video" && !player.error && Boolean(track)
  const disabled = !track || player.error !== null
  const loved = track ? liked(track.path) : false
  const repeatLabel =
    player.repeat === "all" ? t("player_repeat_all") : player.repeat === "one" ? t("player_repeat_one") : t("player_repeat_off")
  const cinema = music.cinema

  useEffect(() => {
    return () => setVideoSlot(null)
  }, [])

  useEffect(() => {
    if (!showVideo) return
    let stop = false
    const read = () => {
      const video = rootRef.current?.querySelector("video")
      if (!(video instanceof HTMLVideoElement) || video.videoWidth < 1) return false
      setRatio(video.videoWidth / video.videoHeight)
      return true
    }
    if (read()) return
    const timer = window.setInterval(() => {
      if (stop || read()) window.clearInterval(timer)
    }, 200)
    return () => {
      stop = true
      window.clearInterval(timer)
    }
  }, [showVideo, track?.path])

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target
      if (target instanceof HTMLElement && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)) return
      if (event.code === "Space") {
        event.preventDefault()
        togglePlayback()
      } else if (event.key === "ArrowRight" && event.ctrlKey) {
        playNext()
      } else if (event.key === "ArrowLeft" && event.ctrlKey) {
        playPrevious()
      } else if (event.key === "ArrowRight") {
        seek(player.currentTime + 5)
      } else if (event.key === "ArrowLeft") {
        seek(Math.max(0, player.currentTime - 5))
      } else if (event.key.toLowerCase() === "l") {
        setPanel("lyrics")
        if (!cinema) setCinema(false)
      } else if (event.key.toLowerCase() === "q") {
        setPanel("queue")
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [player.currentTime, cinema])

  const wide = showVideo && ratio > 1.15
  const frame = showVideo
    ? { width: `min(100%, ${wide ? 720 : 360}px, calc(${wide ? 46 : 40}vh * ${ratio}))`, aspectRatio: String(ratio) }
    : { width: "min(100%, 280px, 34vh)", aspectRatio: "1" }

  const picture = (
    <>
      {showVideo ? (
        <div ref={setVideoSlot} className="stage-video absolute inset-0 bg-black" />
      ) : (
        <Thumb src={track?.thumbnail} videoUrl={track?.pageUrl || track?.title || track?.path} className="absolute inset-0 h-full w-full rounded-none" />
      )}
      {player.buffering ? (
        <p className="absolute bottom-3 left-3 rounded-full bg-black/55 px-3 py-1 text-[12px] text-white">{t("player_buffering")}</p>
      ) : null}
    </>
  )

  const stage = cinema ? (
    <div className="relative h-full w-full overflow-hidden bg-black">{picture}</div>
  ) : (
    <div className="relative mx-auto overflow-hidden rounded-2xl bg-black" style={frame}>
      {picture}
    </div>
  )

  async function discover() {
    const plan = discoverySplit(Math.min(library.length, 12), app.config.discoverRatio)
    const known = library.slice(0, plan.library)
    if (known.length) playTracks(known, 0)
    if (!isTauri()) return
    const artists = [...new Set(library.map((item) => item.artist).filter(Boolean))].slice(0, 8)
    const queries = await invoke<string[]>("discover_queries", { artists })
    await enqueueItems(
      queries.slice(0, plan.fresh || 2).map((queryText) => ({
        url: `ytsearch1:${queryText}`,
        title: queryText,
        audioOnly: true,
        audioFormat: "opus",
        audioBitrate: "24k",
      })),
    )
  }

  async function mood(name: string) {
    if (!isTauri()) return
    const rows = library.map((item) => ({ path: item.path, artist: item.artist, title: item.title, album: item.album, plays: 1 }))
    const paths = await runAi(t("ai_mood"), "gemini-3.5-flash-lite", () => invoke<string[]>("mood_mix", { mood: name, library: rows }), (found) => `${found.length}`)
    const list = paths.map((path) => library.find((item) => item.path === path)).filter((item): item is Track => Boolean(item))
    if (list.length) playTracks(list, 0)
  }

  async function askQueue() {
    const text = ask.trim()
    if (!text) return
    if (!isTauri()) {
      setNote(t("player_ask"))
      return
    }
    const rows = library.map((item) => ({ path: item.path, artist: item.artist, title: item.title, album: item.album, plays: 1 }))
    const paths = await runAi(t("ai_thinking"), "gemini-3.5-flash-lite", () => invoke<string[]>("natural_queue", { text, library: rows }), (found) => `${found.length}`)
    const list = paths.map((path) => library.find((item) => item.path === path)).filter((item): item is Track => Boolean(item))
    if (list.length) playTracks(list, 0)
    setNote(list.length ? `Fila com ${list.length} ${list.length === 1 ? "música" : "músicas"}` : t("lyrics_empty"))
  }

  const transport = (
    <div className="flex items-center justify-center gap-0.5">
      <TransportButton label={t("player_shuffle")} pressed={player.shuffle} disabled={!player.queue.length} onClick={toggleShuffle}>
        <Shuffle className="h-[18px] w-[18px]" />
      </TransportButton>
      <TransportButton label={t("player_prev")} disabled={disabled} onClick={playPrevious}>
        <SkipBack className="h-5 w-5" />
      </TransportButton>
      <TransportButton label={player.playing ? t("pause") : t("player_play")} disabled={disabled} onClick={togglePlayback} large>
        {player.playing ? <Pause className="h-5 w-5" /> : <Play className="h-5 w-5" />}
      </TransportButton>
      <TransportButton label={t("player_next")} disabled={disabled} onClick={playNext}>
        <SkipForward className="h-5 w-5" />
      </TransportButton>
      <TransportButton label={repeatLabel} pressed={player.repeat !== "off"} disabled={!player.queue.length} onClick={cycleRepeat}>
        {player.repeat === "one" ? <Repeat1 className="h-[18px] w-[18px]" /> : <Repeat className="h-[18px] w-[18px]" />}
      </TransportButton>
    </div>
  )

  const tools = (
    <div className="flex items-center gap-0.5">
      <IconButton
        label={t("player_ask")}
        pressed={askOpen}
        onClick={() => {
          setAskOpen((value) => !value)
          setGear(false)
          setViewOpen(false)
        }}
      >
        <Sparkles className="h-4 w-4" />
      </IconButton>
      <div className="relative">
        <IconButton label={t("player_fullscreen")} pressed={viewOpen} onClick={() => setViewOpen((value) => !value)}>
          <Maximize2 className="h-4 w-4" />
        </IconButton>
        {viewOpen ? (
          <div className="absolute bottom-10 right-0 z-20 flex flex-col items-start rounded-2xl border border-line bg-surface p-2 text-[13px]">
            <button type="button" className="px-2 py-1 text-muted hover:text-ink" onClick={() => setCinema(!cinema)}>
              {cinema ? t("player_cinema_off") : t("player_cinema")}
            </button>
            <button
              type="button"
              className="px-2 py-1 text-muted hover:text-ink"
              onClick={() => {
                const node = rootRef.current
                if (!node) return
                if (document.fullscreenElement) void document.exitFullscreen()
                else void node.requestFullscreen()
              }}
            >
              {t("player_fullscreen")}
            </button>
            <button type="button" className="px-2 py-1 text-muted hover:text-ink" onClick={() => void invoke("open_mini").catch(() => {})}>
              {t("player_mini")}
            </button>
          </div>
        ) : null}
      </div>
      <div className="relative">
        <IconButton
          label={t("player_tools")}
          pressed={gear}
          onClick={() => {
            setGear((value) => !value)
            setAskOpen(false)
          }}
        >
          <Settings className="h-[18px] w-[18px]" />
        </IconButton>
        {gear ? (
          <div className="absolute bottom-11 left-1/2 z-20 w-64 -translate-x-1/2 rounded-2xl border border-line bg-surface p-4 text-[14px] shadow-sm">
            <label className="flex items-center gap-3">
              <span className="w-24 shrink-0 text-muted">{t("player_crossfade")}</span>
              <input className="min-w-0 flex-1" type="range" min={0} max={12} step={1} value={music.crossfade} aria-label={t("player_crossfade")} onChange={(event) => setCrossfade(Number(event.target.value))} />
              <span className="w-8 tabular-nums text-muted">{music.crossfade === 0 ? t("player_off") : `${music.crossfade}s`}</span>
            </label>
            <label className="mt-3 flex items-center justify-between gap-3">
              <span>{t("player_normalize")}</span>
              <input type="checkbox" checked={music.normalize} onChange={(event) => setNormalize(event.target.checked)} />
            </label>
            <label className="mt-3 flex items-center justify-between gap-3">
              <span className="text-muted">{t("player_sleep")}</span>
              <select
                aria-label={t("player_sleep")}
                value={music.sleepMode}
                onChange={(event) => setSleep(event.target.value as SleepMode)}
                className="h-8 rounded-lg bg-soft px-2 text-[13px]"
              >
                <option value="off">{t("player_off")}</option>
                <option value="15">{t("player_sleep_min", { n: "15" })}</option>
                <option value="30">{t("player_sleep_min", { n: "30" })}</option>
                <option value="45">{t("player_sleep_min", { n: "45" })}</option>
                <option value="60">{t("player_sleep_min", { n: "60" })}</option>
                <option value="end">{t("player_sleep_end")}</option>
              </select>
            </label>
            <div className="mt-3 flex flex-col items-start gap-1">
              <button type="button" className="text-muted hover:text-ink" onClick={() => void discover().catch(() => {})}>
                {t("player_discover")}
              </button>
              <button type="button" className="text-muted hover:text-ink" onClick={() => void mood(t("player_mood_study")).catch(() => {})}>
                {t("player_mood_study")}
              </button>
              <button type="button" className="text-muted hover:text-ink" onClick={() => void mood(t("player_mood_train")).catch(() => {})}>
                {t("player_mood_train")}
              </button>
              <button type="button" className="text-muted hover:text-ink" onClick={() => setEqOpen(true)}>
                {t("eq_title")}
              </button>
            </div>
          </div>
        ) : null}
      </div>
    </div>
  )

  const meta = (
    <div className="mx-auto w-full max-w-[560px]">
      <h1 className="flex items-center justify-center gap-2 truncate text-center text-[20px] font-medium tracking-[-0.03em]">
        <span className="truncate">{title || (library.length ? t("player_idle") : pending ? t("searching") : t("player_empty"))}</span>
        {track ? (
          <button type="button" aria-label={loved ? t("player_unlike") : t("player_like")} className="text-muted hover:text-ink" onClick={() => toggleLike(track.path)}>
            <Heart className="h-4 w-4" fill={loved ? "currentColor" : "none"} />
          </button>
        ) : null}
      </h1>
      <p className="mt-1 truncate text-center text-[14px] text-muted">{[artist, album].filter(Boolean).join(" · ") || (track ? "\u00a0" : t("player_empty_hint"))}</p>
      {player.error ? <p className="mt-2 text-center text-[14px] text-danger">{t(player.error === "missing" ? "player_missing" : "player_failed")}</p> : null}
      {askOpen ? (
        <form
          className="mx-auto mt-3 flex max-w-sm gap-2"
          onSubmit={(event) => {
            event.preventDefault()
            void askQueue().catch((error: Error) => setNote(error.message))
          }}
        >
          <input
            value={ask}
            onChange={(event) => setAsk(event.target.value)}
            placeholder={t("player_ask")}
            aria-label={t("player_ask")}
            className="h-9 min-w-0 flex-1 rounded-full border border-line bg-surface px-3 text-[14px] outline-none"
          />
        </form>
      ) : null}
      {note ? <p className="mt-2 text-center text-[12px] text-muted">{note}</p> : null}
      <div className="mt-4">{transport}</div>
      <div className="mt-3">
        <Scrubber value={player.currentTime} max={player.duration} label={t("player_now")} disabled={disabled} onChange={seek} />
        <div className="mt-1 flex justify-between text-[14px] tabular-nums text-muted">
          <span>{formatClock(player.currentTime)}</span>
          <span>{formatClock(player.duration || track?.duration)}</span>
        </div>
      </div>
      <div className="mx-auto mt-2 flex max-w-xs items-center gap-3">
        <button type="button" aria-label={player.muted ? t("player_volume") : t("player_mute")} title={t("player_volume")} className="text-muted hover:text-ink" onClick={toggleMute}>
          {player.muted || player.volume === 0 ? <VolumeX className="h-4 w-4" /> : <Volume2 className="h-4 w-4" />}
        </button>
        <div className="min-w-0 flex-1">
          <Scrubber value={player.muted ? 0 : player.volume} max={1} label={t("player_volume")} onChange={setVolume} />
        </div>
        {tools}
      </div>
    </div>
  )

  const lyricsPane = <Lyrics time={player.currentTime} onFind={() => setFindOpen(true)} onKaraoke={() => setKaraoke(true)} />

  return (
    <div ref={rootRef} className="flex h-full min-h-0 flex-col bg-bg text-ink">
      {cinema ? (
        <div className="flex min-h-0 flex-1">
          <div className="min-h-0 min-w-0" style={{ flex: "7 1 0%" }}>
            {stage}
          </div>
          <div className="flex min-h-0 min-w-0 flex-col border-l border-line" style={{ flex: "2 1 0%" }}>
            {lyricsPane}
          </div>
        </div>
      ) : (
        <div className="flex min-h-0 min-w-0 flex-1">
          <section className="flex min-h-0 min-w-0 flex-1 flex-col items-center justify-center overflow-hidden px-6 py-6">
            {stage}
            <div className="mt-6 w-full shrink-0">{meta}</div>
          </section>
          <aside className="flex h-full w-[300px] min-h-0 shrink-0 flex-col border-l border-line">
            <div className="shrink-0 px-3 pt-3">
              <Segmented
                value={player.panel}
                onChange={setPanel}
                options={[
                  { value: "library", label: t("player_library") },
                  { value: "queue", label: t("player_queue") },
                  { value: "lyrics", label: t("player_lyrics") },
                ]}
              />
            </div>
            {player.panel === "lyrics" ? (
              <div className="mt-2 flex min-h-0 flex-1 flex-col">{lyricsPane}</div>
            ) : player.panel === "library" ? (
              <div className="min-h-0 flex-1">
                <LibraryPanel
                  tracks={library}
                  pending={pending}
                  query={query}
                  onQuery={setQuery}
                  renderTrack={(item, onPlay) => (
                    <TrackButton track={item} active={item.path === track?.path} playing={player.playing && item.path === track?.path} onClick={onPlay} />
                  )}
                />
              </div>
            ) : (
              <div className="min-h-0 flex-1">
                {queue.length === 0 ? (
                  <p className="px-4 py-8 text-[14px] text-muted">{t("player_queue_empty")}</p>
                ) : (
                  <VirtualList
                    count={queue.length}
                    rowHeight={56}
                    className="h-full overflow-auto"
                    render={(index) => {
                      const item = queue[index]
                      return (
                        <TrackButton
                          track={item}
                          active={player.order[index] === player.index}
                          playing={player.playing && item.path === track?.path}
                          draggable
                          onDragStart={() => setDragFrom(index)}
                          onDrop={() => {
                            if (dragFrom != null) reorderQueue(dragFrom, index)
                            setDragFrom(null)
                          }}
                          onClick={() => playOrderSlot(index)}
                        />
                      )
                    }}
                  />
                )}
              </div>
            )}
          </aside>
        </div>
      )}
      {cinema ? <div className="shrink-0 border-t border-line px-6 py-3">{meta}</div> : null}
      {track && player.error ? (
        <button type="button" className="sr-only" onClick={() => void openOutput(track.path)}>
          {t("open_folder")}
        </button>
      ) : null}
      {eqOpen ? <Equalizer path={track?.path} onClose={() => setEqOpen(false)} /> : null}
      {findOpen && track ? (
        <FindLyrics
          path={track.path}
          title={title}
          artist={artist}
          duration={player.duration || track.duration}
          onClose={() => setFindOpen(false)}
          onPinned={() => {
            reloadLyrics()
            setFindOpen(false)
          }}
        />
      ) : null}
      {karaoke && player.lyrics ? <Karaoke lyrics={player.lyrics} time={lyricClock(player.currentTime, track ? lyricNudgeFor(track.path) : 0)} onClose={() => setKaraoke(false)} /> : null}
    </div>
  )
}
