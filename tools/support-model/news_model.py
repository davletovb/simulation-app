#!/usr/bin/env python3
"""An independent replica of the news calendar (decision 0010), to check the engine against.

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


FAMILIES = ('staffing', 'repairs', 'access')
CAP = {'staffing': 12, 'repairs': 10, 'access': 4}
LAG = {'staffing': 1, 'repairs': 2, 'access': 0}
BALANCED = {'staffing': 50, 'repairs': 40, 'access': 10}


def capacity_kept(seed, quarter, answers=()):
    """Percent of its usual capacity each kind of work keeps in `quarter`, given the disruptions in the news (decision 0011):
    a disruption lasts `quarters` from the quarter it lands in, may carry into the next term, an answered one is cut by 60% less,
    and where several meet the worst rules."""
    keep = {f: 100 for f in FAMILIES}
    term = (quarter - 1) // 6 + 1
    for t in (max(term - 1, 0), term):
        if t < 1:
            continue
        for q, tone, index, _surprise in schedule(seed, t):
            d = NEWS[tone][index].get('disrupts') if tone == 'bad' else None
            if not d or not (q <= quarter < q + d['quarters']):
                continue
            kept = d['keep_pct']
            if q in answers:
                kept = 100 - (100 - kept) * (100 - NEWS['relief']['cut_pct']) // 100
            keep[d['family']] = min(keep[d['family']], kept)
    return keep


def deliveries(campaign_id, plans, answers=(), quarters=None):
    """Cumulative [staffing, repairs, access] after each quarter when `plans[t]` (a dict, or None to keep the programme) is
    approved in the first quarter of term t+1, delivered at full effort under the engine's lags and capacities, with the
    disruptions of this campaign's calendar. Work not delivered by the end of a term is dropped (a new plan replaces it)."""
    seed = news_seed(campaign_id)
    quarters = quarters or 6 * len(plans)
    total, out, left = {f: 0 for f in FAMILIES}, [], {f: 0 for f in FAMILIES}
    for q in range(1, quarters + 1):
        term, qt = (q - 1) // 6 + 1, (q - 1) % 6 + 1
        if qt == 1 and plans[term - 1] is not None:
            left = dict(plans[term - 1])
        keep = capacity_kept(seed, q, answers)
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
    return out


def severity(term):
    s, n = NEWS['severity'], max(0, term - 2)
    return (s['base_ppm'] * min(s['base_cap_pct'], 100 + s['base_step_pct'] * n) // 100,
            min(s['share_cap_pct'], s['share_pct'] + s['share_step_pct'] * n))


def landing_shock(tone, index, term, support):
    if tone == 'good':
        return [min(g, SHOCK_CAP) for g in NEWS['good'][index]['gain_ppm']]
    base, share = severity(term)
    out = []
    for exposure, now in zip(NEWS['bad'][index]['exposure_pct'], support):
        lead = max(0, now - HELD_ABOVE)
        out.append(-min(SHOCK_CAP, exposure * (base + lead * share // 100) // 100))
    return out


def soften(shock):
    return [sm.trunc_div(x * (100 - NEWS['relief']['cut_pct']), 100) if x < 0 else x for x in shock]


def overall_ppm(shock):
    """The groups' effects weighted by their share of the voters, truncated toward zero as the engine does."""
    return sm.trunc_div(sum(s * w for s, w in zip(shock, sm.W)), 1_000_000)


def relief_cost(shock):
    lost = sum(abs(s) * w for s, w in zip(shock, sm.W) if s < 0) // 1_000_000
    return max(1, -(-lost * NEWS['relief']['credits_per_point'] // 10_000))


def play(cums, campaign_id, quarters, answers=()):
    """Support after `quarters` quarters of the calendar and the given cumulative deliveries, as the engine does it: each
    quarter's headline lands first, then queued responses, then this quarter's delivery is queued. `answers` are the
    landing quarters whose headline was paid to soften. Returns (support, settled for election night, landed)."""
    seed, sup, queue, prev, landed = news_seed(campaign_id), [500_000] * 6, {}, [0, 0, 0], []
    book = {}
    for term in range(1, (quarters - 1) // 6 + 2):
        for q, tone, index, surprise in schedule(seed, term):
            book[q] = (tone, index, surprise)
    for q in range(1, quarters + 1):
        cum = cums[q - 1] if q - 1 < len(cums) else cums[-1]
        d = [cum[k] - prev[k] for k in range(3)]
        prev = list(cum)
        if q in book:
            tone, index, surprise = book[q]
            shock = landing_shock(tone, index, (q - 1) // 6 + 1, sup)
            if tone == 'bad' and q in answers:
                shock = soften(shock)
            sup = [max(0, min(10**6, s + v)) for s, v in zip(sup, shock)]
            landed.append((q, tone, NEWS[tone][index]['id'], surprise, shock))
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
        for q, tone, hid, surprise, shock in landed:
            print(f'         Q{q} {tone:4} {hid:22} {"surprise" if surprise else "announced":9} {shock}')


def check():
    """The numbers the engine's own tests pin (crates/domain/src/news.rs and the election-night test in
    crates/local-api/src/connected.rs). Both sides must say the same, or one of them is wrong."""
    ok = True

    def expect(name, got, want):
        nonlocal ok
        good = got == want
        ok &= good
        print(f'{"ok  " if good else "FAIL"} {name}' + ('' if good else f'\n       got  {got}\n       want {want}'))

    expect('schedule, seed 12345, term 1', schedule(12345, 1), [(2, 'good', 3, False), (4, 'good', 0, False)])
    expect('schedule, seed 12345, term 2', schedule(12345, 2),
           [(8, 'bad', 8, False), (9, 'bad', 5, True), (10, 'bad', 6, False), (11, 'bad', 4, False)])
    expect('schedule, seed 12345, term 3', schedule(12345, 3),
           [(14, 'bad', 7, False), (16, 'bad', 6, False), (17, 'bad', 4, True), (18, 'bad', 2, True)])
    expect('severity, terms 2, 3, 10', [severity(2), severity(3), severity(10)], [(5000, 40), (7500, 48), (20000, 90)])
    expect('bad news taxes the lead', landing_shock('bad', 3, 2, [500000, 500000, 500000, 560000, 520000, 500000]),
           [0, 0, 0, -29000, -13000, -5000])
    expect('a response softens by 60%', soften([-10000, -20000, 0, -30000, 0, 5000]), [-4000, -8000, 0, -12000, 0, 5000])
    expect('a response costs ceil(10 credits a point)', relief_cost([-10000, -20000, 0, -30000, 0, 5000]), 12)
    # Campaign "dz7" (crates/local-api/src/connected.rs, the election-night test): Balanced in term 1, then 50/35/0 in term 2,
    # with a flood announced for quarter 11 that stops Repairs for two quarters, answered in quarter 10.
    cid, plan2 = 'dz7', {'staffing': 50, 'repairs': 35, 'access': 0}
    sup, settled, landed = play(deliveries(cid, [BALANCED], quarters=6), cid, 6)
    expect('dz7: support before election night 1', sup, [602120, 613600, 484280, 603536, 620080, 495824])
    expect('dz7: election night 1', settled, [603800, 624000, 484200, 611240, 637600, 503960])
    expect('dz7: the headlines of term 1', [(q, h) for q, _, h, _, _ in landed], [(3, 'recruitment_up'), (6, 'attendance_rises')])
    sup9, _, _ = play(deliveries(cid, [BALANCED, plan2], quarters=9), cid, 9)
    expect('dz7: support at the start of quarter 10', sup9, [595420, 611100, 476420, 592156, 614300, 489244])
    flood = next(i for i, h in enumerate(NEWS['bad']) if h['id'] == 'flooding_halts_repairs')
    mood = landing_shock('bad', flood, 2, sup9)
    # 25 repair units still to deliver; stopped in quarters 11 and 12 only quarter 10's ten arrive: 15 slip past the election.
    delivery_effect = [-sm.response(ix, [0, 15, 0], **P) for ix in range(6)]
    total = [a + b for a, b in zip(mood, delivery_effect)]
    expect('dz7: what the delayed work would have earned, taken away', delivery_effect, [1200, -12000, -1200, -10140, -24600, -15060])
    expect('dz7: the quote for the flood', total, [-3116, -16944, -1700, -22698, -39816, -16560])
    expect('dz7: ...overall, truncated toward zero', overall_ppm(total), -12208)
    expect('dz7: ...and what answering costs', relief_cost(total), 13)
    expect('an answer turns a stoppage into a 60% slowdown', capacity_kept(news_seed(cid), 11, answers=(11,))['repairs'], 60)
    cums = deliveries(cid, [BALANCED, plan2], answers=(11,), quarters=12)
    expect('dz7: units delivered by quarter 12 with the answer', cums[-1], [100, 72, 10])
    _, settled12, landed12 = play(cums, cid, 12, answers=(11,))
    expect('dz7: election night 2', settled12, [632663, 673322, 461560, 609905, 664664, 501697])
    expect('dz7: the flood, answered', [shock for q, _, _, _, shock in landed12 if q == 11][0], [-1952, -2336, -200, -6819, -8396, -1217])
    return ok


if __name__ == '__main__':
    if '--check' in sys.argv:
        sys.exit(0 if check() else 1)
    report()
