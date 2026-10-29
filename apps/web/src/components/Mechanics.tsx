import type { ReactNode } from 'react'

/** Second visibility layer: the exact authored rule behind a public stance. */
export function Mechanics({ children }: { children: ReactNode }) {
  return (
    <details className="mechanics">
      <summary>View mechanics</summary>
      <div className="mechanics-body">
        <span className="mechanics-tag">Authored game rule</span>
        {children}
      </div>
    </details>
  )
}
