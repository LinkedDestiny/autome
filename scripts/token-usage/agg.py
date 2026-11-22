import json, sqlite3, os, glob, sys, collections
db = sqlite3.connect(f"file:{os.path.expanduser('~/.autome/state/automed.sqlite3')}?mode=ro", uri=True)
meta = {}
for subj, p in db.execute("select subject_id, payload from events where kind='session.started'"):
    d = json.loads(p); k = d.get("kind", {})
    meta[d["session_id"]] = (k.get("role") or k.get("kind"), d.get("runtime"), subj)
# 用法：python3 -I agg.py 项目名=项目根目录 [项目名=项目根目录 ...]
roots = {a.split("=",1)[0]: os.path.join(os.path.expanduser(a.split("=",1)[1]), ".autome/output/sessions") for a in sys.argv[1:]}
if not roots: sys.exit("用法：python3 -I agg.py 项目名=项目根目录 ...")
agg = collections.defaultdict(lambda: collections.Counter())
rows = []
for proj, root in roots.items():
    for f in glob.glob(f"{root}/*/*.jsonl"):
        sid = os.path.basename(f)[:-6]
        role, rt, task = meta.get(sid, ("?", "?", "?"))
        u = None; cost = 0.0; turns = 0
        with open(f, errors="ignore") as fh:
            for line in fh:
                if '"usage"' not in line: continue
                try: d = json.loads(line)
                except: continue
                if d.get("type") == "result":
                    us = d["usage"]; cost = d.get("total_cost_usd") or 0; turns = d.get("num_turns") or 0
                    u = dict(fresh=us.get("input_tokens",0)+us.get("cache_creation_input_tokens",0), cached=us.get("cache_read_input_tokens",0), out=us.get("output_tokens",0))
                elif d.get("type") == "turn.completed":
                    us = d["usage"]; c = us.get("cached_input_tokens",0)
                    u0 = u or dict(fresh=0,cached=0,out=0)
                    u = dict(fresh=u0["fresh"]+us.get("input_tokens",0)-c, cached=u0["cached"]+c, out=u0["out"]+us.get("output_tokens",0))
        if not u: continue
        psz = os.path.getsize(f[:-6]+".prompt") if os.path.exists(f[:-6]+".prompt") else 0
        key = (proj, rt, role)
        a = agg[key]; a["n"] += 1; a["fresh"] += u["fresh"]; a["cached"] += u["cached"]; a["out"] += u["out"]; a["cost"] += cost; a["psz"] += psz
        rows.append((proj, task, sid, rt, role, u["fresh"]+u["cached"], u["out"], cost))
tot = sum(a["fresh"]+a["cached"] for a in agg.values())
print(f"{'项目':6} {'运行时':6} {'角色':10} {'会话':>5} {'输入M':>8} {'其中缓存%':>8} {'每会话输入M':>10} {'输出M':>7} {'占比%':>6} {'美元':>7} {'prompt KB/会话':>8}")
for k, a in sorted(agg.items(), key=lambda kv: -(kv[1]["fresh"]+kv[1]["cached"])):
    i = a["fresh"]+a["cached"]
    print(f"{k[0]:6} {str(k[1]):6} {str(k[2]):10} {a['n']:5} {i/1e6:8.1f} {100*a['cached']/max(i,1):8.1f} {i/a['n']/1e6:10.2f} {a['out']/1e6:7.2f} {100*i/tot:6.1f} {a['cost']:7.1f} {a['psz']/a['n']/1024:8.1f}")
print("总输入 M", round(tot/1e6,1))
rows.sort(key=lambda r: -r[5])
print("\n最大的 12 个会话：")
for r in rows[:12]: print(r[0], r[1], r[3], r[4], f"in={r[5]/1e6:.1f}M out={r[6]/1e3:.0f}k ${r[7]:.2f}", r[2])
json.dump(rows, open(os.environ.get("AUTOME_TOKENS_ROWS", "/tmp/autome-tokens-rows.json"),"w"))
