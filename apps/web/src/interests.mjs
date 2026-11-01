/**
 * Who a plan helps and who pays for it (decision 0014). The engine says what one delivered credit of each kind of work adds to
 * each group's support, net of what it costs that group's voters (`support.net_per_credit_ppm`, `[group][staffing, repairs,
 * access]`); this module only multiplies by the plan and puts the result in words. It is a rough picture of the whole plan once
 * delivered, before any news: work arrives over several quarters and each quarter's response is clipped by the engine.
 */
/** `urban_low` as "urban low-income": short enough to list without confusing the commas in the full names. */
export function shortGroup(id) {
  const [place, income] = id.split('_')
  return income ? `${place} ${income}-income` : id
}

/** A group moves by at least this much (half a point) before it counts as helped or as paying. */
export const MATERIAL_PPM = 5000

/**
 * What delivering the whole plan would add to each group's support, in ppm. A negative number is a group that pays more than it
 * gains.
 * @param {{ staff: number, repairs: number, access: number }} allocation
 * @param {number[][]} net `[group][staffing, repairs, access]`
 * @returns {number[]}
 */
export function planEffects(allocation, net) {
  return net.map(row => row[0] * allocation.staff + row[1] * allocation.repairs + row[2] * allocation.access)
}

/** "A", "A and B", "A, B and C". */
function listWords(words) {
  if (words.length <= 1) return words.join('')
  return `${words.slice(0, -1).join(', ')} and ${words.at(-1)}`
}

/**
 * The plan in one or two sentences: who it helps and who it costs. Groups that move by less than half a point are not named.
 * @param {number[]} effectsPpm
 * @param {readonly string[]} groupIds
 */
export function interestsText(effectsPpm, groupIds) {
  const helped = groupIds.filter((_, i) => effectsPpm[i] >= MATERIAL_PPM).map(shortGroup)
  const pays = groupIds.filter((_, i) => effectsPpm[i] <= -MATERIAL_PPM).map(shortGroup)
  if (helped.length === 0 && pays.length === 0) return 'No group moves by much.'
  if (pays.length === 0) return `It helps ${listWords(helped)} voters, and costs no group any support.`
  if (helped.length === 0) return `It costs ${listWords(pays)} voters support and helps no one.`
  return `It helps ${listWords(helped)} voters. It costs ${listWords(pays)} voters support.`
}
