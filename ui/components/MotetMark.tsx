import { useId } from "react";

export default function MotetMark({ className = "duet" }: { className?: string }) {
  const mask = useId();
  return <svg className={className} viewBox="0 0 52 52" aria-hidden="true"><mask id={mask} maskUnits="userSpaceOnUse" x="0" y="0" width="52" height="52"><rect width="52" height="52" fill="#fff" stroke="none"/><path d="M14 10v12c0 12 24 6 24 20" fill="none" stroke="#000" strokeWidth="13.5"/></mask><path d="M38 10v12c0 12-24 6-24 20" mask={`url(#${mask})`} fill="none" stroke="var(--voice-them)" strokeWidth="5.2" strokeLinecap="round"/><path d="M14 10v12c0 12 24 6 24 20" fill="none" stroke="var(--voice-me)" strokeWidth="5.2" strokeLinecap="round"/><circle cx="14" cy="10" r="5.4" fill="var(--voice-me)" stroke="none"/><circle cx="14" cy="42" r="5.4" fill="var(--voice-them)" stroke="none"/></svg>;
}
