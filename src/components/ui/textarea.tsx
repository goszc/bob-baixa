import { forwardRef, type TextareaHTMLAttributes } from "react"
import { cn } from "../../lib/cn"

export const Textarea = forwardRef<HTMLTextAreaElement, TextareaHTMLAttributes<HTMLTextAreaElement>>(
  function Textarea({ className, ...props }, ref) {
    return (
      <textarea
        ref={ref}
        className={cn(
          "w-full resize-none bg-transparent text-[15px] leading-relaxed outline-none placeholder:text-muted",
          className,
        )}
        {...props}
      />
    )
  },
)
