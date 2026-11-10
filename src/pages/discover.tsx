import { useState } from "react"
import { createPlaylist, useMusic } from "../lib/music"
import { currentTrack, usePlayer } from "../lib/player"
import { enqueueItems, useApp, useT } from "../store"

export function Discover() {
  const app = useApp()
  const player = usePlayer()
  const music = useMusic()
  const t = useT()
  const [note, setNote] = useState("")
  const track = currentTrack(player)

  function seeds() {
    const counts = new Map<string, number>()
    for (const event of music.events) {
      if (!event.artist) continue
      counts.set(event.artist, (counts.get(event.artist) || 0) + 1)
    }
    const top = [...counts.entries()]
      .sort((left, right) => right[1] - left[1])
      .slice(0, 6)
      .map(([artist]) => artist)
    const list = []
    if (track?.artist || track?.title) list.push(`${track.artist} ${track.title}`.trim())
    list.push(...top)
    return list.length ? [...new Set(list)] : ["indie folk"]
  }

  async function one() {
    const seed = seeds()[0]
    const audio = app.config.audioOnly
    await enqueueItems([
      {
        url: `ytsearch1:${seed}`,
        title: seed,
        audioOnly: audio,
        container: audio ? undefined : "mp4",
        audioFormat: audio ? app.config.audioFormat || "mp3" : undefined,
        tagArtist: track?.artist,
        tagAlbum: track?.album,
        thumbnail: track?.thumbnail,
        playlistTitle: "Descobrir",
      },
    ])
    setNote(t("discover_queued"))
  }

  async function mix() {
    const list = seeds()
    createPlaylist(`Mix ${new Date().toLocaleDateString("pt-BR")}`)
    const items = Array.from({ length: 10 }, (_, index) => {
      const seed = list[index % list.length]
      return {
        url: `ytsearch1:${seed} mix`,
        title: `${seed} ${index + 1}`,
        audioOnly: false,
        container: "mp4",
        formatId: "bv*[height<=480]+ba/b[height<=480]/b",
        tagArtist: seed,
        thumbnail: track?.thumbnail,
        playlistTitle: "Descobrir mix",
      }
    })
    await enqueueItems(items)
    setNote(t("discover_mix_ready"))
  }

  return (
    <div>
      <h1 className="text-[28px] font-medium tracking-[-0.04em]">{t("nav_discover")}</h1>
      <p className="mt-2 max-w-[460px] text-[14px] leading-relaxed text-muted">{t("discover_sub")}</p>
      <div className="mt-6 flex gap-3">
        <button type="button" className="h-11 flex-1 rounded-full text-[14px] font-medium" style={{ background: "var(--accent)", color: "#101010" }} onClick={() => void one()}>
          {t("discover_one")}
        </button>
        <button type="button" className="h-11 flex-1 rounded-full border border-line text-[14px]" onClick={() => void mix()}>
          {t("discover_mix")}
        </button>
      </div>
      {note ? <p className="mt-4 text-[13px] text-muted">{note}</p> : null}
    </div>
  )
}
