import { PageHeader } from "../components/page-header"
import { Segmented } from "../components/segmented"
import { Thumb } from "../components/thumb"
import { Button } from "../components/ui/button"
import { Input } from "../components/ui/input"
import { formatClock } from "../lib/format"
import { enqueueHit, runSearch, setField, useApp, useT } from "../store"

export function Search() {
  const app = useApp()
  const t = useT()
  return (
    <div>
      <PageHeader title={t("search_title")} sub={t("search_sub")} />
      <form
        className="flex gap-2"
        onSubmit={(event) => {
          event.preventDefault()
          void runSearch()
        }}
      >
        <Input
          value={app.searchQuery}
          placeholder={t("search_placeholder")}
          onChange={(event) => setField("searchQuery", event.target.value)}
        />
        <Button type="submit" disabled={app.busy === "search"}>
          {app.busy === "search" ? t("searching") : t("search_btn")}
        </Button>
      </form>
      <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
        <div>
          <p className="text-[14px]">{t("search_format")}</p>
          <p className="mt-1 text-[13px] text-muted">{t("search_format_hint")}</p>
        </div>
        <Segmented
          value={app.searchFormat}
          onChange={(value) => setField("searchFormat", value)}
          options={[
            { value: "mp3", label: "mp3" },
            { value: "mp4", label: "mp4" },
          ]}
        />
      </div>
      {app.error && app.page === "search" ? <p className="mt-3 text-[13px] text-danger">{app.error}</p> : null}
      {app.searchHits.length === 0 ? (
        <div className="border-t border-line mt-8 py-16">
          <p className="text-[15px]">{t("search_empty")}</p>
          <p className="mt-2 text-[13px] text-muted">{t("empty_search_hint")}</p>
        </div>
      ) : (
        <ul className="mt-8 border-t border-line">
          {app.searchHits.map((hit) => (
            <li key={hit.id} className="flex items-center gap-4 border-b border-line py-4">
              <Thumb src={hit.thumbnail} videoUrl={hit.url} className="h-[68px] w-[120px]" />
              <div className="min-w-0 flex-1">
                <h2 className="truncate text-[15px] font-medium tracking-tight select-text">{hit.title}</h2>
                <p className="mt-1 text-[13px] text-muted">
                  {[hit.channel, hit.duration ? formatClock(hit.duration) : null].filter(Boolean).join(" · ")}
                </p>
              </div>
              <Button size="sm" variant="outline" onClick={() => void enqueueHit(hit)}>
                {t("add_queue")}
              </Button>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
