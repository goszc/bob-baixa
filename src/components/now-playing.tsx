import type { ReactNode } from "react"
import { formatClock } from "../lib/format"
import { trackKey } from "../lib/library"
import { toggleLike, useMusic } from "../lib/music"
import { displayArtist, displayTitle, playNext, playPrevious, seek, setVolume, toggleMute, togglePlayback, usePlayer } from "../lib/player"
import { go, useT } from "../store"
import { cn } from "../lib/cn"
import { IconNext, IconPause, IconPlay, IconPrev, IconVolume } from "./player-icons"
import { Scrubber } from "./scrubber"
import { Thumb } from "./thumb"

function RoundButton({
  label,
  onClick,
  disabled,
  children,
  large,
}: {
  label: string
  onClick: () => void
  disabled?: boolean
  children: ReactNode
  large?: boolean
}) {
  return (
    <button
      type="button"
      aria-label={label}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "inline-flex items-center justify-center rounded-full text-ink transition-colors hover:bg-soft disabled:opacity-30",
        large ? "h-10 w-10 bg-fill text-onfill hover:bg-fill hover:opacity-90" : "h-9 w-9",
      )}
    >
      {children}
    </button>
  )
}

export function NowPlaying() {
  const player = usePlayer()
  const music = useMusic()
  const t = useT()
  const track = player.queue[player.index]
  const loved = track ? music.likes.includes(trackKey(track.path)) : false
  const title = displayTitle(player)
  const artist = displayArtist(player)
  const idle = !track
  return (
    <div className="shrink-0 border-t border-line bg-surface">
      <div className="px-4 pt-2 md:px-6">
        <Scrubber
          value={player.currentTime}
          max={player.duration}
          label={t("player_now")}
          disabled={idle || player.error !== null}
          onChange={seek}
        />
      </div>
      <div className="flex h-[68px] items-center gap-3 px-4 pb-2 md:px-6">
        <button type="button" className="flex min-w-0 flex-1 items-center gap-3 text-left" onClick={() => go("player")}>
          <Thumb src={track?.thumbnail} videoUrl={track?.pageUrl} className="h-12 w-12 rounded-[10px]" />
          <span className="min-w-0">
            <span className="block truncate text-[14px] font-medium">{title || t("player_idle")}</span>
            <span className="block truncate text-[12px] text-muted">
              {artist || (track ? formatClock(player.duration || track.duration) : t("nav_player"))}
            </span>
          </span>
        </button>
        {track ? (
          <button
            type="button"
            aria-pressed={loved}
            aria-label={loved ? t("player_unlike") : t("player_like")}
            className={loved ? "text-ink" : "text-muted"}
            onClick={() => toggleLike(track.path)}
          >
            ♥
          </button>
        ) : null}
        <div className="flex items-center gap-1">
          <RoundButton label={t("player_prev")} disabled={idle} onClick={playPrevious}>
            <IconPrev className="h-4 w-4" />
          </RoundButton>
          <RoundButton label={player.playing ? t("pause") : t("player_play")} disabled={idle || player.error !== null} onClick={togglePlayback} large>
            {player.playing ? <IconPause className="h-4 w-4" /> : <IconPlay className="h-4 w-4" />}
          </RoundButton>
          <RoundButton label={t("player_next")} disabled={idle} onClick={playNext}>
            <IconNext className="h-4 w-4" />
          </RoundButton>
        </div>
        <div className="flex w-[132px] shrink-0 items-center gap-1">
          <button
            type="button"
            aria-label={player.muted || player.volume === 0 ? t("player_unmute") : t("player_mute")}
            aria-pressed={player.muted || player.volume === 0}
            onClick={toggleMute}
            className="inline-flex h-9 w-9 items-center justify-center rounded-full text-muted transition-colors hover:bg-soft hover:text-ink"
          >
            <IconVolume className="h-4 w-4" muted={player.muted || player.volume === 0} />
          </button>
          <div className="min-w-0 flex-1">
            <Scrubber value={player.muted ? 0 : player.volume} max={1} label={t("player_volume")} onChange={setVolume} />
          </div>
        </div>
        <span className="hidden w-16 text-right text-[12px] tabular-nums text-muted sm:block">
          {idle ? "" : formatClock(player.currentTime)}
        </span>
      </div>
    </div>
  )
}
