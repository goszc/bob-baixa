import { useState } from "react"
import { Maximize2 } from "lucide-react"
import { setChrome, setPanelWidth, useChrome, type PanelTab } from "../lib/chrome"
import { trackKey, type Track } from "../lib/library"
import { useMusic } from "../lib/music"
import { currentTrack, orderedTracks, playOrderSlot, playTracks, reorderQueue, usePlayer } from "../lib/player"
import { useLibraryTracks } from "../lib/use-library"
import { useT } from "../store"
import { cn } from "../lib/cn"
import { Lyrics } from "../pages/player"
import { FindLyrics } from "./lyrics-tools"
import { Thumb } from "./thumb"
import { VirtualList } from "./virtual-list"

export function SidePanel() {
  const chrome = useChrome()
  const player = usePlayer()
  const music = useMusic()
  const tracks = useLibraryTracks()
  const t = useT()
  const queue = orderedTracks(player)
  const track = currentTrack(player)
  const loved = tracks.filter((item) => music.likes.includes(trackKey(item.path))).slice(0, 40)
  const list = loved.length ? loved : tracks.slice(0, 40)

  return (
    <aside
      className={cn("relative hidden h-full shrink-0 overflow-hidden border-l border-line transition-[width] duration-200 lg:block", chrome.open ? "" : "border-l-0")}
      style={{ width: chrome.open ? chrome.width : 0 }}
      aria-hidden={!chrome.open}
    >
      <div
        className="absolute top-0 bottom-0 left-0 z-10 w-1 cursor-col-resize"
        onPointerDown={(event) => {
          const startX = event.clientX
          const start = chrome.width
          const move = (ev: PointerEvent) => setPanelWidth(start - (ev.clientX - startX))
          const up = () => {
            window.removeEventListener("pointermove", move)
            window.removeEventListener("pointerup", up)
          }
          window.addEventListener("pointermove", move)
          window.addEventListener("pointerup", up)
        }}
      />
      <div className="flex h-full min-h-0 flex-col" style={{ width: chrome.width }}>
        <div className="flex shrink-0 gap-1 px-3 pt-3" role="tablist" aria-label={t("player_queue")}>
          {(
            [
              ["queue", t("player_queue")],
              ["lyrics", t("player_lyrics")],
              ["library", t("nav_library")],
            ] as [PanelTab, string][]
          ).map(([value, label]) => (
            <button
              key={value}
              type="button"
              role="tab"
              aria-selected={chrome.tab === value}
              className={cn("h-8 min-w-0 flex-1 rounded-full px-2 text-[12px] whitespace-nowrap", chrome.tab === value ? "bg-soft text-ink" : "text-muted hover:text-ink")}
              onClick={() => setChrome({ tab: value })}
            >
              {label}
            </button>
          ))}
        </div>
        {chrome.tab === "lyrics" ? (
          <div className="mt-2 flex min-h-0 flex-1 flex-col">
            <div className="flex justify-end px-3">
              <button type="button" aria-label={t("lyrics_stage")} title={t("lyrics_stage")} className="grid h-8 w-8 place-items-center rounded-full text-muted hover:bg-soft hover:text-ink" onClick={() => setChrome({ lyrics: true })}>
                <Maximize2 className="h-4 w-4" />
              </button>
            </div>
            <Lyrics time={player.currentTime} onFind={() => setChrome({ find: true })} onKaraoke={() => setChrome({ lyrics: true, karaoke: true })} />
          </div>
        ) : chrome.tab === "library" ? (
          <div className="min-h-0 flex-1 overflow-auto py-2">
            {list.map((item, index) => (
              <button key={item.path} type="button" className="flex h-14 w-full items-center gap-3 px-3 text-left hover:bg-soft" onClick={() => playTracks(list, index)}>
                <Thumb src={item.thumbnail} videoUrl={item.pageUrl || item.title} className="h-10 w-10 rounded-md" />
                <span className="min-w-0">
                  <span className="block truncate text-[14px]">{item.title}</span>
                  <span className="block truncate text-[12px] text-muted">{item.artist}</span>
                </span>
              </button>
            ))}
          </div>
        ) : (
          <QueuePane queue={queue} active={player.index} playingPath={player.playing ? track?.path : ""} />
        )}
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
    </aside>
  )
}

function QueuePane({ queue, active, playingPath }: { queue: Track[]; active: number; playingPath?: string }) {
  const t = useT()
  const [dragFrom, setDragFrom] = useState<number | null>(null)
  if (!queue.length) return <p className="px-4 py-8 text-[14px] text-muted">{t("player_queue_empty")}</p>
  return (
    <VirtualList
      count={queue.length}
      rowHeight={56}
      className="h-full overflow-auto"
      render={(index) => {
        const item = queue[index]
        const on = item.path === playingPath
        return (
          <button
            type="button"
            draggable
            className={cn("flex h-14 w-full items-center gap-3 px-3 text-left", on ? "bg-soft" : "hover:bg-soft")}
            onDragStart={() => setDragFrom(index)}
            onDragOver={(event) => event.preventDefault()}
            onDrop={() => {
              if (dragFrom != null) reorderQueue(dragFrom, index)
              setDragFrom(null)
            }}
            onClick={() => playOrderSlot(index)}
          >
            <Thumb src={item.thumbnail} videoUrl={item.pageUrl || item.title} className="h-10 w-10 rounded-md" />
            <span className="min-w-0 flex-1">
              <span className="block truncate text-[14px]">{item.title}</span>
              <span className="block truncate text-[12px] text-muted">{item.artist}</span>
            </span>
            <span className="text-muted" aria-hidden="true">≡</span>
            <span className="sr-only">{active}</span>
          </button>
        )
      }}
    />
  )
}
