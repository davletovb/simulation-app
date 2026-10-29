import { useRef, type KeyboardEvent } from 'react'
import { Icon, type IconName } from './Icon'

export type View = 'command' | 'cabinet' | 'campaign' | 'intel'

export const VIEWS: readonly { id: View; label: string; icon: IconName; key: string }[] = [
  { id: 'command', label: 'Command', icon: 'command', key: '1' },
  { id: 'cabinet', label: 'Board', icon: 'board', key: '2' },
  { id: 'campaign', label: 'Campaign', icon: 'campaign', key: '3' },
  { id: 'intel', label: 'Intel', icon: 'intel', key: '4' },
]

export const isView = (value: string | null): value is View => VIEWS.some(v => v.id === value)

/** Side rail on wide screens, bottom tab bar on phones; one tablist either way. */
export function ViewNav({ view, onChange }: { view: View; onChange: (next: View) => void }) {
  const refs = useRef<(HTMLButtonElement | null)[]>([])

  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    const last = VIEWS.length - 1
    const next = { ArrowDown: index + 1, ArrowRight: index + 1, ArrowUp: index - 1, ArrowLeft: index - 1,
      Home: 0, End: last }[event.key]
    if (next === undefined) return
    event.preventDefault()
    const wrapped = (next + VIEWS.length) % VIEWS.length
    const target = VIEWS[wrapped]
    if (!target) return
    onChange(target.id)
    refs.current[wrapped]?.focus()
  }

  return (
    <nav className="rail" aria-label="Game views">
      <div role="tablist" className="rail-tabs">
        {VIEWS.map((v, i) => {
          const selected = v.id === view
          return (
            <button key={v.id} ref={el => { refs.current[i] = el }} type="button" role="tab" id={`tab-${v.id}`}
              aria-selected={selected} aria-controls="view-panel" tabIndex={selected ? 0 : -1}
              aria-keyshortcuts={v.key} className={`rail-tab${selected ? ' is-active' : ''}`}
              onClick={() => onChange(v.id)} onKeyDown={event => onKeyDown(event, i)}>
              <Icon name={v.icon} size={22} />
              <span className="rail-label">{v.label}</span>
              <kbd className="rail-key" aria-hidden="true">{v.key}</kbd>
            </button>
          )
        })}
      </div>
    </nav>
  )
}
