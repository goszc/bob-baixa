import { useMemo } from "react"
import { catalogTracks, useCatalog } from "./catalog"
import { demoTracks } from "./demo-library"
import { useApp } from "../store"

export function useLibraryTracks() {
  const app = useApp()
  const disk = useCatalog()
  const demo = demoTracks()
  return useMemo(() => {
    if (demo.length) return demo
    return catalogTracks(app.jobs, app.history, app.convertJob, app.config.outputDir)
  }, [app.jobs, app.history, app.convertJob, app.config.outputDir, disk, demo])
}
