import { useState } from "react"
import { Thumb } from "../components/thumb"
import { rememberDiscovery, useDiscoveries } from "../lib/discoveries"
import type { Track } from "../lib/library"
import { createPlaylist, useMusic } from "../lib/music"
import { currentTrack, playTracks, usePlayer } from "../lib/player"
import { useLibraryTracks } from "../lib/use-library"
import { enqueueItems, useApp, useT } from "../store"

export function Discover() {
  const app = useApp()
  const player = usePlayer()
  const music = useMusic()
  const tracks = useLibraryTracks()
  const history = useDiscoveries()
  const t = useT()
  const [note, setNote] = useState("")
  const track = currentTrack(player)
  const ideas = suggestions(tracks, track, music.events)

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
    rememberDiscovery({ title: seed, artist: track?.artist || seed, thumbnail: track?.thumbnail })
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
    rememberDiscovery({ title: `Mix · ${list[0]}`, artist: list[0], thumbnail: track?.thumbnail })
    await enqueueItems(items)
    setNote(t("discover_mix_ready"))
  }

  async function download(item: Track) {
    const audio = app.config.audioOnly
    rememberDiscovery({ title: item.title, artist: item.artist, thumbnail: item.thumbnail })
    await enqueueItems([
      {
        url: item.pageUrl || `ytsearch1:${item.artist} ${item.title}`,
        title: item.title,
        audioOnly: audio,
        container: audio ? undefined : "mp4",
        audioFormat: audio ? app.config.audioFormat || "mp3" : undefined,
        tagArtist: item.artist,
        tagAlbum: item.album,
        thumbnail: item.thumbnail,
      },
    ])
  }

  return (
    <div>
      <h1 className="text-[28px] font-medium tracking-[-0.04em]">{t("nav_discover")}</h1>
      <p className="mt-2 max-w-[520px] text-[14px] leading-relaxed text-muted">{t("discover_sub")}</p>
      <div className="mt-6 flex max-w-[640px] gap-3">
        <button type="button" className="h-11 flex-1 rounded-full text-[14px] font-medium" style={{ background: "var(--accent)", color: "#101010" }} onClick={() => void one()}>
          {t("discover_one")}
        </button>
        <button type="button" className="h-11 flex-1 rounded-full border border-line text-[14px]" onClick={() => void mix()}>
          {t("discover_mix")}
        </button>
      </div>
      {note ? <p className="mt-4 text-[13px] text-muted">{note}</p> : null}
      <section className="mt-10">
        <h2 className="text-[22px] font-medium tracking-[-0.03em]">{t("discover_suggest")}</h2>
        {ideas.length === 0 ? (
          <p className="mt-4 max-w-[420px] text-[15px] leading-relaxed text-muted">{t("discover_none")}</p>
        ) : (
          <div className="home-grid mt-4">
            {ideas.map((item) => (
              <article key={item.path} className="rounded-2xl bg-soft p-3">
                <Thumb src={item.thumbnail} videoUrl={item.pageUrl || item.title} className="aspect-square w-full rounded-xl" />
                <p className="mt-3 truncate text-[14px]">{item.title}</p>
                <p className="truncate text-[12px] text-muted">{item.artist}</p>
                <div className="mt-3 flex gap-2">
                  <button type="button" className="h-8 rounded-full bg-surface px-3 text-[12px]" onClick={() => playTracks([item], 0)}>
                    {t("discover_preview")}
                  </button>
                  <button type="button" className="h-8 rounded-full px-3 text-[12px]" style={{ background: "var(--accent)", color: "#101010" }} onClick={() => void download(item)}>
                    {t("discover_get")}
                  </button>
                </div>
              </article>
            ))}
          </div>
        )}
      </section>
      <section className="mt-10">
        <h2 className="text-[22px] font-medium tracking-[-0.03em]">{t("discover_recent")}</h2>
        {history.length === 0 ? (
          <p className="mt-4 max-w-[420px] text-[15px] leading-relaxed text-muted">{t("discover_none")}</p>
        ) : (
          <ul className="mt-4 grid gap-2" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))" }}>
            {history.map((item) => (
              <li key={`${item.title}-${item.at}`} className="flex items-center gap-3 rounded-2xl bg-surface px-3 py-2">
                <Thumb src={item.thumbnail} videoUrl={item.title} className="h-12 w-12 rounded-lg" />
                <span className="min-w-0">
                  <span className="block truncate text-[14px]">{item.title}</span>
                  <span className="block truncate text-[12px] text-muted">{item.artist}</span>
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  )
}

function suggestions(tracks: Track[], current: Track | null, events: { artist: string; path: string }[]) {
  const artists = new Set<string>()
  if (current?.artist) artists.add(current.artist)
  const counts = new Map<string, number>()
  for (const event of events) {
    if (!event.artist) continue
    counts.set(event.artist, (counts.get(event.artist) || 0) + 1)
  }
  for (const [artist] of [...counts.entries()].sort((left, right) => right[1] - left[1]).slice(0, 4)) artists.add(artist)
  return tracks.filter((item) => artists.has(item.artist) && item.path !== current?.path).slice(0, 8)
}
