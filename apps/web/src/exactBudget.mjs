/** Exact ASCII integer credit amounts. Never use parseFloat, scientific
 * notation or implicit Number coercion for player money. Server authority
 * must revalidate every value in the typed command. */
export function parseExactBudget(text,max) {
  if (typeof text !== 'string' || !/^(0|[1-9][0-9]{0,2})$/.test(text)) return null
  if (!Number.isSafeInteger(max) || max < 0) return null
  const credits=Number(text)
  return Number.isSafeInteger(credits) && credits<=max ? credits : null
}
