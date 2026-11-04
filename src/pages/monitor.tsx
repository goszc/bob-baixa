import { Field } from "../components/field"
import { PageHeader } from "../components/page-header"
import { Button } from "../components/ui/button"
import { Input } from "../components/ui/input"
import { Switch } from "../components/ui/switch"
import { formatWhen } from "../lib/format"
import { addWatch, setConfig, setField, useApp, useT, watchAction } from "../store"

export function Monitor() {
  const app = useApp()
  const t = useT()
  return (
    <div>
      <PageHeader title={t("monitor_title")} sub={t("monitor_sub")} />
      <p className="mb-6 text-[13px] leading-relaxed text-muted">{t("monitor_first")}</p>
      <form
        className="flex flex-wrap items-end gap-3"
        onSubmit={(event) => {
          event.preventDefault()
          void addWatch()
        }}
      >
        <div className="min-w-[240px] flex-1">
          <Field label={t("watch_placeholder")}>
            <Input value={app.watchUrl} placeholder="https://..." onChange={(event) => setField("watchUrl", event.target.value)} />
          </Field>
        </div>
        <Field label={t("every")}>
          <div className="flex items-center gap-2">
            <Input
              type="number"
              min={5}
              max={1440}
              className="w-[88px]"
              value={app.watchEvery}
              onChange={(event) => setField("watchEvery", Number(event.target.value))}
            />
            <span className="text-[13px] text-muted">{t("minutes")}</span>
          </div>
        </Field>
        <Button type="submit">{t("watch_add")}</Button>
      </form>
      <div className="mt-4 flex items-center justify-between gap-6 py-3">
        <span className="text-[14px]">{t("download_known")}</span>
        <Switch
          label={t("download_known")}
          checked={app.config.downloadKnownOnFirstWatch}
          onCheckedChange={(value) => setConfig({ downloadKnownOnFirstWatch: value })}
        />
      </div>
      {app.error && app.page === "monitor" ? <p className="text-[13px] text-danger">{app.error}</p> : null}
      {app.watches.length === 0 ? (
        <p className="mt-10 border-t border-line py-16 text-[15px]">{t("monitor_empty")}</p>
      ) : (
        <ul className="mt-6 border-t border-line">
          {app.watches.map((watch) => (
            <li key={watch.id} className="flex items-center gap-4 border-b border-line py-4">
              <div className="min-w-0 flex-1">
                <h2 className="truncate text-[15px] font-medium tracking-tight">{watch.title}</h2>
                <p className="mt-1 truncate text-[13px] text-muted">{watch.url}</p>
                <p className="mt-1 text-[12px] text-muted">
                  {t("every")} {watch.everyMinutes} {t("minutes")} · {t("known")} {watch.known} · {t("last_check")}{" "}
                  {watch.lastChecked ? formatWhen(watch.lastChecked, app.config.language) : t("never")}
                </p>
              </div>
              <Switch label={watch.title} checked={watch.enabled} onCheckedChange={() => void watchAction(watch.id, "toggle")} />
              <Button size="sm" variant="ghost" onClick={() => void watchAction(watch.id, "check")}>
                {t("check_now")}
              </Button>
              <Button size="sm" variant="quiet" onClick={() => void watchAction(watch.id, "remove")}>
                {t("remove")}
              </Button>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
