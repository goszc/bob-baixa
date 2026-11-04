import { Field } from "../components/field"
import { PageHeader } from "../components/page-header"
import { Progress } from "../components/progress"
import { Button } from "../components/ui/button"
import { Select } from "../components/ui/select"
import { fileName } from "../lib/format"
import type { ConvertPreset } from "../lib/types"
import { cancelConvert, pickConvertFile, setField, startConvert, useApp, useT } from "../store"

export function Convert() {
  const app = useApp()
  const t = useT()
  const running = app.convertJob?.state === "running"
  return (
    <div>
      <PageHeader title={t("convert_title")} sub={t("convert_sub")} />
      <div className="space-y-5">
        <Field label={t("choose_file")}>
          <div className="flex gap-2">
            <div className="flex h-10 min-w-0 flex-1 items-center rounded-[10px] border border-line bg-surface px-3 text-[14px]">
              <span className="truncate text-muted">{app.convertInput ? fileName(app.convertInput) : t("no_file")}</span>
            </div>
            <Button variant="outline" onClick={() => void pickConvertFile()}>
              {t("choose_file")}
            </Button>
          </div>
        </Field>
        <Field label={t("preset")}>
          <Select value={app.convertPreset} onChange={(event) => setField("convertPreset", event.target.value as ConvertPreset)}>
            <option value="mp3">mp3</option>
            <option value="m4a">m4a</option>
            <option value="opus">opus</option>
            <option value="flac">flac</option>
            <option value="mp4">{t("preset_mp4")}</option>
            <option value="webm">webm</option>
          </Select>
        </Field>
        {app.convertPreset !== "flac" && app.convertPreset !== "mp4" && app.convertPreset !== "webm" ? (
          <Field label={t("bitrate")}>
            <Select value={app.convertBitrate} onChange={(event) => setField("convertBitrate", event.target.value)}>
              {["320k", "256k", "192k", "128k", "96k"].map((rate) => (
                <option key={rate} value={rate}>
                  {rate}
                </option>
              ))}
            </Select>
          </Field>
        ) : null}
        <div className="flex gap-2">
          <Button disabled={!app.convertInput || running} onClick={() => void startConvert()}>
            {running ? t("converting") : t("convert_btn")}
          </Button>
          {running ? (
            <Button variant="ghost" onClick={cancelConvert}>
              {t("cancel")}
            </Button>
          ) : null}
        </div>
        {app.convertJob ? (
          <div className="pt-2">
            <Progress value={app.convertJob.progress} active={app.convertJob.state === "running"} />
            <p className="mt-2 text-[13px] text-muted">
              {app.convertJob.state === "done" ? app.convertJob.output : app.convertJob.error || t(app.convertJob.state === "failed" ? "convert_fail" : "converting")}
            </p>
          </div>
        ) : null}
        {app.error && app.page === "convert" ? <p className="text-[13px] text-danger">{app.error}</p> : null}
      </div>
    </div>
  )
}
