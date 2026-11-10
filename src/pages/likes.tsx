import { playTracks } from "../lib/player"
import { trackKey } from "../lib/library"
import { useMusic } from "../lib/music"
import { useLibraryTracks } from "../lib/use-library"
import { go, useT } from "../store"
import { Thumb } from "../components/thumb"

export function Likes() {
  const music = useMusic()
  const tracks = useLibraryTracks()
  const t = useT()
  const loved = tracks.filter((track) => music.likes.includes(trackKey(track.path)))
  return (
    <div>
      <h1 className="text-[28px] font-medium tracking-[-0.04em]">{t("nav_likes")}</h1>
      <p className="mt-2 text-[14px] text-muted">{loved.length}</p>
      {loved.length === 0 ? (
        <div className="mt-10 max-w-[360px]">
          <p className="text-[16px] leading-relaxed">{t("empty_likes")}</p>
          <button type="button" className="mt-4 h-10 rounded-full px-4 text-[14px]" style={{ background: "var(--accent)", color: "#101010" }} onClick={() => go("discover")}>
            {t("empty_action")}
          </button>
        </div>
      ) : (
        <ul className="mt-6">
          {loved.map((track, index) => (
            <li key={track.path}>
              <button type="button" className="flex h-16 w-full items-center gap-3 rounded-xl px-2 text-left hover:bg-soft" onClick={() => playTracks(loved, index)}>
                <Thumb src={track.thumbnail} videoUrl={track.pageUrl || track.title} className="h-12 w-12 rounded-lg" />
                <span className="min-w-0">
                  <span className="block truncate text-[14px]">{track.title}</span>
                  <span className="block truncate text-[12px] text-muted">{track.artist}</span>
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
