import { useEffect, useRef } from "react"
import { X } from "lucide-react"
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
  const upcoming = chrome.karaoke ? lines.slice(current, current + 4) : lines.slice(Math.max(0, current - 1), current + 6)

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

  if (!chrome.lyrics) return null

  return (
    <div className="fixed inset-0 z-50 flex flex-col bg-[#14110e] text-white">
      <div className="absolute inset-0" style={{ background: "color-mix(in srgb, var(--accent) 62%, #14110e)" }} />
      {track?.thumbnail ? <img src={track.thumbnail} alt="" className="lyric-wash" /> : null}
      <div className="absolute inset-0 bg-black/45" />
      <button type="button" aria-label={t("close")} className="absolute top-4 right-4 z-10 grid h-10 w-10 place-items-center rounded-full bg-black/40" onClick={() => setChrome({ lyrics: false, karaoke: false })}>
        <X className="h-5 w-5" />
      </button>
      <div className="relative z-[1] flex min-h-0 flex-1 items-center gap-10 px-10 pt-16 pb-4">
        <div className="min-w-0 flex-1">
          {lines.length === 0 ? (
            <div>
              <p className="text-[28px] font-medium tracking-[-0.03em]">{t("player_lyrics_missing")}</p>
              <button type="button" className="mt-6 h-10 rounded-full bg-white/15 px-4 text-[14px]" onClick={() => setChrome({ find: true })}>
                {t("lyrics_find")}
              </button>
            </div>
          ) : (
            <div className="space-y-3">
              {upcoming.map((line, offset) => {
                const index = (chrome.karaoke ? current : Math.max(0, current - 1)) + offset
                const on = index === current
                return (
                  <button
                    key={`${line.at}-${index}`}
                    type="button"
                    className={cn("block max-w-[820px] text-left transition-all", on ? "text-[40px] font-medium leading-tight tracking-[-0.04em]" : "text-[22px] text-white/45")}
                    onClick={() => player.lyrics?.synced && seek(line.at)}
                  >
                    {line.text}
                  </button>
                )
              })}
            </div>
          )}
          <button type="button" className="mt-8 text-[13px] text-white/70" onClick={() => setChrome({ karaoke: !chrome.karaoke })}>
            {t("lyrics_karaoke")}
          </button>
        </div>
        <div className="hidden w-[min(36%,420px)] shrink-0 lg:block">
          <div className="relative aspect-square overflow-hidden rounded-3xl bg-black/30">
            <Thumb src={track?.thumbnail} videoUrl={track?.pageUrl || track?.title} className="absolute inset-0 h-full w-full rounded-none" />
            <div ref={videoRef} className="stage-video absolute inset-0" />
          </div>
        </div>
      </div>
      <div className="relative z-[1] border-t border-white/10 bg-black/45 px-6 py-3">
        <div className="mx-auto flex max-w-3xl items-center gap-3">
          <button type="button" aria-label={t("player_prev")} className="grid h-9 w-9 place-items-center" onClick={playPrevious}>
            <IconPrev className="h-4 w-4" />
          </button>
          <button type="button" aria-label={player.playing ? t("pause") : t("player_play")} className="grid h-11 w-11 place-items-center rounded-full bg-white text-black" onClick={togglePlayback}>
            {player.playing ? <IconPause className="h-4 w-4" /> : <IconPlay className="h-4 w-4" />}
          </button>
          <button type="button" aria-label={t("player_next")} className="grid h-9 w-9 place-items-center" onClick={playNext}>
            <IconNext className="h-4 w-4" />
          </button>
          <div className="min-w-0 flex-1">
            <Scrubber value={player.currentTime} max={player.duration} label={t("player_now")} onChange={seek} />
          </div>
          <button type="button" aria-label={t("player_volume")} onClick={toggleMute}>
            <IconVolume className="h-4 w-4" muted={player.muted || player.volume === 0} />
          </button>
          <div className="w-24">
            <Scrubber value={player.muted ? 0 : player.volume} max={1} label={t("player_volume")} onChange={setVolume} />
          </div>
          <button type="button" aria-label={t("panel_expand")} aria-pressed={chrome.open} onClick={togglePanel}>
            <span className="text-[12px]">{t("player_queue")}</span>
          </button>
        </div>
        <p className="mt-1 truncate text-center text-[12px] text-white/70">
          {displayTitle(player)} · {displayArtist(player)}
        </p>
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
