import { useEffect, useMemo, useRef, useState, type ReactNode } from "react"
import { Segmented } from "../components/segmented"
import { IconNext, IconPause, IconPlay, IconPrev, IconRepeat, IconShuffle, IconVolume } from "../components/player-icons"
import { Scrubber } from "../components/scrubber"
import { Thumb } from "../components/thumb"
import { Equalizer } from "../components/equalizer"
import { LibraryPanel } from "../components/library-panel"
import { FindLyrics, Karaoke } from "../components/lyrics-tools"
import { invoke, isTauri } from "../lib/api"
import { runAi } from "../lib/ai-activity"
import { catalogTracks, refreshCatalog, useCatalog, useCatalogSettled } from "../lib/catalog"
import { formatClock } from "../lib/format"
import type { Key } from "../lib/i18n"
import { relatedTracks, type Track } from "../lib/library"
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
import { getState, openOutput, useApp, useT } from "../store"

const SOURCE_KEY: Record<string, Key> = {
  embedded: "lyrics_source_embedded",
  file: "lyrics_source_file",
  lrcmux: "lyrics_source_lrcmux",
  karalyr: "lyrics_source_karalyr",
  lrclib: "lyrics_source_lrclib",
  ovh: "lyrics_source_ovh",
  ai: "lyrics_ai_mark",
}

function accentFor(seed: string) {
  let hash = 0
  for (const ch of seed) hash = (hash * 33 + ch.charCodeAt(0)) >>> 0
  return `hsl(${hash % 360} 46% 48%)`
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
          ? "h-14 w-14 bg-[var(--player-accent,#161616)] text-white hover:opacity-90"
          : pressed
            ? "h-10 w-10 bg-soft text-ink"
            : "h-10 w-10 text-muted hover:bg-soft hover:text-ink",
      )}
    >
      {children}
    </button>
  )
}

function ToolButton({ label, pressed, onClick }: { label: string; pressed?: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      title={label}
      aria-pressed={pressed}
      onClick={onClick}
      className={cn(
        "h-8 rounded-full px-3 text-[12px] transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink",
        pressed ? "bg-fill text-onfill" : "bg-soft text-muted hover:text-ink",
      )}
    >
      {label}
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
      className={cn("flex w-full items-center gap-3 rounded-xl px-2 py-2 text-left focus-visible:outline focus-visible:outline-2 focus-visible:outline-ink", active ? "bg-soft" : "hover:bg-soft/80")}
    >
      <Thumb src={track.thumbnail} videoUrl={track.pageUrl} className="h-12 w-12 rounded-[10px]" />
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
      <span className="text-[12px] tabular-nums text-muted">{formatClock(track.duration)}</span>
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
  const [tools, setTools] = useState(false)
  const [dragFrom, setDragFrom] = useState<number | null>(null)
  const demoOnce = useRef(false)
  const [accent, setAccent] = useState("")
  const disk = useCatalog()
  const settled = useCatalogSettled()
  const library = useMemo(
    () => catalogTracks(app.jobs, app.history, app.convertJob, app.config.outputDir),
    [app.jobs, app.history, app.convertJob, app.config.outputDir, disk],
  )
  const pending = !settled && library.length === 0
  useEffect(() => {
    void refreshCatalog(app.config.outputDir, () => getState().config.outputDir)
  }, [app.config.outputDir, app.history.length, app.jobs.length])
  useEffect(() => {
    const demo = new URLSearchParams(window.location.search).get("demo")
    if (demo === "cinema" || demo === "player") {
      if (demoOnce.current) return
      demoOnce.current = true
      if (demo === "cinema") setCinema(true)
      const demoTrack: Track = {
        id: "preview-1",
        path: "/preview/clip.mp4",
        title: "Noite acesa",
        artist: "Banda Demo",
        album: "Ao vivo",
        folder: "demo",
        pageUrl: "",
        kind: "video",
        duration: 8,
      }
      playTracks([demoTrack], 0)
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
    const src = track?.thumbnail
    if (!src) {
      setAccent(accentFor(title || "bob"))
      return
    }
    const image = new Image()
    image.crossOrigin = "anonymous"
    image.onload = () => {
      try {
        const canvas = document.createElement("canvas")
        canvas.width = 4
        canvas.height = 4
        const ctx = canvas.getContext("2d")
        if (!ctx) return
        ctx.drawImage(image, 0, 0, 4, 4)
        const data = ctx.getImageData(0, 0, 4, 4).data
        let r = 0
        let g = 0
        let b = 0
        const count = data.length / 4
        for (let index = 0; index < data.length; index += 4) {
          r += data[index]
          g += data[index + 1]
          b += data[index + 2]
        }
        setAccent(`rgb(${Math.round(r / count)} ${Math.round(g / count)} ${Math.round(b / count)})`)
      } catch {
        setAccent(accentFor(title || src))
      }
    }
    image.onerror = () => setAccent(accentFor(title || "bob"))
    image.src = src
  }, [track?.thumbnail, title])

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

  const stage = (
    <div className={cn("relative overflow-hidden bg-black", cinema ? "h-full w-full" : "aspect-square rounded-[28px]")}>
      {showVideo ? (
        <div ref={setVideoSlot} className="absolute inset-0 bg-black" />
      ) : (
        <Thumb src={track?.thumbnail} videoUrl={track?.pageUrl || track?.title || track?.path} className="absolute inset-0 h-full w-full rounded-none" />
      )}
      {player.buffering ? (
        <p className="absolute bottom-3 left-3 rounded-full bg-black/60 px-3 py-1 text-[12px] text-white">{t("player_buffering")}</p>
      ) : null}
    </div>
  )

  const controls = (
    <div className="shrink-0 px-4 py-4 md:px-8">
      <div className={cn(cinema ? "mx-auto flex max-w-5xl items-end gap-6" : "")}>
        <div className="min-w-0 flex-1">
          {!cinema ? <p className="text-[12px] text-muted">{t("player_now")}</p> : null}
          <h1 className="truncate text-[26px] font-medium tracking-[-0.04em] md:text-[32px]">
            {title || (library.length ? t("player_idle") : pending ? t("searching") : t("player_empty"))}
          </h1>
          <p className="truncate text-[15px] text-muted">{[artist, album].filter(Boolean).join(" · ") || (track ? "\u00a0" : t("player_empty_hint"))}</p>
          {player.error ? (
            <p className="mt-2 text-[13px] text-danger">{t(player.error === "missing" ? "player_missing" : "player_failed")}</p>
          ) : null}
          <div className="mt-4">
            <Scrubber value={player.currentTime} max={player.duration} label={t("player_now")} disabled={disabled} onChange={seek} />
            <div className="mt-1 flex justify-between text-[12px] tabular-nums text-muted">
              <span>{formatClock(player.currentTime)}</span>
              <span>{formatClock(player.duration || track?.duration)}</span>
            </div>
          </div>
          <div className="mt-3 flex items-center justify-center gap-2">
            <TransportButton label={t("player_shuffle")} pressed={player.shuffle} disabled={!player.queue.length} onClick={toggleShuffle}>
              <IconShuffle className="h-[18px] w-[18px]" />
            </TransportButton>
            <TransportButton label={t("player_prev")} disabled={disabled} onClick={playPrevious}>
              <IconPrev className="h-5 w-5" />
            </TransportButton>
            <TransportButton label={player.playing ? t("pause") : t("player_play")} disabled={disabled} onClick={togglePlayback} large>
              {player.playing ? <IconPause className="h-5 w-5" /> : <IconPlay className="h-5 w-5" />}
            </TransportButton>
            <TransportButton label={t("player_next")} disabled={disabled} onClick={playNext}>
              <IconNext className="h-5 w-5" />
            </TransportButton>
            <TransportButton label={repeatLabel} pressed={player.repeat !== "off"} disabled={!player.queue.length} onClick={cycleRepeat}>
              <IconRepeat className="h-[18px] w-[18px]" one={player.repeat === "one"} />
            </TransportButton>
          </div>
          <div className="mx-auto mt-4 flex max-w-md items-center gap-3">
            <button type="button" aria-label={player.muted ? t("player_volume") : t("player_mute")} title={t("player_volume")} className="text-muted hover:text-ink" onClick={toggleMute}>
              <IconVolume className="h-5 w-5" muted={player.muted || player.volume === 0} />
            </button>
            <div className="min-w-0 flex-1">
              <Scrubber value={player.muted ? 0 : player.volume} max={1} label={t("player_volume")} onChange={setVolume} />
            </div>
          </div>
          <div className="mt-4 flex flex-wrap items-center justify-center gap-2">
            <ToolButton label={loved ? t("player_unlike") : t("player_like")} pressed={loved} onClick={() => track && toggleLike(track.path)} />
            <ToolButton label={t("player_lyrics")} pressed={player.panel === "lyrics" || cinema} onClick={() => (cinema ? setCinema(false) : setPanel("lyrics"))} />
            <ToolButton label={t("player_queue")} pressed={player.panel === "queue"} onClick={() => setPanel("queue")} />
            <ToolButton
              label={t("player_radio")}
              onClick={() => {
                if (!track) return
                const list = relatedTracks(track, library)
                if (list.length < 2) return
                playTracks(list, 0, true)
              }}
            />
            <ToolButton label={t("player_tools")} pressed={tools} onClick={() => setTools((value) => !value)} />
            <ToolButton label={cinema ? t("player_cinema_off") : t("player_cinema")} pressed={cinema} onClick={() => setCinema(!cinema)} />
            <ToolButton
              label={t("player_fullscreen")}
              onClick={() => {
                const node = rootRef.current
                if (!node) return
                if (document.fullscreenElement) void document.exitFullscreen()
                else void node.requestFullscreen()
              }}
            />
            <ToolButton label={t("player_mini")} onClick={() => void invoke("open_mini").catch(() => {})} />
          </div>
          {tools ? (
            <div className="mx-auto mt-3 max-w-lg rounded-2xl bg-soft p-3 text-[13px]">
              <label className="flex items-center gap-3">
                <span className="text-muted">{t("player_crossfade")}</span>
                <input type="range" min={0} max={12} step={1} value={music.crossfade} aria-label={t("player_crossfade")} onChange={(event) => setCrossfade(Number(event.target.value))} />
                <span className="tabular-nums text-muted">{music.crossfade === 0 ? t("player_off") : `${music.crossfade}s`}</span>
              </label>
              <label className="mt-2 flex items-center justify-between">
                <span>{t("player_normalize")}</span>
                <input type="checkbox" checked={music.normalize} onChange={(event) => setNormalize(event.target.checked)} />
              </label>
              <div className="mt-2 flex flex-wrap gap-1">
                <span className="mr-1 self-center text-muted">{t("player_sleep")}</span>
                {(["off", "15", "30", "45", "60", "end"] as SleepMode[]).map((mode) => (
                  <button
                    key={mode}
                    type="button"
                    aria-pressed={music.sleepMode === mode}
                    className={cn("h-7 rounded-full px-2 text-[12px]", music.sleepMode === mode ? "bg-fill text-onfill" : "bg-surface text-muted")}
                    onClick={() => setSleep(mode)}
                  >
                    {mode === "off" ? t("player_off") : mode === "end" ? t("player_sleep_end") : t("player_sleep_min", { n: mode })}
                  </button>
                ))}
              </div>
              <button type="button" className="mt-2 text-[12px] text-muted hover:text-ink" onClick={() => setEqOpen(true)}>
                {t("eq_title")}
              </button>
            </div>
          ) : null}
        </div>
      </div>
    </div>
  )

  const lyricsPane = <Lyrics time={player.currentTime} onFind={() => setFindOpen(true)} onKaraoke={() => setKaraoke(true)} />

  return (
    <div ref={rootRef} className="flex h-full min-h-0 flex-col bg-bg text-ink" style={{ ["--player-accent" as string]: accent || undefined }}>
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
        <div className="flex min-h-0 flex-1 flex-col lg:flex-row">
          <section className="min-h-0 flex-1 overflow-auto px-6 py-6 md:px-10">
            <div className="mx-auto w-full max-w-[460px]">{stage}</div>
          </section>
          <aside className="flex min-h-[280px] w-full min-w-0 flex-1 flex-col border-t border-line px-4 py-4 lg:min-h-0 lg:w-[400px] lg:flex-none lg:border-t-0 lg:border-l">
            <Segmented
              value={player.panel}
              onChange={setPanel}
              options={[
                { value: "library", label: t("player_library") },
                { value: "queue", label: t("player_queue") },
                { value: "lyrics", label: t("player_lyrics") },
              ]}
            />
            {player.panel === "lyrics" ? (
              <div className="mt-3 flex min-h-0 flex-1 flex-col">{lyricsPane}</div>
            ) : player.panel === "library" ? (
              <LibraryPanel
                tracks={library}
                pending={pending}
                query={query}
                onQuery={setQuery}
                renderTrack={(item, onPlay) => (
                  <TrackButton track={item} active={item.path === track?.path} playing={player.playing && item.path === track?.path} onClick={onPlay} />
                )}
              />
            ) : (
              <div className="mt-3 min-h-0 flex-1 space-y-1 overflow-auto pb-4">
                {queue.length === 0 ? <p className="px-2 py-10 text-[14px] text-muted">{t("player_queue_empty")}</p> : null}
                {queue.map((item, index) => (
                  <TrackButton
                    key={`${item.path}-${index}`}
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
                ))}
              </div>
            )}
          </aside>
        </div>
      )}
      {controls}
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
