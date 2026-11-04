import { useEffect } from "react"
import { Shell } from "./components/shell"
import { Convert } from "./pages/convert"
import { History } from "./pages/history"
import { Home } from "./pages/home"
import { Monitor } from "./pages/monitor"
import { Queue } from "./pages/queue"
import { Search } from "./pages/search"
import { Settings } from "./pages/settings"
import { initApp, useApp } from "./store"

export default function App() {
  const page = useApp().page
  useEffect(() => {
    initApp()
  }, [])
  return (
    <Shell>
      {page === "home" && <Home />}
      {page === "queue" && <Queue />}
      {page === "search" && <Search />}
      {page === "convert" && <Convert />}
      {page === "monitor" && <Monitor />}
      {page === "history" && <History />}
      {page === "settings" && <Settings />}
    </Shell>
  )
}
