import { PageHeader } from "../components/page-header"
import { Thumb } from "../components/thumb"
import { Button } from "../components/ui/button"
import { formatWhen } from "../lib/format"
import { askClearHistory, openOutput, useApp, useT } from "../store"

export function History() {
  const app = useApp()
  const t = useT()
  return (
    <div>
      <PageHeader title={t("history_title")} sub={t("history_sub")} />
      {app.history.length > 0 ? (
        <div className="mb-4 flex justify-end">
          <Button variant="ghost" onClick={askClearHistory}>
            {t("clear_history")}
          </Button>
        </div>
      ) : null}
      {app.history.length === 0 ? (
        <p className="border-t border-line py-16 text-[15px]">{t("empty_history")}</p>
      ) : (
        <ul className="border-t border-line">
          {app.history.map((entry) => (
            <li key={entry.id} className="flex items-center gap-4 border-b border-line py-4">
              <Thumb videoUrl={entry.url} className="h-[56px] w-[100px]" />
              <div className="min-w-0 flex-1">
                <h2 className="truncate text-[15px] font-medium tracking-tight select-text">{entry.title}</h2>
                <p className="mt-1 text-[13px] text-muted">
                  {formatWhen(entry.at, app.config.language)} · {entry.ok ? t("state_done") : t("state_failed")}
                </p>
                {entry.detail ? <p className="mt-1 text-[13px] text-danger">{entry.detail}</p> : null}
              </div>
              {entry.path ? (
                <Button size="sm" variant="outline" onClick={() => void openOutput(entry.path!)}>
                  {t("open_folder")}
                </Button>
              ) : null}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
