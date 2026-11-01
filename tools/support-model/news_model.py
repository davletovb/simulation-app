#!/usr/bin/env python3
"""An independent replica of the news calendar (decisions 0010, 0011, 0012, 0013 and 0014), to check the engine against.

Source of truth: crates/domain/src/news.rs and `Campaign::advance` in crates/domain/src/reducer.rs. This file repeats the
rules from their description, reads the headlines from `data/england-school-pilot-v1/authored.json` (so the deck cannot drift
from the data), and takes the support model from `support_model.py`. If the engine and this file ever disagree, one is wrong:
that is the point.

    python3 tools/support-model/news_model.py            # print the first election of a few campaigns
    python3 tools/support-model/news_model.py --check    # the numbers the engine's tests pin
"""
import hashlib, json, pathlib, sys

HERE = pathlib.Path(__file__).parent
sys.path.insert(0, str(HERE))
import support_model as sm

AUTHORED = json.loads((HERE / '..' / '..' / 'data' / 'england-school-pilot-v1' / 'authored.json').read_text())
NEWS = AUTHORED['news_calendar']
P = sm.CALIBRATIONS['v2']
MASK = (1 << 64) - 1
SHOCK_CAP = 40_000
HELD_ABOVE = AUTHORED['election']['bloc_held_above_ppm']
BAR = AUTHORED['election']['min_weighted_support_ppm']


def _electorate():
    """The cells and the named groups of the authored electorate (decision 0015), from its attributes and selectors alone:
    for every group id (each cell is one too) the positions of the cells it selects. Cells are every combination of levels,
    the first attribute varying slowest."""
    e = AUTHORED['electorate']
    levels = [[lv['id'] for lv in a['levels']] for a in e['attributes']]
    names = [a['id'] for a in e['attributes']]
    cells = [[]]
    for options in levels:
        cells = [c + [o] for c in cells for o in options]

    def holds(sel, cell):
        if sel == 'everyone':
            return True
        (key, inner), = sel.items()
        if key == 'is':
            return cells[cell][names.index(inner['attribute'])] == inner['level']
        if key == 'all':
            return all(holds(x, cell) for x in inner)
        if key == 'any':
            return any(holds(x, cell) for x in inner)
        assert key == 'not'
        return not holds(inner, cell)
    groups = {'_'.join(c): {i} for i, c in enumerate(cells)}
    for g in e['groups']:
        groups[g['id']] = {i for i in range(len(cells)) if holds(g['select'], i)}
    return len(cells), groups


CELLS, GROUPS = _electorate()


def spread(entries, cap):
    """What named targets come to for each cell: the entries that reach a cell add up, and no cell goes past `cap`."""
    out = [0] * CELLS
    for group, amount in entries.items():
        for cell in GROUPS[group]:
            out[cell] += amount
    return [min(x, cap) for x in out]


def news_seed(campaign_id):
    """The first eight bytes of a digest of the campaign's identity, big-endian (crates/hiyal-adapter/src/lib.rs)."""
    return int.from_bytes(hashlib.sha256(f'myth-maker-news-calendar:{campaign_id}'.encode()).digest()[:8], 'big')


class SplitMix:
    def __init__(self, state):
        self.state = state & MASK

    def next(self):
        self.state = (self.state + 0x9E3779B97F4A7C15) & MASK
        z = self.state
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
        return z ^ (z >> 31)


def schedule(seed, term):
    """[(quarter, tone, index, surprise)] for one term, in quarter order."""
    count = NEWS['quiet_term_headlines'] if term == 1 else NEWS['headlines_per_term']
    rng = SplitMix((seed + term * 0x9E3779B97F4A7C15) & MASK)
    slots = [2, 3, 4, 5, 6]
    for i in range(count):
        j = i + rng.next() % (len(slots) - i)
        slots[i], slots[j] = slots[j], slots[i]
    used = {'bad': set(), 'good': set()}
    out = []
    for in_term in sorted(slots[:count]):
        tone_roll = rng.next() % 100
        index_roll = rng.next()
        surprise_roll = rng.next() % NEWS['surprise_one_in']
        tone = 'good' if term == 1 or tone_roll < NEWS['good_percent'] else 'bad'
        deck = NEWS[tone]
        index = index_roll % len(deck)
        while index in used[tone]:
            index = (index + 1) % len(deck)
        used[tone].add(index)
        out.append(((term - 1) * 6 + in_term, tone, index, surprise_roll == 0))
    return out


GOLDEN = 0x9E3779B97F4A7C15
MIX_SALT = 0xD1B54A32D192ED03


def trade_offs(seed, slots):
    """Some bad quarters carry a trade-off story instead (decision 0014). Each bad quarter is decided on its own by a draw from
    the seed and the quarter alone: the state is (seed + quarter x golden ratio) xor a fixed salt, one SplitMix output is drawn,
    and one time in `mixed_one_in` the quarter takes the story at (output >> 16) mod the deck size, or the next unused one on.
    Returns the slots as [quarter, tone, index, surprise, mixed]."""
    deck, used = NEWS['mixed'], set()
    out = []
    for q, tone, index, surprise in slots:
        mixed = False
        if tone == 'bad':
            roll = SplitMix(((seed + q * GOLDEN) & MASK) ^ MIX_SALT).next()
            if roll % NEWS['mixed_one_in'] == 0:
                first = (roll >> 16) % len(deck)
                free = [(first + k) % len(deck) for k in range(len(deck)) if (first + k) % len(deck) not in used]
                if free:
                    index, mixed = free[0], True
                    used.add(index)
        out.append([q, tone, index, surprise, mixed])
    return out


def schedule_real(seed, term, real=()):
    """The calendar of one term with trade-off stories (decision 0014) and real headlines imported for it (decision 0013).
    `real` is a list of (tone, effect[, winners]) in import order, only those a rule matched. The invented calendar is drawn
    exactly as without either, trade-off stories replace some bad quarters, then each real headline takes the first quarter of its
    tone that no real headline has taken, announced a quarter ahead (never a surprise); at most two a term.
    Returns (quarter, tone, index, surprise, taken, mixed) where `taken` is the (item, tone, effect, winners) of the real
    headline in that quarter, if any."""
    base = schedule(seed, term)
    if term >= NEWS['mixed_from_term']:
        base = [s[:4] for s in trade_offs(seed, base)]
        flags = [s[4] for s in trade_offs(seed, schedule(seed, term))]
    else:
        flags = [False] * len(base)
    out = [[q, tone, index, surprise, None, mixed] for (q, tone, index, surprise), mixed in zip(base, flags)]
    for item, r in list(enumerate(real))[:2]:
        tone, effect = r[0], r[1]
        winners = r[2] if len(r) > 2 else [0] * 6
        for slot in out:
            if slot[4] is None and slot[1] == tone:
                slot[4], slot[3] = (item, tone, effect, winners), False
                break
    return [tuple(s) for s in out]


FAMILIES = ('staffing', 'repairs', 'access')
CAP = {'staffing': 12, 'repairs': 10, 'access': 4}
LAG = {'staffing': 1, 'repairs': 2, 'access': 0}
BALANCED = {'staffing': 50, 'repairs': 40, 'access': 10}


def capacity_kept(seed, quarter, answers=(), real=None):
    """Percent of its usual capacity each kind of work keeps in `quarter`, given the disruptions in the news (decision 0011):
    a disruption lasts `quarters` from the quarter it lands in, may carry into the next term, an answered one is cut by 60% less,
    and where several meet the worst rules."""
    keep = {f: 100 for f in FAMILIES}
    term = (quarter - 1) // 6 + 1
    for t in (max(term - 1, 0), term):
        if t < 1:
            continue
        for q, tone, index, _surprise, taken, mixed in schedule_real(seed, t, (real or {}).get(t, ())):
            d = NEWS[tone][index].get('disrupts') if tone == 'bad' and taken is None and not mixed else None
            if not d or not (q <= quarter < q + d['quarters']):
                continue
            kept = d['keep_pct']
            if q in answers:
                kept = 100 - (100 - kept) * (100 - NEWS['relief']['cut_pct']) // 100
            keep[d['family']] = min(keep[d['family']], kept)
    return keep


def deliveries(campaign_id, plans, answers=(), quarters=None, owed=None, real=None):
    """Cumulative [staffing, repairs, access] after each quarter when `plans[t]` (a dict, or None to keep the programme) is
    approved in the first quarter of term t+1, delivered at full effort under the engine's lags and capacities, with the
    disruptions of this campaign's calendar. Work not delivered by the end of a term is dropped (a new plan replaces it).
    If `owed` is a list, the promised-and-undelivered units after each quarter are appended to it, as dicts."""
    seed = news_seed(campaign_id)
    quarters = quarters or 6 * len(plans)
    total, out, left = {f: 0 for f in FAMILIES}, [], {f: 0 for f in FAMILIES}
    for q in range(1, quarters + 1):
        term, qt = (q - 1) // 6 + 1, (q - 1) % 6 + 1
        if qt == 1 and plans[term - 1] is not None:
            left = dict(plans[term - 1])
        keep = capacity_kept(seed, q, answers, real)
        for f in FAMILIES:
            if qt <= LAG[f]:
                continue
            cap = CAP[f] * keep[f] // 100
            if f == 'access' and keep[f] == 100 and qt >= 2 and plans[term - 1] is not None and plans[term - 1]['access'] >= 10 and left[f] > 0:
                cap += 1   # the volunteer bonus, from the second quarter, while access work remains
            d = min(left[f], cap)
            left[f] -= d
            total[f] += d
        out.append([total[f] for f in FAMILIES])
        if owed is not None:
            owed.append(dict(left))
    return out


def severity(term):
    s, n = NEWS['severity'], max(0, term - 2)
    return (s['base_ppm'] * min(s['base_cap_pct'], 100 + s['base_step_pct'] * n) // 100,
            min(s['share_cap_pct'], s['share_pct'] + s['share_step_pct'] * n))


def landing_shock(tone, index, term, support):
    if tone == 'good':
        return spread(NEWS['good'][index]['gain_ppm'], SHOCK_CAP)
    base, share = severity(term)
    out = []
    for exposure, now in zip(spread(NEWS['bad'][index]['exposure_pct'], 100), support):
        lead = max(0, now - HELD_ABOVE)
        out.append(-min(SHOCK_CAP, exposure * (base + lead * share // 100) // 100))
    return out


def shock_from(tone, effect, term, support, winners=None):
    """What a headline does given only its numbers: a real headline, or any other once its deck entry is looked up. Bad news
    may also give some groups a flat gain (`winners`, in ppm; decision 0014)."""
    if tone == 'good':
        return [min(g, SHOCK_CAP) for g in effect]
    base, share = severity(term)
    loss = [-min(SHOCK_CAP, exposure * (base + max(0, now - HELD_ABOVE) * share // 100) // 100)
            for exposure, now in zip(effect, support)]
    return [x + min(w, SHOCK_CAP) for x, w in zip(loss, winners or [0] * 6)]


def soften(shock):
    return [sm.trunc_div(x * (100 - NEWS['relief']['cut_pct']), 100) if x < 0 else x for x in shock]


def overall_ppm(shock):
    """The groups' effects weighted by their share of the voters, truncated toward zero as the engine does."""
    return sm.trunc_div(sum(s * w for s, w in zip(shock, sm.W)), 1_000_000)


def relief_cost(shock):
    lost = sum(abs(s) * w for s, w in zip(shock, sm.W) if s < 0) // 1_000_000
    return max(1, -(-lost * NEWS['relief']['credits_per_point'] // 10_000))


def scrutiny_pct(term):
    s, n = NEWS['scrutiny'], max(0, term - 2)
    return min(s['cap_pct'], s['pct'] + s['step_pct'] * n)


def scrutiny_shock(owed, term):
    """What inspectors take from each group: a share of what the promised-and-undelivered work would have earned it (never
    more than the shock cap), and nothing from a group that work would have cost more than it gave. `owed` is
    [staffing, repairs, access] units."""
    pct = scrutiny_pct(term)
    return [-min(SHOCK_CAP, max(0, sm.response(ix, owed, **P)) * pct // 100) for ix in range(6)]


def play(cums, campaign_id, quarters, answers=(), owed=None, real=None):
    """Support after `quarters` quarters of the calendar and the given cumulative deliveries, as the engine does it: each
    quarter's headline lands first, then queued responses, then this quarter's delivery is queued. `answers` are the
    landing quarters whose headline was paid to soften. `owed` is what `deliveries` reports is still promised after each
    quarter (an inspection counts it). Returns (support, settled for election night, landed), where each landed entry ends
    with the credits a windfall brought (zero otherwise). `real` maps a term to the (tone, effect) of the real headlines imported
    for it that a rule matched (decision 0013); a landed real headline is reported with the id `real`."""
    seed, sup, queue, prev, landed = news_seed(campaign_id), [500_000] * 6, {}, [0, 0, 0], []
    book = {}
    for term in range(1, (quarters - 1) // 6 + 2):
        for q, tone, index, surprise, taken, mixed in schedule_real(seed, term, (real or {}).get(term, ())):
            book[q] = (tone, index, surprise, taken, mixed)
    for q in range(1, quarters + 1):
        cum = cums[q - 1] if q - 1 < len(cums) else cums[-1]
        d = [cum[k] - prev[k] for k in range(3)]
        prev = list(cum)
        if q in book:
            tone, index, surprise, taken, mixed = book[q]
            term = (q - 1) // 6 + 1
            if taken is not None:
                shock = shock_from(taken[1], taken[2], term, sup, taken[3])
                headline = {'id': 'real'}
            elif mixed:
                headline = NEWS['mixed'][index]
                shock = shock_from('bad', spread(headline['exposure_pct'], 100), term, sup, spread(headline['winners_ppm'], SHOCK_CAP))
            else:
                shock = landing_shock(tone, index, term, sup)
                headline = NEWS[tone][index]
            if tone == 'bad' and headline.get('scrutiny'):
                counted = headline['scrutiny']['families']
                units = [owed[q - 1][f] if f in counted else 0 for f in FAMILIES]
                shock = [max(-SHOCK_CAP, a + b) for a, b in zip(shock, scrutiny_shock(units, term))]
            if tone == 'bad' and q in answers:
                shock = soften(shock)
            sup = [max(0, min(10**6, s + v)) for s, v in zip(sup, shock)]
            landed.append((q, tone, headline['id'], surprise, shock, headline.get('grant_credits', 0)))
        for ed in queue.pop(q, []):
            sup = [max(0, min(10**6, s + sm.response(ix, ed, **P))) for ix, s in enumerate(sup)]
        if sum(d) > 0:
            queue.setdefault(q + 1, []).append(d)
    settled = list(sup)
    for ed in queue.get(quarters + 1, []):
        settled = [max(0, min(10**6, s + sm.response(ix, ed, **P))) for ix, s in enumerate(settled)]
    return sup, settled, landed


def first_election(campaign_id):
    """Balanced approved in quarter 1, then the programme kept for the other five (what the engine tests play)."""
    cums = []
    for q in range(1, 7):
        cums.append([sum([0, 12, 12, 12, 12, 2][:q]), sum([0, 0, 10, 10, 10, 10][:q]), sum([4, 5, 1, 0, 0, 0][:q])])
    return play(cums, campaign_id, 6)


def report():
    for cid in ('term', 'a', 'b'):
        sup, settled, landed = first_election(cid)
        held = sum(sm.W[i] for i in range(6) if settled[i] > HELD_ABOVE)
        print(f'{cid:6} seed {news_seed(cid):20} settled {settled} overall {sum(w * s for w, s in zip(sm.W, settled)) // 10**6} held {held}')
        for q, tone, hid, surprise, shock, credits in landed:
            print(f'         Q{q} {tone:4} {hid:22} {"surprise" if surprise else "announced":9} {shock}' + (f' +{credits} credits' if credits else ''))


def check():
    """The numbers the engine's own tests pin (crates/domain/src/news.rs and the election-night test in
    crates/local-api/src/connected.rs). Both sides must say the same, or one of them is wrong."""
    ok = True

    def expect(name, got, want):
        nonlocal ok
        good = got == want
        ok &= good
        print(f'{"ok  " if good else "FAIL"} {name}' + ('' if good else f'\n       got  {got}\n       want {want}'))

    expect('schedule, seed 12345, term 1', schedule(12345, 1), [(2, 'good', 3, False), (4, 'good', 2, False)])
    expect('schedule, seed 12345, term 2', schedule(12345, 2),
           [(8, 'bad', 8, False), (9, 'bad', 2, True), (10, 'bad', 9, False), (11, 'bad', 7, False)])
    expect('schedule, seed 12345, term 3', schedule(12345, 3),
           [(14, 'bad', 13, False), (16, 'bad', 3, False), (17, 'bad', 10, True), (18, 'bad', 5, True)])
    expect('severity, terms 2, 3, 10', [severity(2), severity(3), severity(10)], [(5000, 40), (7500, 48), (20000, 90)])
    expect('bad news taxes the lead', landing_shock('bad', 3, 2, [500000, 500000, 500000, 560000, 520000, 500000]),
           [0, 0, 0, -29000, -13000, -5000])
    expect('a response softens by 60%', soften([-10000, -20000, 0, -30000, 0, 5000]), [-4000, -8000, 0, -12000, 0, 5000])
    expect('a response costs ceil(10 credits a point)', relief_cost([-10000, -20000, 0, -30000, 0, 5000]), 12)
    # Campaign "dz4827" (crates/local-api/src/connected.rs, the election-night test): Balanced in term 1, then 50/35/0 in term 2,
    # with a flood announced for quarter 11 that stops Repairs for two quarters, answered in quarter 10.
    cid, plan2 = 'dz4827', {'staffing': 50, 'repairs': 35, 'access': 0}
    sup, settled, landed = play(deliveries(cid, [BALANCED], quarters=6), cid, 6)
    expect('dz4827: support before election night 1', sup, [590120, 601600, 472280, 616536, 633080, 508824])
    expect('dz4827: election night 1', settled, [591800, 612000, 472200, 624240, 650600, 516960])
    expect('dz4827: the headlines of term 1', [(q, h) for q, _, h, _, _, _ in landed], [(3, 'volunteer_surge'), (5, 'attendance_rises')])
    funding = next(i for i, h in enumerate(NEWS['bad']) if h['id'] == 'funding_row')
    expect('dz4827: what answering the funding row would cost', relief_cost(landing_shock('bad', funding, 2, sup)), 18)
    sup9, _, _ = play(deliveries(cid, [BALANCED, plan2], quarters=9), cid, 9)
    expect('dz4827: support at the start of quarter 10', sup9, [605820, 611500, 464420, 622556, 634700, 499644])
    flood = next(i for i, h in enumerate(NEWS['bad']) if h['id'] == 'flooding_halts_repairs')
    mood = landing_shock('bad', flood, 2, sup9)
    # 25 repair units still to deliver; stopped in quarters 11 and 12 only quarter 10's ten arrive: 15 slip past the election.
    delivery_effect = [-sm.response(ix, [0, 15, 0], **P) for ix in range(6)]
    total = [a + b for a, b in zip(mood, delivery_effect)]
    expect('dz4827: what the delayed work would have earned, taken away', delivery_effect, [1200, -12000, -1200, -10140, -24600, -15060])
    expect('dz4827: the quote for the flood', total, [-3532, -16960, -1700, -26346, -42264, -16560])
    expect('dz4827: ...overall, truncated toward zero', overall_ppm(total), -12772)
    expect('dz4827: ...and what answering costs', relief_cost(total), 13)
    expect('an answer turns a stoppage into a 60% slowdown', capacity_kept(news_seed(cid), 11, answers=(11,))['repairs'], 60)
    answered_quote = [a + b for a, b in zip(soften(mood), [-sm.response(ix, [0, 3, 0], **P) for ix in range(6)])]
    expect('dz4827: ...once answered only 3 units slip past, and the loss is softened', answered_quote, [-1652, -4384, -440, -8510, -11985, -3612])
    cums = deliveries(cid, [BALANCED, plan2], answers=(11,), quarters=12)
    expect('dz4827: units delivered by quarter 12 with the answer', cums[-1], [100, 72, 10])
    _, settled12, landed12 = play(cums, cid, 12, answers=(11,))
    expect('dz4827: election night 2', settled12, [648262, 680358, 450060, 616966, 662205, 489996])
    expect('dz4827: the flood, answered', [shock for q, _, _, _, shock, _ in landed12 if q == 11][0], [-2118, -2342, -200, -5158, -6255, -600])
    # Campaign "sc156" (the inspection test): Balanced, then 50/35/0; an education grant lands in quarter 4 of term 1; in term 2
    # an inspection of building promises (quarter 10) is left alone, a spot check on every promise (quarter 11) is answered,
    # and a charity donation arrives in quarter 12 (decision 0012).
    cid = 'sc156'
    owed = []
    cums = deliveries(cid, [BALANCED, plan2], quarters=12, owed=owed)
    sup6, settled1, landed1 = play(cums[:6], cid, 6, owed=owed)
    expect('sc156: election night 1', settled1, [586800, 617000, 487200, 594240, 630600, 506960])
    expect('sc156: the grant', [(q, h, c) for q, _, h, _, _, c in landed1], [(3, 'inspectorate_praise', 0), (4, 'education_grant', 15)])
    audit = next(i for i, h in enumerate(NEWS['bad']) if h['id'] == 'building_promises_audit')
    sup8, _, _ = play(cums[:8], cid, 8, owed=owed)
    units = [owed[9][f] if f in NEWS['bad'][audit]['scrutiny']['families'] else 0 for f in FAMILIES]
    expect('sc156: promised repairs still unkept when the inspectors arrive', units, [0, 15, 0])
    expect('sc156: the count alone', scrutiny_shock(units, 2), [0, -6000, -600, -5070, -12300, -7530])
    quote = [max(-SHOCK_CAP, a + b) for a, b in zip(landing_shock('bad', audit, 2, sup8), scrutiny_shock(units, 2))]
    expect('sc156: the quote for the audit, a quarter ahead', quote, [-5560, -14702, -1600, -14036, -26724, -9865])
    expect('sc156: ...overall', overall_ppm(quote), -10115)
    expect('sc156: ...and what answering costs', relief_cost(quote), 11)
    spot = next(i for i, h in enumerate(NEWS['bad']) if h['id'] == 'spot_check_on_every_promise')
    sup9, _, _ = play(cums[:9], cid, 9, owed=owed)
    units = [owed[10][f] for f in FAMILIES]
    expect('sc156: promised work still unkept at the spot check', units, [2, 5, 0])
    quote = [max(-SHOCK_CAP, a + b) for a, b in zip(landing_shock('bad', spot, 2, sup9), scrutiny_shock(units, 2))]
    expect('sc156: the quote for the spot check', quote, [-9479, -15518, -1250, -10200, -17352, -2808])
    expect('sc156: ...and what answering costs', relief_cost(quote), 10)
    _, settled12, landed12 = play(cums, cid, 12, answers=(11,), owed=owed)
    shocks = {q: shock for q, _, _, _, shock, _ in landed12}
    expect('sc156: the audit, as it landed', shocks[10], [-6751, -15854, -1600, -14715, -27530, -9030])
    expect('sc156: the spot check, answered', shocks[11], [-4084, -6468, -500, -3988, -6764, -1123])
    expect('sc156: election night 2', settled12, [605375, 661958, 465900, 590775, 658810, 484347])
    expect('sc156: the donation', [c for q, _, _, _, _, c in landed12 if q == 12], [10])
    # Campaign "rn112" (crates/local-api/src/connected.rs, the real-news test): the test feed (hand-written, not a real capture) gives
    # term 1 a real teacher pay award (good, takes the first good quarter, 2) and a RAAC story (bad, no bad quarter in term 1, so it
    # changes nothing), and term 2 a real strike ballot (bad, takes the first bad quarter, 10). Decision 0013.
    cid = 'rn112'
    pay, raac = ('good', spread({'everyone': 12000}, SHOCK_CAP)), ('bad', spread({'urban': 25, 'rural': 75}, 100))
    strike = ('bad', spread({'urban': 50, 'rural': 25}, 100))
    real = {1: [pay, raac], 2: [strike]}
    seed = news_seed(cid)
    expect('rn112: the real pay award takes quarter 2 and the bad one finds no room in term 1',
           [(q, taken is not None) for q, _, _, _, taken, _ in schedule_real(seed, 1, real[1])], [(2, True), (5, False)])
    expect('rn112: the real strike ballot takes the first bad quarter of term 2',
           [q for q, tone, _, _, taken, _ in schedule_real(seed, 2, real[2]) if taken is not None], [10])
    owed = []
    cums = deliveries(cid, [BALANCED, plan2], quarters=12, owed=owed, real=real)
    sup, settled, landed = play(cums[:6], cid, 6, owed=owed, real=real)
    expect('rn112: support before election night 1', sup, [602120, 613600, 484280, 603536, 620080, 495824])
    expect('rn112: election night 1', settled, [603800, 624000, 484200, 611240, 637600, 503960])
    expect('rn112: the real pay award is a flat gift', [sh for q, _, h, _, sh, _ in landed if h == 'real'][0], [12000] * 6)
    sup8, _, _ = play(cums[:8], cid, 8, owed=owed, real=real)
    quote = shock_from('bad', strike[1], 2, sup8)
    expect('rn112: the quote for the real strike ballot', quote, [-27260, -29300, -2500, -14374, -16010, -1646])
    expect('rn112: ...overall, and what answering would cost', (overall_ppm(quote), relief_cost(quote)), (-19382, 20))
    sup9, _, _ = play(cums[:9], cid, 9, owed=owed, real=real)
    expect('rn112: a quarter later', shock_from('bad', strike[1], 2, sup9), [-33236, -35180, -2500, -16440, -18182, -2003])
    _, settled12, landed12 = play(cums, cid, 12, owed=owed, real=real)
    expect('rn112: election night 2', settled12, [664764, 701820, 477500, 637060, 689818, 497838])
    expect('rn112: the strike ballot as it landed', [sh for q, _, _, _, sh, _ in landed12 if q == 10][0], [-33236, -35180, -2500, -16440, -18182, -2003])
    # Campaign "tf2966" (crates/local-api/src/connected.rs, the trade-off test): Balanced every term. Terms 1 and 2 have no trade-off
    # story (they begin in term 3); in term 3 quarter 14 carries "small village schools to be merged", which hurts the rural groups and
    # helps urban middle and high income (decision 0014).
    cid = 'tf2966'
    seed = news_seed(cid)
    owed = []
    cums = deliveries(cid, [BALANCED, BALANCED, BALANCED], quarters=18, owed=owed)
    expect('tf2966: no trade-off story in terms 1 and 2', [m for t in (1, 2) for *_, m in schedule_real(seed, t, ())], [False] * 6)
    expect('tf2966: term 3 has two, in quarters 14 and 18',
           [(q, NEWS['mixed'][i]['id']) for q, _, i, _, _, m in schedule_real(seed, 3, ()) if m],
           [(14, 'small_schools_merged'), (18, 'funding_formula_shift')])
    _, s1, _ = play(cums[:6], cid, 6, owed=owed)
    _, s2, _ = play(cums[:12], cid, 12, owed=owed)
    expect('tf2966: election nights 1 and 2', [sum(w * x for w, x in zip(sm.W, s)) // 10**6 for s in (s1, s2)], [576584, 623698])
    sup13, _, _ = play(cums[:13], cid, 13, owed=owed)
    story = NEWS['mixed'][3]
    quote = shock_from('bad', spread(story['exposure_pct'], 100), 3, sup13, spread(story['winners_ppm'], SHOCK_CAP))
    expect('tf2966: the trade-off quote', quote, [0, 8000, 12000, -40000, -40000, -9979])
    expect('tf2966: ...overall, and what answering would cost (only losses are priced)', (overall_ppm(quote), relief_cost(quote)), (-1298, 7))
    expect('tf2966: ...answered, the losses shrink by 60% and the gains stay',
           soften(quote), [0, 8000, 12000, -16000, -16000, -3991])
    sup14, _, landed14 = play(cums[:14], cid, 14, owed=owed)
    expect('tf2966: support at the start of quarter 15', sup14, [645824, 707620, 474300, 637492, 701276, 526135])
    expect('scrutiny takes a share of what the work would have earned', scrutiny_shock([26, 30, 0], 2), [-14920, -20000, 0, -16276, -20000, -2684])
    expect('...growing with the term', [scrutiny_pct(t) for t in range(1, 11)], [50, 50, 65, 80, 95, 110, 125, 140, 155, 170])
    return ok


if __name__ == '__main__':
    if '--check' in sys.argv:
        sys.exit(0 if check() else 1)
    report()
