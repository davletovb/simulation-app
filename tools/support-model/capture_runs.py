#!/usr/bin/env python3
"""Play ten fixed strategies to the end against a running local service and save what happened.

This produces captured_runs_<version>.json, the evidence `support_model.py --check` uses to prove
its replica of the support model matches the real engine. Re-run it after any recalibration
(docs/decisions/0003-recalibrate-the-support-model.md). Standard library only.

    MYTH_SAVE_DIR=/tmp/myth-capture cargo run --locked -p myth-local-api     # in another terminal
    python3 tools/support-model/capture_runs.py v3

The game is deterministic, so capturing the same engine twice gives byte-identical files.
"""
import json, pathlib, sys, time, urllib.error, urllib.request, uuid

BASE = 'http://127.0.0.1:8787/api/campaigns'
OUT = pathlib.Path(__file__).parent
BAL = {'staff': 50, 'repairs': 40, 'access': 10}


def call(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(BASE + path, data=data, method=method, headers={'Content-Type': 'application/json'})
    try:
        with urllib.request.urlopen(req) as f:
            return f.status, json.load(f)
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read() or b'{}')


class Game:
    def __init__(self, name):
        self.id = f'{name}-{uuid.uuid4().hex[:6]}'
        assert call('POST', '/' + self.id, {})[0] in (200, 201), 'could not start a campaign; is the service running?'

    def snap(self):
        return call('GET', f'/{self.id}/snapshot')[1]['snapshot']

    def do(self, kind):
        cid, version = 'op-' + str(uuid.uuid4()), self.snap()['view_version']
        code, _ = call('POST', f'/{self.id}/commands', {'command_id': cid, 'expected_world_version': version, 'kind': kind})
        assert code in (200, 202), (code, kind)
        for _ in range(100):
            if call('GET', f'/{self.id}/commands/{cid}')[1]['status'] != 'accepted':
                break
            time.sleep(0.05)

    def adv(self):
        self.do({'kind': 'advance_slice'})

    def meeting(self, alloc, approve=True):
        self.do({'kind': 'propose', 'allocation': alloc}); self.adv()   # propose, hear reactions
        self.adv(); self.adv()                                           # keep the plan, final votes
        self.do({'kind': 'decide', 'ratify': approve})

    def keep(self):
        self.do({'kind': 'continue_quarter'}); self.do({'kind': 'finish_continuation'})


def meet(alloc, approve=True):
    def play(g):
        g.meeting(alloc, approve); g.adv()
    return play


def postpone(g):
    g.meeting(BAL, approve=False); g.adv()


def keep(g):
    g.keep()


STRATEGIES = {
    'A Balanced Q1': [meet(BAL)] + [keep] * 5,
    'B Staffing-first Q1': [meet({'staff': 60, 'repairs': 30, 'access': 10})] + [keep] * 5,
    'C Staff 45/40/10': [meet({'staff': 45, 'repairs': 40, 'access': 10})] + [keep] * 5,
    'D Balanced a quarter late': [postpone, meet(BAL)] + [keep] * 4,
    'E Balanced two quarters late': [postpone, postpone, meet(BAL)] + [keep] * 3,
    'F Stagger then re-plan': [meet({'staff': 45, 'repairs': 0, 'access': 0}), meet({'staff': 45, 'repairs': 35, 'access': 10})] + [keep] * 4,
    'G Never spend': [postpone] + [keep] * 5,
    'H Repairs-only 35': [meet({'staff': 0, 'repairs': 35, 'access': 0})] + [keep] * 5,
    'I Access-only 24 + Repairs 35': [meet({'staff': 0, 'repairs': 35, 'access': 24})] + [keep] * 5,
    'J Staffing-only 45': [meet({'staff': 45, 'repairs': 0, 'access': 0})] + [keep] * 5,
}

if __name__ == '__main__':
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    runs = {}
    for name, quarters in STRATEGIES.items():
        game = Game(name[0].lower())
        for quarter in quarters:
            quarter(game)
        state = game.snap()
        # The first term is what is captured: a win waits on election night for the second term (rules v3),
        # a loss closes the game.
        assert state['closed'] or state['phase'] == 'ElectionNight', f'{name}: the first term did not finish'
        runs[name] = [{'q': r['quarter'], 'cum': r['reported_cumulative'], 'support': r['weighted_support_ppm']}
                      for r in state['delivery_history']]
        print(f"{name:32} final support {runs[name][-1]['support'] / 1e4:.3f}%")
    path = OUT / f'captured_runs_{sys.argv[1]}.json'
    path.write_text(json.dumps(runs))
    print('wrote', path)
