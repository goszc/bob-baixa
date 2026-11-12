import { Shuffle } from "lucide-react"
import { TrackTable } from "../components/track-table"
import { trackKey } from "../lib/library"
import { useMusic } from "../lib/music"
import { playTracks } from "../lib/player"
import { useLibraryTracks } from "../lib/use-library"
import { useT } from "../store"

export function Likes() {
  const music = useMusic()
  const tracks = useLibraryTracks().filter((track) => music.likes.includes(trackKey(track.path)))
  const t = useT()
  return (
    <div>
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-[28px] font-medium tracking-[-0.04em]">{t("nav_likes")}</h1>
          <p className="mt-1 text-[13px] text-muted">{tracks.length}</p>
        </div>
        <div className="flex items-center gap-2">
          <button type="button" className="h-10 rounded-full px-4 text-[14px] font-medium" style={{ background: "var(--accent)", color: "#101010" }} onClick={() => tracks.length && playTracks(tracks, 0)}>
            {t("play_all")}
          </button>
          <button
            type="button"
            className="inline-flex h-10 items-center gap-2 rounded-full border border-line px-4 text-[14px]"
            onClick={() => tracks.length && playTracks(tracks, Math.floor(Math.random() * tracks.length))}
          >
            <Shuffle className="h-4 w-4" />
            {t("shuffle_play")}
          </button>
        </div>
      </header>
      <div className="mt-6">
        {tracks.length ? <TrackTable tracks={tracks} /> : <p className="max-w-[420px] text-[15px] text-muted">{t("empty_likes")}</p>}
      </div>
    </div>
  )
}
