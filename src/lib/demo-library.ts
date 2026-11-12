import type { Track } from "./library"
import { addToPlaylist, createPlaylist, getMusic, hear, rememberSession, toggleLike, touchRecent } from "./music"

const ARTISTS = ["Low Roar", "Nils Frahm", "Agnes Obel", "Ólafur Arnalds", "Bon Iver", "Fleet Foxes", "José González", "Feist"]
const TITLES = ["I'll Keep Coming", "Says", "Familiar", "Near Light", "Holocene", "Mykonos", "Heartbeats", "Mushaboom", "Open", "Softly"]
const ALBUMS = ["Once", "Spaces", "Aventine", "re:member", "For Emma", "Fleet Foxes", "Veneer", "The Reminder"]
const SWATCH = ["#c4a574", "#7f9c96", "#8d7aa8", "#c47c6a", "#6f8fbf", "#b7a15a", "#7d8a6a", "#a98474"]

function coverSwatch(index: number) {
  const color = SWATCH[index % SWATCH.length]
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 80"><rect width="80" height="80" fill="${color}"/><path d="M34 56V24l22-6v28" fill="none" stroke="#fff" stroke-width="3" stroke-linecap="round"/><circle cx="30" cy="56" r="6" fill="#fff"/><circle cx="52" cy="46" r="6" fill="#fff"/></svg>`
  return `data:image/svg+xml,${encodeURIComponent(svg)}`
}

export function makeDemoLibrary(): Track[] {
  return Array.from({ length: 200 }, (_, index) => ({
    id: index === 0 ? "preview-1" : `demo-${index + 1}`,
    path: index === 0 ? "/preview/clip.mp4" : `/library/demo-${index + 1}.mp3`,
    title: index < TITLES.length ? TITLES[index] : `${TITLES[index % TITLES.length]} ${index + 1}`,
    artist: ARTISTS[index % ARTISTS.length],
    album: ALBUMS[index % ALBUMS.length],
    folder: "Demo",
    pageUrl: "",
    kind: index === 0 ? "video" : "audio",
    duration: 150 + (index % 90),
    thumbnail: coverSwatch(index),
  }))
}

export function demoMode() {
  if (typeof window === "undefined") return ""
  return new URLSearchParams(window.location.search).get("demo") || ""
}

export function demoTracks() {
  return demoMode() ? makeDemoLibrary() : []
}

let seeded = false
export function seedDemoMusic() {
  if (seeded || !demoMode()) return
  seeded = true
  const music = getMusic()
  const tracks = makeDemoLibrary()
  if (!music.recent.length) for (const track of tracks.slice(0, 6)) touchRecent(track.path)
  if (!music.likes.length) for (const track of tracks.slice(0, 4)) toggleLike(track.path)
  if (!music.playlists.length) {
    const id = createPlaylist("Noite")
    for (const track of tracks.slice(1, 6)) addToPlaylist(id, track.path)
  }
  if (!music.session?.paths.length) {
    rememberSession({
      paths: tracks.slice(0, 6).map((track) => track.path),
      index: 0,
      position: 4,
      shuffle: false,
      repeat: "off",
    })
  }
  if (!music.events.length) {
    const now = Date.now()
    tracks.slice(0, 8).forEach((track, index) => {
      for (let step = 0; step < 16; step += 1) hear(track.path, track.title, track.artist, track.album, 2_000, now - index * 3_600_000)
    })
  }
}
