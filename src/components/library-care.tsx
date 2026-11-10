import { useEffect, useState } from "react"
import { invoke, isTauri, listenBob } from "../lib/api"
import { duplicatePairs } from "../lib/duplicates"
import { dailyMix } from "../lib/mix"
import { useMusic } from "../lib/music"
import { playTracks } from "../lib/player"
import type { Track } from "../lib/library"
import { enqueueItems, useApp, useT } from "../store"
import { Button } from "./ui/button"
import { Input } from "./ui/input"

interface Spot {
  path: string
  title: string
  artist: string
  line: string
  at: number
}

interface DupCopy {
  path: string
  format: string
  bitrate: number
  size: number
  lyrics: boolean
  cover: boolean
}

interface Issues {
  duplicates: { reason: string; paths: string[]; copies?: DupCopy[]; worse?: string }[]
  broken: string[]
}

function shownDuplicates(issues: Issues | null, tracks: Track[]) {
  const scanned = (issues?.duplicates ?? []).map((group) => ({
    reason: group.reason,
    worse: group.worse ?? "",
    copies: group.copies ?? group.paths.map((path) => ({ path, format: "", bitrate: 0, size: 0, lyrics: false, cover: false })),
  }))
  const known = new Set(scanned.flatMap((group) => group.copies.map((copy) => copy.path)))
  const extra = duplicatePairs(tracks)
    .filter((pair) => pair.copies.some((copy) => !known.has(copy.path)))
    .map((pair) => ({ reason: pair.reason, worse: pair.worse, copies: pair.copies.map((copy) => ({ ...copy, lyrics: false })) }))
  return [...scanned, ...extra]
}

interface TagPreview {
  path: string
  currentTitle: string
  currentArtist: string
  currentAlbum: string
  title: string
  artist: string
  album: string
  releaseId: string
  coverUrl: string
  score: number
}

export function MixList({ tracks }: { tracks: Track[] }) {
  const t = useT()
  const music = useMusic()
  const mix = dailyMix(tracks, music.events, Date.now())
  if (!mix.length) return <p className="px-2 py-8 text-[14px] text-muted">{t("mix_empty")}</p>
  return (
    <div>
      <p className="mb-3 px-2 text-[13px] text-muted">{t("mix_hint")}</p>
      <Button className="mb-3" onClick={() => playTracks(mix, 0)}>
        {t("player_play")}
      </Button>
      {mix.map((track, index) => (
        <button key={track.path} type="button" className="flex w-full items-center justify-between rounded-xl px-2 py-2 text-left hover:bg-soft" onClick={() => playTracks(mix, index)}>
          <span className="min-w-0">
            <span className="block truncate text-[14px]">{track.title}</span>
            <span className="block truncate text-[12px] text-muted">{track.artist}</span>
          </span>
        </button>
      ))}
    </div>
  )
}

export function CarePanel({ tracks }: { tracks: Track[] }) {
  const t = useT()
  const app = useApp()
  const [tab, setTab] = useState<"lyrics" | "dupes" | "tags" | "economic">("lyrics")
  const [query, setQuery] = useState("")
  const [spots, setSpots] = useState<Spot[]>([])
  const [issues, setIssues] = useState<Issues | null>(null)
  const [tags, setTags] = useState<TagPreview[]>([])
  const [picked, setPicked] = useState<Record<string, boolean>>({})
  const [deleteOriginals, setDeleteOriginals] = useState(false)
  const [progress, setProgress] = useState("")
  const [note, setNote] = useState("")

  useEffect(() => {
    if (!isTauri()) return
    let stop = () => {}
    void listenBob<{ type: string; done?: number; total?: number; current?: string; state?: string }>((event) => {
      if (event.type !== "economic") return
      setProgress(`${event.done ?? 0}/${event.total ?? 0} ${event.current ?? ""} ${event.state ?? ""}`.trim())
    }).then((unlisten) => {
      stop = unlisten
    })
    return () => stop()
  }, [])

  const search = async () => {
    if (!isTauri()) {
      setSpots(tracks.slice(0, 3).map((track) => ({ path: track.path, title: track.title, artist: track.artist, line: query, at: 0 })))
      return
    }
    const found = await invoke<Spot[]>("search_library_lyrics", { root: app.config.outputDir, query })
    setSpots(found)
  }

  const loadIssues = async () => {
    if (!isTauri()) {
      setIssues({ duplicates: [], broken: [] })
      return
    }
    setIssues(await invoke<Issues>("library_issues", { root: app.config.outputDir }))
  }

  const loadTags = async () => {
    if (!isTauri()) return
    setNote(t("care_tags_hint"))
    const paths = tracks.map((track) => track.path).filter((path) => !path.startsWith("drive:")).slice(0, 20)
    const found = await invoke<TagPreview[]>("preview_tags", { paths })
    setTags(found)
    setPicked(Object.fromEntries(found.map((item) => [item.path, true])))
  }

  return (
    <div>
      <div className="mb-3 flex flex-wrap gap-1">
        {(["lyrics", "dupes", "tags", "economic"] as const).map((id) => (
          <button key={id} type="button" className={tab === id ? "rounded-full bg-fill px-3 py-1 text-[13px] text-onfill" : "rounded-full bg-soft px-3 py-1 text-[13px] text-muted"} onClick={() => setTab(id)}>
            {t(id === "lyrics" ? "care_lyrics" : id === "dupes" ? "care_dupes" : id === "tags" ? "care_tags" : "care_economic")}
          </button>
        ))}
      </div>
      {tab === "lyrics" ? (
        <div>
          <p className="mb-2 text-[13px] text-muted">{t("care_lyrics_hint")}</p>
          <div className="mb-3 flex gap-2">
            <Input value={query} onChange={(event) => setQuery(event.target.value)} />
            <Button onClick={() => void search()}>{t("search_btn")}</Button>
          </div>
          {spots.map((spot) => (
            <button
              key={`${spot.path}-${spot.line}`}
              type="button"
              className="block w-full rounded-xl px-2 py-2 text-left hover:bg-soft"
              onClick={() => {
                const index = tracks.findIndex((track) => track.path === spot.path)
                if (index >= 0) playTracks(tracks, index)
              }}
            >
              <span className="block text-[14px]">{spot.title}</span>
              <span className="block text-[12px] text-muted">{spot.line}</span>
            </button>
          ))}
        </div>
      ) : null}
      {tab === "dupes" ? (
        <div>
          <Button className="mb-3" onClick={() => void loadIssues()}>
            {t("search_btn")}
          </Button>
          {shownDuplicates(issues, tracks).map((group) => (
            <div key={group.copies.map((copy) => copy.path).join("|")} className="mb-3 rounded-xl border border-line p-3 text-[13px]">
              <div className="mb-1 text-muted">{group.reason}</div>
              {group.copies.map((copy) => (
                <div key={copy.path} className="truncate">
                  {copy.path}
                  {copy.format ? ` · ${copy.format}` : ""}
                  {copy.bitrate ? ` · ${Math.round(copy.bitrate / 1000)} kbps` : ""}
                  {copy.lyrics ? " · letra" : ""}
                  {copy.cover ? " · capa" : ""}
                </div>
              ))}
              {group.worse ? (
                <button
                  type="button"
                  className="mt-2 text-muted hover:text-ink"
                  onClick={() => void invoke("trash_copy", { path: group.worse }).then(() => void loadIssues())}
                >
                  {t("care_trash_worse")}
                </button>
              ) : null}
            </div>
          ))}
          {issues?.broken.length ? <p className="mb-2 text-[13px]">{t("care_broken")}</p> : null}
          {issues?.broken.map((path) => (
            <div key={path} className="mb-2 flex items-center justify-between gap-2 text-[13px]">
              <span className="truncate">{path}</span>
              <button
                type="button"
                className="text-muted"
                onClick={() => {
                  const track = tracks.find((item) => item.path === path)
                  if (track?.pageUrl) void enqueueItems([{ url: track.pageUrl, title: track.title }])
                }}
              >
                {t("care_redownload")}
              </button>
            </div>
          ))}
          {issues && !issues.duplicates.length && !issues.broken.length ? <p className="text-[13px] text-muted">{t("care_none")}</p> : null}
        </div>
      ) : null}
      {tab === "tags" ? (
        <div>
          <p className="mb-2 text-[13px] text-muted">{t("care_tags_hint")}</p>
          <Button className="mb-3" onClick={() => void loadTags()}>
            {t("search_btn")}
          </Button>
          {tags.map((tag) => (
            <label key={tag.path} className="mb-2 flex items-start gap-2 text-[13px]">
              <input type="checkbox" checked={picked[tag.path] !== false} onChange={(event) => setPicked({ ...picked, [tag.path]: event.target.checked })} />
              <span>
                <span className="block">{tag.currentTitle || tag.path}</span>
                <span className="text-muted">
                  {tag.artist} — {tag.album}
                </span>
              </span>
            </label>
          ))}
          {tags.length ? (
            <Button
              onClick={() => {
                const edits = tags.filter((tag) => picked[tag.path] !== false)
                void invoke("apply_tags", { edits }).then(() => setNote(t("care_apply")))
              }}
            >
              {t("care_apply")}
            </Button>
          ) : null}
        </div>
      ) : null}
      {tab === "economic" ? (
        <div>
          <p className="mb-2 text-[13px] text-muted">{t("care_economic_hint")}</p>
          <label className="mb-3 flex items-center gap-2 text-[13px]">
            <input type="checkbox" checked={deleteOriginals} onChange={(event) => setDeleteOriginals(event.target.checked)} />
            {t("care_delete_originals")}
          </label>
          <div className="flex gap-2">
            <Button
              onClick={() => {
                void invoke("start_economic", {
                  paths: tracks.map((track) => track.path).filter((path) => !path.startsWith("drive:")),
                  deleteOriginals,
                })
                setProgress(t("care_start"))
              }}
            >
              {t("care_start")}
            </Button>
            <Button variant="outline" onClick={() => void invoke("cancel_economic")}>
              {t("care_cancel")}
            </Button>
          </div>
          {progress ? <p className="mt-2 text-[13px] text-muted">{progress}</p> : null}
        </div>
      ) : null}
      {note ? <p className="mt-3 text-[13px] text-muted">{note}</p> : null}
    </div>
  )
}
