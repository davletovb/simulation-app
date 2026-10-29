import type { ReactNode } from 'react'

export type IconName =
  | 'staff' | 'repairs' | 'access' | 'token' | 'hourglass' | 'board' | 'command' | 'campaign'
  | 'intel' | 'help' | 'lock' | 'alert' | 'spark' | 'check' | 'close' | 'minus' | 'plus'
  | 'eye' | 'eye-off' | 'reset' | 'people' | 'banner' | 'audit' | 'chair'

const PATHS: Record<IconName, ReactNode> = {
  staff: <><circle cx="12" cy="7.5" r="3.5" /><path d="M5 20c.8-3.9 3.6-6 7-6s6.2 2.1 7 6" /></>,
  repairs: <path d="M14.7 6.3a4 4 0 0 0-5.3 5.3L4 17l3 3 5.4-5.4a4 4 0 0 0 5.3-5.3l-2.5 2.5-2.6-.4-.4-2.6z" />,
  access: <><circle cx="6" cy="18" r="2" /><circle cx="18" cy="6" r="2" /><path d="M8 18h6.5a3.5 3.5 0 0 0 0-7h-5a3.5 3.5 0 0 1 0-7H16" /></>,
  token: <><path d="M12 3l7.8 4.5v9L12 21l-7.8-4.5v-9z" /><path d="M12 8.5l3 1.75v3.5L12 15.5l-3-1.75v-3.5z" /></>,
  hourglass: <><path d="M6 3h12M6 21h12" /><path d="M7 3v2a5 5 0 0 0 10 0V3M7 21v-2a5 5 0 0 1 10 0v2" /></>,
  board: <><path d="M3 9l9-5 9 5" /><path d="M5.5 9.5v8M10 9.5v8M14 9.5v8M18.5 9.5v8" /><path d="M3 20h18" /></>,
  command: <><rect x="4" y="6.5" width="11" height="14" rx="2" /><path d="M8.5 3.5H18a2 2 0 0 1 2 2V17" /></>,
  campaign: <><path d="M5 21V4" /><path d="M5 4.5h12l-2.5 4 2.5 4H5" /></>,
  intel: <><circle cx="10.5" cy="10.5" r="6" /><path d="M20 20l-5-5" /></>,
  help: <><circle cx="12" cy="12" r="9" /><path d="M9.6 9.4a2.5 2.5 0 1 1 3.4 2.4c-.6.3-1 .8-1 1.5v.6" /><path d="M12 17h.01" /></>,
  lock: <><rect x="5" y="11" width="14" height="9.5" rx="2" /><path d="M8 11V8a4 4 0 0 1 8 0v3" /></>,
  alert: <><path d="M12 4l9 16H3z" /><path d="M12 10v4M12 17h.01" /></>,
  spark: <path d="M12 3l1.9 5.6L19.5 10l-5.6 1.9L12 17.5l-1.9-5.6L4.5 10l5.6-1.4z" />,
  check: <path d="M5 12.5l4.5 4.5L19 7.5" />,
  close: <path d="M6 6l12 12M18 6L6 18" />,
  minus: <path d="M6 12h12" />,
  plus: <path d="M12 6v12M6 12h12" />,
  eye: <><path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z" /><circle cx="12" cy="12" r="3" /></>,
  'eye-off': <><path d="M3 3l18 18" /><path d="M10.6 5.6A9.7 9.7 0 0 1 12 5.5c6 0 9.5 6.5 9.5 6.5a17 17 0 0 1-3 3.8M6.3 6.9A16.6 16.6 0 0 0 2.5 12S6 18.5 12 18.5c1.6 0 3-.4 4.2-1" /></>,
  reset: <><path d="M4.5 12a7.5 7.5 0 1 0 2.2-5.3" /><path d="M4.5 4v4h4" /></>,
  people: <><circle cx="9" cy="8" r="3" /><path d="M3 20c.6-3.4 3-5.5 6-5.5s5.4 2.1 6 5.5" /><circle cx="17" cy="9" r="2.5" /><path d="M16.6 14.6c2.3.3 3.9 2.2 4.4 5.4" /></>,
  banner: <path d="M6 3h12v18l-6-4-6 4z" />,
  audit: <><rect x="5" y="3.5" width="14" height="17" rx="2" /><path d="M9 8h6M9 12h6M9 16h3" /></>,
  chair: <><path d="M7 3h10v9H7z" /><path d="M5 12h14M7 12v9M17 12v9" /></>,
}

export function Icon({ name, size = 20, label }: { name: IconName; size?: number; label?: string }) {
  return (
    <svg className="icon" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor"
      strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round"
      role={label ? 'img' : undefined} aria-label={label} aria-hidden={label ? undefined : true}>
      {PATHS[name]}
    </svg>
  )
}
