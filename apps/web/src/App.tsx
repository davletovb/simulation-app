import { useCallback, useEffect, useState } from 'react'
import { OFFLINE_PILOT } from './contracts'
import { COUNTRY_NAME, DEFAULT_CARD, type Allocation } from './game'
import { readStored, writeStored } from './storage'
import { Briefing } from './components/Briefing'
import { Hud } from './components/Hud'
import { Icon } from './components/Icon'
import { VIEWS, ViewNav, isView, type View } from './components/ViewNav'
import { CabinetView } from './views/CabinetView'
import { ConnectedView } from './views/ConnectedView'
import { CampaignView } from './views/CampaignView'
import { CommandView } from './views/CommandView'
import { IntelView } from './views/IntelView'

const VIEW_KEY = 'myth-maker.view'
const BRIEFED_KEY = 'myth-maker.briefed'

const typingIn = (target: EventTarget | null): boolean =>
  target instanceof HTMLElement && (target.isContentEditable || target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement || (target instanceof HTMLInputElement && target.type !== 'range'))

export function App() {
  const [view, setView] = useState<View>(() => {
    const stored = readStored(VIEW_KEY)
    return isView(stored) ? stored : 'command'
  })
  const [allocation, setAllocation] = useState<Allocation>(DEFAULT_CARD.allocation)
  const [briefingOpen, setBriefingOpen] = useState(() => readStored(BRIEFED_KEY) !== 'yes')

  const changeView = useCallback((next: View) => {
    setView(next)
    writeStored(VIEW_KEY, next)
    window.scrollTo({ top: 0 })
  }, [])

  const closeBriefing = () => {
    setBriefingOpen(false)
    writeStored(BRIEFED_KEY, 'yes')
  }

  useEffect(() => {
    document.documentElement.lang = OFFLINE_PILOT.locale
  }, [])

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey) return
      if (typingIn(event.target) || document.querySelector('dialog[open], [role="dialog"]')) return
      if (event.key === '?') {
        event.preventDefault()
        setBriefingOpen(true)
        return
      }
      const target = VIEWS.find(v => v.key === event.key)
      if (target) {
        event.preventDefault()
        changeView(target.id)
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [changeView])

  return (
    <div className="game">
      <a className="skip-link" href="#view-panel">Skip to the game view</a>
      <header className="top">
        <Hud allocation={allocation} onHelp={() => setBriefingOpen(true)} />
        <p className="notice" role="note">
          <Icon name="eye" size={16} />
          <span><strong>Fictional data twin.</strong> Illustrative mechanics, not government forecasts. The connected decision room runs a local, durable scripted simulation. Other panels are design previews; no model calls are made.</span>
        </p>
      </header>

      <div className="stage">
        <ViewNav view={view} onChange={changeView} />
        <main className="view">
          <div id="view-panel" role="tabpanel" aria-labelledby={`tab-${view}`} tabIndex={-1} className="view-inner">
            {view === 'command' && <>
              <ConnectedView allocation={allocation} />
              <CommandView allocation={allocation} onChange={setAllocation} />
            </>}
            {view === 'cabinet' && <CabinetView />}
            {view === 'campaign' && <CampaignView />}
            {view === 'intel' && <IntelView />}
          </div>
          <footer className="foot">
            <span>Myth Maker · framework v0.8.1 · P7 connected offline candidate · P6 review pending</span>
            <span>{OFFLINE_PILOT.jurisdiction} is the education jurisdiction, not the whole {COUNTRY_NAME} · {OFFLINE_PILOT.locale}</span>
          </footer>
        </main>
      </div>

      <Briefing open={briefingOpen} onClose={closeBriefing} />
    </div>
  )
}
