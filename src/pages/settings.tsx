import type { ReactNode } from "react"
import { PageHeader } from "../components/page-header"
import { Segmented } from "../components/segmented"
import { Button } from "../components/ui/button"
import { Input } from "../components/ui/input"
import { Select } from "../components/ui/select"
import { Switch } from "../components/ui/switch"
import type { Key } from "../lib/i18n"
import { cn } from "../lib/cn"
import type { CookiesBrowser, Lang, ThemeMode } from "../lib/types"
import { checkUpdates, pickFolder, reinstallTools, setConfig, updateYtDlp, useApp, useT } from "../store"

const CATS: { id: string; key: Key }[] = [
  { id: "sponsor", key: "cat_sponsor" },
  { id: "intro", key: "cat_intro" },
  { id: "outro", key: "cat_outro" },
  { id: "selfpromo", key: "cat_selfpromo" },
  { id: "interaction", key: "cat_interaction" },
  { id: "preview", key: "cat_preview" },
  { id: "filler", key: "cat_filler" },
  { id: "music_offtopic", key: "cat_music_offtopic" },
]

const BROWSERS: CookiesBrowser[] = ["auto", "none", "chrome", "edge", "firefox", "brave", "opera", "vivaldi", "chromium"]

export function Settings() {
  const app = useApp()
  const t = useT()
  const config = app.config
  return (
    <div>
      <PageHeader title={t("settings_title")} sub={t("settings_sub")} />
      <Section title={t("appearance")} first>
        <Row label={t("theme")} hint={t("theme_hint")}>
          <Segmented<ThemeMode>
            value={config.theme}
            onChange={(theme) => setConfig({ theme })}
            options={[
              { value: "system", label: t("theme_system") },
              { value: "light", label: t("theme_light") },
              { value: "dark", label: t("theme_dark") },
            ]}
          />
        </Row>
        <Row label={t("language")} hint={t("language_hint")}>
          <Segmented<Lang>
            value={config.language}
            onChange={(language) => setConfig({ language })}
            options={[
              { value: "pt", label: t("portuguese") },
              { value: "en", label: t("english") },
            ]}
          />
        </Row>
      </Section>
      <Section title={t("destination")}>
        <Row label={t("folder")}>
          <div className="flex gap-2">
            <Input readOnly className="w-[240px]" value={config.outputDir} placeholder="Downloads/bob baixa" />
            <Button variant="outline" onClick={() => void pickFolder()}>
              {t("choose_folder")}
            </Button>
          </div>
        </Row>
        <div className="border-b border-line py-4">
          <div className="text-[14px]">{t("filename")}</div>
          <p className="mt-1 text-[13px] text-muted">{t("filename_hint")}</p>
          <Input className="mt-3" value={config.filenameTemplate} onChange={(event) => setConfig({ filenameTemplate: event.target.value })} />
        </div>
      </Section>
      <Section title={t("queue_settings")}>
        <Row label={t("parallel")}>
          <Segmented
            value={String(config.parallel)}
            onChange={(value) => setConfig({ parallel: Number(value) })}
            options={[1, 2, 3, 4].map((count) => ({ value: String(count), label: String(count) }))}
          />
        </Row>
        <Row label={t("global_speed")}>
          <Input className="w-[120px]" placeholder="2m" value={config.globalSpeed} onChange={(event) => setConfig({ globalSpeed: event.target.value })} />
        </Row>
        <Row label={t("clipboard_watch")} hint={t("clipboard_hint")}>
          <Switch label={t("clipboard_watch")} checked={config.clipboardWatch} onCheckedChange={(value) => setConfig({ clipboardWatch: value })} />
        </Row>
        <Row label={t("tray")} hint={t("tray_hint")}>
          <Switch label={t("tray")} checked={config.minimizeToTray} onCheckedChange={(value) => setConfig({ minimizeToTray: value })} />
        </Row>
        <Row label={t("notify")}>
          <Switch label={t("notify")} checked={config.notifications} onCheckedChange={(value) => setConfig({ notifications: value })} />
        </Row>
        <Row label={t("hotkey")} hint={t("hotkey_help")}>
          <Switch label={t("hotkey")} checked={config.hotkey} onCheckedChange={(value) => setConfig({ hotkey: value })} />
        </Row>
      </Section>
      <Section title={t("subs_extras")}>
        <Row label={t("subs")}>
          <Switch label={t("subs")} checked={config.subtitles} onCheckedChange={(value) => setConfig({ subtitles: value })} />
        </Row>
        <Row label={t("embed_subs")}>
          <Switch label={t("embed_subs")} checked={config.embedSubs} onCheckedChange={(value) => setConfig({ embedSubs: value })} />
        </Row>
        <Row label={t("sub_langs")}>
          <Input className="w-[180px]" value={config.subLangs} onChange={(event) => setConfig({ subLangs: event.target.value })} />
        </Row>
        <Row label={t("embed_thumb")}>
          <Switch label={t("embed_thumb")} checked={config.embedThumbnail} onCheckedChange={(value) => setConfig({ embedThumbnail: value })} />
        </Row>
        <Row label={t("embed_meta")}>
          <Switch label={t("embed_meta")} checked={config.embedMetadata} onCheckedChange={(value) => setConfig({ embedMetadata: value })} />
        </Row>
        <Row label={t("embed_chapters")}>
          <Switch label={t("embed_chapters")} checked={config.embedChapters} onCheckedChange={(value) => setConfig({ embedChapters: value })} />
        </Row>
      </Section>
      <Section title={t("sponsor")}>
        <Row label={t("sponsor")} hint={t("sponsor_help")}>
          <Switch label={t("sponsor")} checked={config.sponsorblock} onCheckedChange={(value) => setConfig({ sponsorblock: value })} />
        </Row>
        <div className="flex flex-wrap gap-2 py-4">
          {CATS.map((cat) => {
            const on = config.sponsorCategories.includes(cat.id)
            return (
              <button
                key={cat.id}
                type="button"
                onClick={() =>
                  setConfig({
                    sponsorCategories: on ? config.sponsorCategories.filter((item) => item !== cat.id) : [...config.sponsorCategories, cat.id],
                  })
                }
                className={cn(
                  "h-8 rounded-full border px-3 text-[13px] transition-colors",
                  on ? "border-fill bg-fill text-onfill" : "border-line text-muted hover:text-ink",
                )}
              >
                {t(cat.key)}
              </button>
            )
          })}
        </div>
      </Section>
      <Section title={t("cookies")}>
        <Row label={t("cookies")} hint={t("cookies_help")}>
          <div className="w-[180px]">
            <Select value={config.cookiesBrowser} onChange={(event) => setConfig({ cookiesBrowser: event.target.value as CookiesBrowser })}>
              {BROWSERS.map((browser) => (
                <option key={browser} value={browser}>
                  {browser === "none" ? t("none") : browser === "auto" ? t("auto") : browser}
                </option>
              ))}
            </Select>
          </div>
        </Row>
      </Section>
      <Section title={t("tools")}>
        <p className="py-3 text-[13px] leading-relaxed text-muted">{t("tools_hint")}</p>
        <p className="pb-3 text-[13px] text-muted">
          bob baixa · {app.tools.appVersion ?? "1.0.0"}
          <br />
          yt-dlp {app.tools.ytdlpVersion ? `· ${app.tools.ytdlpVersion}` : `· ${t("tools_missing")}`}
          <br />
          ffmpeg {app.tools.ffmpegVersion ? `· ${app.tools.ffmpegVersion}` : `· ${t("tools_missing")}`}
        </p>
        <Row label={t("auto_update")} hint={t("auto_update_hint")}>
          <Switch label={t("auto_update")} checked={config.autoUpdate} onCheckedChange={(value) => setConfig({ autoUpdate: value })} />
        </Row>
        <div className="border-b border-line py-4">
          <div className="text-[14px]">{t("update_url")}</div>
          <Input className="mt-3" value={config.updateUrl} placeholder="https://.../latest.json" onChange={(event) => setConfig({ updateUrl: event.target.value })} />
        </div>
        <div className="flex gap-2 py-4">
          <Button variant="outline" disabled={app.busy === "tools"} onClick={() => void checkUpdates()}>
            {t("check_updates")}
          </Button>
          <Button variant="ghost" disabled={app.busy === "tools"} onClick={() => void updateYtDlp()}>
            {t("update_ytdlp")}
          </Button>
          <Button variant="ghost" disabled={app.busy === "tools"} onClick={() => void reinstallTools()}>
            {t("reinstall_tools")}
          </Button>
        </div>
        {app.tools.message ? <p className="pb-3 text-[13px] text-danger">{app.tools.message}</p> : null}
      </Section>
      <Section title={t("about")}>
        <p className="py-3 text-[14px]">bob baixa · {t("version")}</p>
        <p className="max-w-[52ch] pb-2 text-[13px] leading-relaxed text-muted">{t("extension_hint")}</p>
        <p className="max-w-[52ch] pb-4 text-[13px] leading-relaxed text-muted">{t("protocol")}</p>
      </Section>
    </div>
  )
}

function Section({ title, first, children }: { title: string; first?: boolean; children: ReactNode }) {
  return (
    <section className={first ? "" : "mt-10"}>
      <h2 className="mb-1 text-[13px] text-muted">{title}</h2>
      {children}
    </section>
  )
}

function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-8 border-b border-line py-4">
      <div className="max-w-[300px]">
        <div className="text-[14px]">{label}</div>
        {hint ? <p className="mt-1 text-[13px] leading-relaxed text-muted">{hint}</p> : null}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  )
}
