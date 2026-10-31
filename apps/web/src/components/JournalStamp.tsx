/**
 * Where on the saved journal a figure was read. Replaces the long
 * "Verified local campaign…" sentence that every connected card repeated;
 * the synthetic-evidence caveat stays as one short note.
 */
export function JournalStamp({ campaign, cursor, version, note = true }: {
  campaign: string
  cursor: string
  version: string
  note?: boolean
}) {
  return (
    <div className="stamp">
      <ul className="chips" aria-label="Saved journal position">
        <li className="chip"><span className="stamp-key">Campaign</span>{campaign}</li>
        <li className="chip"><span className="stamp-key">Journal cursor</span>{cursor}</li>
        <li className="chip"><span className="stamp-key">State version</span>{version}</li>
      </ul>
      {note && <p className="hint">Synthetic game evidence, not an empirical forecast.</p>}
    </div>
  )
}
