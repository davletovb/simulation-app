import type {Vote} from './views/ConnectedView'
export function positionsFor(votes:readonly Vote[],ministry:string):Vote[]
export function answeredMinistries(votes:readonly Vote[]):string[]
export function finalSupport(votes:readonly Vote[]):number
