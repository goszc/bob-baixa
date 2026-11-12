import { useEffect, useRef } from "react"
import { ListMusic, Mic2, X } from "lucide-react"
import { setChrome, togglePanel, useChrome } from "../lib/chrome"
import { paintAccent } from "../lib/accent"
import { lyricNudgeFor } from "../lib/music"
import {
  currentTrack,
  displayArtist,
  displayTitle,
  lyricClock,
  playNext,
  playPrevious,
  seek,
  setVideoSlot,
  setVolume,
  toggleMute,
  togglePlayback,
  usePlayer,
} from "../lib/player"
import { cn } from "../lib/cn"
import { useT } from "../store"
import { FindLyrics } from "./lyrics-tools"
import { IconNext, IconPause, IconPlay, IconPrev, IconVolume } from "./player-icons"
import { Scrubber } from "./scrubber"
import { Thumb } from "./thumb"

export function FullscreenLyrics() {
  const chrome = useChrome()
  const player = usePlayer()
  const t = useT()
  const track = currentTrack(player)
  const videoRef = useRef<HTMLDivElement>(null)
  const scroller = useRef<HTMLDivElement>(null)
  const showVideo = Boolean(track) && !player.error && (track?.kind === "video" || player.playbackKind === "video")
  const lines = player.lyrics?.lines ?? []
  const nudge = track ? lyricNudgeFor(track.path) : 0
  const time = lyricClock(player.currentTime, nudge)
  let current = 0
  if (player.lyrics?.synced) {
    for (let index = 0; index < lines.length; index += 1) {
      if (lines[index].at <= time + 0.05) current = index
    }
  }
  useEffect(() => {
    if (!chrome.lyrics) return
    const node = videoRef.current
    if (showVideo && node) setVideoSlot(node)
    return () => {
      if (showVideo) setVideoSlot(null)
    }
  }, [chrome.lyrics, showVideo])

  useEffect(() => {
    if (track?.thumbnail) void paintAccent(track.thumbnail)
  }, [track?.thumbnail])

  useEffect(() => {
    const box = scroller.current
    const node = box?.querySelector<HTMLElement>("[data-lyric-active='true']")
    if (!box || !node) return
    const top = node.offsetTop - box.clientHeight * 0.33
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches
    box.scrollTo({ top: Math.max(0, top), behavior: reduce ? "auto" : "smooth" })
  }, [current, chrome.lyrics, lines.length])

  if (!chrome.lyrics) return null

  return (
    <div className="fixed inset-0 z-50 flex flex-col bg-[#14110e] text-white">
      <div className="absolute inset-0" style={{ background: "color-mix(in srgb, var(--accent) 62%, #14110e)" }} />
      {track?.thumbnail ? <img src={track.thumbnail} alt="" className="lyric-wash" /> : null}
      <div className="absolute inset-0 bg-black/45" />
      <button type="button" aria-label={t("close")} className="absolute top-4 right-4 z-10 grid h-10 w-10 place-items-center rounded-full bg-black/40" onClick={() => setChrome({ lyrics: false, karaoke: false })}>
        <X className="h-5 w-5" />
      </button>
      <div className="relative z-[1] flex min-h-0 flex-1 items-stretch pt-16 pb-4">
        <div className="flex w-[40%] shrink-0 items-center justify-center px-[4vw]">
          <div className="w-[min(260px,100%)]">
            <div className="relative aspect-square overflow-hidden rounded-2xl bg-black/30">
              <Thumb src={track?.thumbnail} videoUrl={track?.pageUrl || track?.title} className="absolute inset-0 h-full w-full rounded-none" />
              {showVideo ? <div ref={videoRef} className="stage-video absolute inset-0" /> : null}
            </div>
            <p className="mt-4 truncate text-[18px] font-medium tracking-[-0.03em]">{displayTitle(player)}</p>
            <p className="truncate text-[14px] text-white/70">{displayArtist(player)}</p>
          </div>
        </div>
        <div ref={scroller} className="h-full w-[60%] overflow-y-auto pr-[4vw] pl-[8vw] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
          {lines.length === 0 ? (
            <div className="flex h-full items-center">
              <div>
                <p className="text-[40px] font-semibold tracking-[-0.04em]">{t("player_lyrics_missing")}</p>
                <button type="button" className="mt-6 h-10 rounded-full bg-white/15 px-4 text-[14px] transition-colors hover:bg-white/25" onClick={() => setChrome({ find: true })}>
                  {t("lyrics_find")}
                </button>
              </div>
            </div>
          ) : (
            <>
              <div style={{ height: "28vh" }} />
              <div className="space-y-6">
                {lines.map((line, index) => {
                  const on = index === current
                  const near = Math.abs(index - current) === 1
                  return (
                    <button
                      key={`${line.at}-${index}`}
                      type="button"
                      data-lyric-active={on ? "true" : "false"}
                      className={cn(
                        "block w-full max-w-[920px] text-left transition-all duration-300",
                        on
                          ? "text-[52px] leading-[1.12] font-semibold tracking-[-0.045em] text-white"
                          : near
                            ? "text-[30px] leading-snug text-white/50 blur-[0.3px]"
                            : "text-[28px] leading-snug text-white/35 blur-[0.8px]",
                      )}
                      onClick={() => player.lyrics?.synced && seek(line.at)}
                    >
                      {line.text}
                    </button>
                  )
                })}
              </div>
              <div style={{ height: "42vh" }} />
            </>
          )}
        </div>
      </div>
      <div className="relative z-[1] border-t border-white/10 bg-black/45 px-6 py-3">
        <div className="mx-auto flex max-w-3xl items-center gap-3">
          <button type="button" aria-label={t("player_prev")} className="grid h-9 w-9 place-items-center rounded-full transition-colors hover:bg-white/15" onClick={playPrevious}>
            <IconPrev className="h-4 w-4" />
          </button>
          <button type="button" aria-label={player.playing ? t("pause") : t("player_play")} className="grid h-11 w-11 place-items-center rounded-full bg-white text-black" onClick={togglePlayback}>
            {player.playing ? <IconPause className="h-4 w-4" /> : <IconPlay className="h-4 w-4" />}
          </button>
          <button type="button" aria-label={t("player_next")} className="grid h-9 w-9 place-items-center rounded-full transition-colors hover:bg-white/15" onClick={playNext}>
            <IconNext className="h-4 w-4" />
          </button>
          <div className="min-w-0 flex-1">
            <Scrubber value={player.currentTime} max={player.duration} label={t("player_now")} onChange={seek} />
          </div>
          <button type="button" aria-label={t("player_volume")} title={t("player_volume")} className="grid h-10 w-10 place-items-center rounded-full transition-colors hover:bg-white/15" onClick={toggleMute}>
            <IconVolume className="h-4 w-4" muted={player.muted || player.volume === 0} />
          </button>
          <div className="w-24">
            <Scrubber value={player.muted ? 0 : player.volume} max={1} label={t("player_volume")} onChange={setVolume} />
          </div>
          <button
            type="button"
            aria-label={t("lyrics_karaoke")}
            title={t("lyrics_karaoke")}
            aria-pressed={chrome.karaoke}
            className={cn("grid h-10 w-10 place-items-center rounded-full transition-colors", chrome.karaoke ? "bg-white text-black" : "text-white/80 hover:bg-white/15 hover:text-white")}
            onClick={() => setChrome({ karaoke: !chrome.karaoke })}
          >
            <Mic2 className="h-4 w-4" />
          </button>
          <button
            type="button"
            aria-label={t("player_queue")}
            title={t("player_queue")}
            aria-pressed={chrome.open}
            className={cn("grid h-10 w-10 place-items-center rounded-full transition-colors", chrome.open ? "bg-white/20 text-white" : "text-white/80 hover:bg-white/15 hover:text-white")}
            onClick={togglePanel}
          >
            <ListMusic className="h-4 w-4" />
          </button>
        </div>
      </div>
      {chrome.find && track ? (
        <FindLyrics
          path={track.path}
          title={track.title}
          artist={track.artist}
          duration={player.duration || track.duration}
          onClose={() => setChrome({ find: false })}
          onPinned={() => setChrome({ find: false })}
        />
      ) : null}
    </div>
  )
}
