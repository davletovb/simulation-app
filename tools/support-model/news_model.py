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
           [(8, 'bad', 4, False), (9, 'bad', 5, True), (10, 'bad', 6, False), (11, 'bad', 7, False)])
    expect('schedule, seed 12345, term 3', schedule(12345, 3),
           [(14, 'bad', 7, False), (16, 'bad', 6, False), (17, 'bad', 0, True), (18, 'bad', 1, True)])
    expect('severity, terms 2, 3, 10', [severity(2), severity(3), severity(10)], [(5000, 40), (7500, 48), (20000, 90)])
    expect('bad news taxes the lead', landing_shock('bad', 3, 2, [500000, 500000, 500000, 560000, 520000, 500000]),
           [0, 0, 0, -29000, -13000, -5000])
    expect('a response softens by 60%', soften([-10000, -20000, 0, -30000, 0, 5000]), [-4000, -8000, 0, -12000, 0, 5000])
    expect('a response costs ceil(10 credits a point)', relief_cost([-10000, -20000, 0, -30000, 0, 5000]), 12)
    sup, settled, landed = first_election('term')
    expect('campaign "term": support before election night', sup, [582120, 603600, 484280, 608536, 635080, 520824])
    expect('campaign "term": election night', settled, [583800, 614000, 484200, 616240, 652600, 528960])
    expect('campaign "term": the headlines of term 1', [(q, h) for q, _, h, _, _ in landed],
           [(5, 'recruitment_up'), (6, 'volunteer_surge')])
    cums = [[sum([0, 12, 12, 12, 12, 2][:q]), sum([0, 0, 10, 10, 10, 10][:q]), sum([4, 5, 1, 0, 0, 0][:q])] for q in range(1, 7)]
    cums += [cums[-1]] * 6
    _, settled12, _ = play(cums, 'term', 12, answers=(8,))
    expect('campaign "term": a hands-off second term, one headline answered', settled12,
           [526848, 549523, 479700, 550310, 586694, 518367])
    expect('...and it loses', sum(w * s for w, s in zip(sm.W, settled12)) // 10**6 < BAR, True)
    return ok


if __name__ == '__main__':
    if '--check' in sys.argv:
        sys.exit(0 if check() else 1)
    report()
