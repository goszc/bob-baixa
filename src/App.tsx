import { useEffect } from "react"
import { Shell } from "./components/shell"
import { Convert } from "./pages/convert"
import { History } from "./pages/history"
import { Discover } from "./pages/discover"
import { Download } from "./pages/download"
import { Home } from "./pages/home"
import { LibraryPage } from "./pages/library"
import { Likes } from "./pages/likes"
import { Player } from "./pages/player"
import { Playlists } from "./pages/playlists"
import { seedDemoMusic } from "./lib/demo-library"
import { Monitor } from "./pages/monitor"
import { Queue } from "./pages/queue"
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
    if (demo === "download") go("download")
    if (demo === "playlists") go("playlists")
    if (demo === "home") go("home")
  }, [])
  if (miniWindow) return <MiniPlayer />
  return (
    <Shell>
      {page === "home" && <Home />}
      {page === "library" && <LibraryPage />}
      {page === "player" && <Player />}
      {page === "likes" && <Likes />}
      {page === "discover" && <Discover />}
      {page === "download" && <Download />}
      {page === "playlists" && <Playlists />}
      {page === "queue" && <Queue />}
      {page === "search" && <Search />}
      {page === "convert" && <Convert />}
      {page === "monitor" && <Monitor />}
      {page === "history" && <History />}
      {page === "settings" && <Settings />}
    </Shell>
  )
}
