import { useEffect, useMemo, useRef, useState, type ReactNode } from "react"
import { Segmented } from "../components/segmented"
import { IconNext, IconPause, IconPlay, IconPrev, IconRepeat, IconShuffle, IconVolume } from "../components/player-icons"
import { Scrubber } from "../components/scrubber"
import { Thumb } from "../components/thumb"
import { Equalizer } from "../components/equalizer"
import { LibraryPanel } from "../components/library-panel"
import { FindLyrics, Karaoke } from "../components/lyrics-tools"
import { invoke, isTauri } from "../lib/api"
import { catalogTracks, refreshCatalog, useCatalog, useCatalogSettled } from "../lib/catalog"
import { formatClock } from "../lib/format"
import type { Key } from "../lib/i18n"
import type { Track } from "../lib/library"
import { cn } from "../lib/cn"
import {
  activeLine,
  currentTrack,
  cycleRepeat,
  displayArtist,
  displayTitle,
  orderedTracks,
  playNext,
  playPrevious,
  reloadLyrics,
  playOrderSlot,
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
      aria-pressed={pressed}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "inline-flex items-center justify-center rounded-full transition-colors disabled:opacity-30",
        large
          ? "h-14 w-14 bg-fill text-onfill hover:opacity-90"
          : pressed
            ? "h-10 w-10 bg-soft text-ink"
            : "h-10 w-10 text-muted hover:bg-soft hover:text-ink",
      )}
    >
      {children}
    </button>
  )
}

function TrackButton({ track, active, playing, onClick }: { track: Track; active: boolean; playing: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn("flex w-full items-center gap-3 rounded-xl px-2 py-2 text-left", active ? "bg-soft" : "hover:bg-soft/80")}
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
  const t = useT()
  const listRef = useRef<HTMLDivElement>(null)
  const [translated, setTranslated] = useState<string[] | null>(null)
  const [translateNote, setTranslateNote] = useState("")
  const lyrics = player.lyrics
  const playing = currentTrack()
  const synced = Boolean(lyrics?.synced)
  const current = synced && lyrics ? activeLine(lyrics.lines, time) : -1

  useEffect(() => {
    const list = listRef.current
    const node = list?.querySelector<HTMLElement>("[data-lyric-active='true']")
    if (!list || !node) return
    const top = node.getBoundingClientRect().top - list.getBoundingClientRect().top + list.scrollTop
    list.scrollTo({ top: top - list.clientHeight / 2 + node.clientHeight / 2, behavior: "smooth" })
  }, [current])

  const source = lyrics ? SOURCE_KEY[lyrics.source] : undefined
  const lines = lyrics?.lines ?? []
  const translate = () => {
    if (!lines.length || !isTauri()) {
      setTranslateNote(t("lyrics_translate_fail"))
      return
    }
    setTranslateNote("")
    void invoke<string[]>("translate_lines", { lines: lines.map((line) => line.text), target: app.config.translateLang || "pt" })
      .then((next) => setTranslated(next))
      .catch(() => setTranslateNote(t("lyrics_translate_fail")))
  }
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="mb-2 flex flex-wrap gap-2 px-1">
        <button type="button" className="rounded-full bg-soft px-3 py-1 text-[12px] text-muted hover:text-ink" onClick={onFind}>
          {t("lyrics_find")}
        </button>
        <button type="button" className="rounded-full bg-soft px-3 py-1 text-[12px] text-muted hover:text-ink" onClick={translate}>
          {t("lyrics_translate")}
        </button>
        <button type="button" className="rounded-full bg-soft px-3 py-1 text-[12px] text-muted hover:text-ink" onClick={onKaraoke} disabled={!lines.length}>
          {t("lyrics_karaoke")}
        </button>
      </div>
      {player.lyricsState === "loading" ? <p className="px-3 py-10 text-[14px] text-muted">{t("lyrics_loading")}</p> : null}
      {player.error ? <p className="px-3 py-10 text-[14px] text-danger">{t(player.error === "missing" ? "player_missing" : "player_failed")}</p> : null}
      {!player.error && player.lyricsState !== "loading" && (!lyrics || lyrics.instrumental || !lines.length) ? (
        <div className="px-3 py-6">
          <p className="text-[14px] text-muted">{t(lyrics?.instrumental ? "lyrics_instrumental" : lyrics?.source === "ai" ? "lyrics_ai_mark" : "lyrics_empty")}</p>
          {playing ? (
            <button
              type="button"
              className="mt-3 text-[13px] text-muted hover:text-ink"
              onClick={() => {
                void invoke("ai_transcribe", { path: playing.path }).then(() => reloadLyrics())
              }}
            >
              {t("lyrics_ai")}
            </button>
          ) : null}
          {lyrics?.tried?.length ? (
            <ul className="mt-3 space-y-1 text-[12px] text-muted">
              {lyrics.tried.map((query) => (
                <li key={query}>{t("lyrics_tried").replace("{query}", query)}</li>
              ))}
            </ul>
          ) : null}
        </div>
      ) : null}
      {source ? <p className="px-3 pb-2 text-[12px] text-muted">{t(source)}</p> : null}
      {translateNote ? <p className="px-3 pb-2 text-[12px] text-muted">{translateNote}</p> : null}
      <div ref={listRef} className="relative min-h-0 flex-1 space-y-1 overflow-auto pb-6">
        {lines.map((line, index) => {
          const on = synced && index === current
          return (
            <button
              key={`${line.at}-${index}`}
              type="button"
              data-lyric-active={on ? "true" : "false"}
              onClick={() => {
                if (synced) seek(line.at)
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
    </div>
  )
}

export function Player() {
  const app = useApp()
  const player = usePlayer()
  const t = useT()
  const [query, setQuery] = useState("")
  const [eqOpen, setEqOpen] = useState(false)
  const [findOpen, setFindOpen] = useState(false)
  const [karaoke, setKaraoke] = useState(false)
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
    restoreSession(library, disk !== null || !isTauri())
    if (disk) syncQueue(library)
  }, [library, disk])
  const queue = orderedTracks(player)
  const track = currentTrack(player)
  const title = displayTitle(player)
  const artist = displayArtist(player)
  const showVideo = player.playbackKind === "video" && !player.error && Boolean(track)
  const disabled = !track || player.error !== null
  const repeatLabel =
    player.repeat === "all" ? t("player_repeat_all") : player.repeat === "one" ? t("player_repeat_one") : t("player_repeat_off")

  useEffect(() => {
    return () => setVideoSlot(null)
  }, [])

  return (
    <div className="flex h-full min-h-0 flex-col lg:flex-row">
      <section className="min-h-0 flex-1 overflow-auto px-6 py-8 md:px-10">
        <div className="mx-auto flex w-full max-w-[460px] flex-col">
          <div className="relative aspect-square overflow-hidden rounded-[28px] bg-soft">
            {showVideo ? (
              <div ref={setVideoSlot} className="absolute inset-0 bg-black" />
            ) : (
              <Thumb src={track?.thumbnail} videoUrl={track?.pageUrl || track?.title || track?.path} className="absolute inset-0 h-full w-full rounded-none" />
            )}
          </div>
          <p className="mt-6 text-[12px] text-muted">{t("player_now")}</p>
          <h1 className="mt-1 truncate text-[28px] font-medium tracking-[-0.04em]">
            {title || (library.length ? t("player_idle") : pending ? t("searching") : t("player_empty"))}
          </h1>
          <p className="mt-1 truncate text-[15px] text-muted">
            {artist || (track ? "\u00a0" : library.length ? t("player_queue_empty") : pending ? "\u00a0" : t("player_empty_hint"))}
          </p>
          {player.error ? (
            <div className="mt-4">
              <p className="text-[13px] leading-relaxed text-danger">
                {t(player.error === "missing" ? "player_missing" : "player_failed")}
              </p>
              {track ? (
                <button type="button" className="mt-2 text-[13px] text-muted hover:text-ink" onClick={() => void openOutput(track.path)}>
                  {t("open_folder")}
                </button>
              ) : null}
            </div>
          ) : null}
          <div className="mt-6">
            <Scrubber value={player.currentTime} max={player.duration} label={t("player_now")} disabled={disabled} onChange={seek} />
            <div className="mt-1 flex justify-between text-[12px] tabular-nums text-muted">
              <span>{formatClock(player.currentTime)}</span>
              <span>{formatClock(player.duration || track?.duration)}</span>
            </div>
          </div>
          <div className="mt-4 flex items-center justify-center gap-2">
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
          <div className="mx-auto mt-5 flex w-full max-w-[240px] items-center gap-3">
            <button type="button" aria-label={player.muted ? t("player_volume") : t("player_mute")} className="text-muted hover:text-ink" onClick={toggleMute}>
              <IconVolume className="h-5 w-5" muted={player.muted || player.volume === 0} />
            </button>
            <div className="min-w-0 flex-1">
              <Scrubber
                value={player.muted ? 0 : player.volume}
                max={1}
                label={t("player_volume")}
                onChange={setVolume}
              />
            </div>
            <button type="button" className="text-[12px] text-muted hover:text-ink" onClick={() => setEqOpen(true)}>
              {t("eq_title")}
            </button>
            <button type="button" className="text-[12px] text-muted hover:text-ink" onClick={() => void invoke("open_mini")}>
              {t("player_mini")}
            </button>
          </div>
        </div>
      </section>
      <aside className="flex min-h-[360px] w-full min-w-0 flex-1 flex-col border-t border-line px-4 py-5 lg:min-h-0 lg:w-[400px] lg:flex-none lg:border-t-0 lg:border-l">
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
          <div className="mt-4 flex min-h-0 flex-1 flex-col">
            <Lyrics time={player.currentTime} onFind={() => setFindOpen(true)} onKaraoke={() => setKaraoke(true)} />
          </div>
        ) : (
          <div className="mt-4 flex min-h-0 flex-1 flex-col">
            {player.panel === "library" ? (
              <LibraryPanel
                tracks={library}
                pending={pending}
                query={query}
                onQuery={setQuery}
                renderTrack={(item, onPlay) => (
                  <TrackButton
                    track={item}
                    active={item.path === track?.path}
                    playing={player.playing && !player.error && item.path === track?.path}
                    onClick={onPlay}
                  />
                )}
              />
            ) : (
              <div className="mt-4 min-h-0 flex-1 space-y-1 overflow-auto pb-4">
                {queue.length === 0 ? <p className="px-2 py-10 text-[14px] text-muted">{t("player_queue_empty")}</p> : null}
                {queue.map((item, index) => (
                  <TrackButton
                    key={`${item.path}-${index}`}
                    track={item}
                    active={item.path === track?.path}
                    playing={player.playing && !player.error && item.path === track?.path}
                    onClick={() => playOrderSlot(index)}
                  />
                ))}
              </div>
            )}
          </div>
        )}
      </aside>
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
      {karaoke && player.lyrics ? <Karaoke lyrics={player.lyrics} time={player.currentTime} onClose={() => setKaraoke(false)} /> : null}
    </div>
  )
}
