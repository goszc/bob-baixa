import { useEffect } from "react"
import { Shell } from "./components/shell"
import { Discover } from "./pages/discover"
import { Download } from "./pages/download"
import { Home } from "./pages/home"
import { LibraryPage } from "./pages/library"
import { Likes } from "./pages/likes"
import { Player } from "./pages/player"
import { Playlists } from "./pages/playlists"
import { seedDemoMusic } from "./lib/demo-library"
import { Search } from "./pages/search"
import { Settings } from "./pages/settings"
import { MiniPlayer } from "./components/mini-player"
import { go, initApp, useApp } from "./store"

const miniWindow = new URLSearchParams(window.location.search).get("mini") === "1"

export default function App() {
  const page = useApp().page
  useEffect(() => {
    if (!miniWindow) initApp()
    seedDemoMusic()
    const demo = new URLSearchParams(window.location.search).get("demo")
    if (demo === "player" || demo === "cinema" || demo === "lyrics" || demo === "video") go("player")
    if (demo === "library" || demo === "menu") go("library")
    if (demo === "likes") go("likes")
    if (demo === "discover") go("discover")
    if (demo === "download" || demo === "fila-download") go("download")
    if (demo === "playlists" || demo === "playlist") go("playlists")
    if (demo === "home" || demo === "letra" || demo === "fila" || demo === "biblioteca") go("home")
    if (demo === "settings") go("settings")
  }, [])
  if (miniWindow) return <MiniPlayer />
  return (
    <Shell>
      {page === "home" && <Home />}
      {page === "library" && <LibraryPage />}
      {page === "player" && <Player />}
      {page === "likes" && <Likes />}
      {page === "discover" && <Discover />}
      {(page === "download" || page === "queue" || page === "convert" || page === "monitor" || page === "history") && <Download />}
      {page === "playlists" && <Playlists />}
      {page === "search" && <Search />}
      {page === "settings" && <Settings />}
    </Shell>
  )
}
