import { useAiJobs } from "../lib/ai-activity"
import { useTranslationProgress } from "../lib/translations"
import { useT } from "../store"

export function AiActivity() {
  const jobs = useAiJobs()
  const progress = useTranslationProgress()
  const t = useT()
  const running = jobs.find((job) => job.phase === "running")
  if (progress > 0 && progress < 1) {
    return <p className="truncate rounded-full bg-soft px-3 py-1 text-[12px] text-muted">{t("ai_translating", { n: Math.round(progress * 100) })}</p>
  }
  if (!running) return null
  return <p className="truncate rounded-full bg-soft px-3 py-1 text-[12px] text-muted">{running.label}</p>
}
