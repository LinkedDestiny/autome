import json, sqlite3, os, collections
# 用法：先跑 agg.py 生成逐会话明细（AUTOME_TOKENS_ROWS），再 python3 -I rate.py
db = sqlite3.connect(f"file:{os.path.expanduser('~/.autome/state/automed.sqlite3')}?mode=ro", uri=True)
rows = json.load(open(os.environ.get("AUTOME_TOKENS_ROWS", "/tmp/autome-tokens-rows.json")))
tok = {r[2]: (r[5], r[0], r[3], r[4]) for r in rows}
start = {}; hourly = collections.defaultdict(lambda: collections.Counter()); conc = collections.Counter()
for p, at in db.execute("select payload, at from events where kind='session.started'"):
    start[json.loads(p)["session_id"]] = at
for p, at in db.execute("select payload, at from events where kind='session.ended'"):
    sid = json.loads(p)["session_id"]
    if sid in tok:
        t, proj, rt, role = tok[sid]; h = at[:13]
        hourly[h][proj] += t; hourly[h]["n"] += 1
for h in sorted(hourly)[-30:]:
    c = hourly[h]; print(h, "friday", round(c["friday"]/1e6), "M  MustAI", round(c["MustAI"]/1e6), "M  会话", c["n"])
# 并发：按 10 分钟取样，同时在跑的会话
import datetime as dt
iv = []
for p, at in db.execute("select payload, at from events where kind='session.ended'"):
    sid = json.loads(p)["session_id"]
    if sid in start: iv.append((start[sid], at))
print("会话总数", len(iv))
