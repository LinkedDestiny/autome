import json, glob, os, collections, sqlite3, sys
# 用法：python3 -I turns.py 项目根目录 [项目根目录 ...]
db = sqlite3.connect(f"file:{os.path.expanduser('~/.autome/state/automed.sqlite3')}?mode=ro", uri=True)
meta = {}
for p, in db.execute("select payload from events where kind='session.started'"):
    d = json.loads(p); k = d.get("kind", {}); meta[d["session_id"]] = (k.get("role") or k.get("kind"), d.get("runtime"))
ends = collections.Counter()
for p, in db.execute("select payload from events where kind='session.ended'"):
    d = json.loads(p); ends[(meta.get(d["session_id"],("?","?"))[0], d.get("end"))] += 1
S = collections.defaultdict(list)
for root in sys.argv[1:]:
  for f in glob.glob(os.path.join(os.path.expanduser(root),".autome/output/sessions")+"/*/*.jsonl"):
    sid=os.path.basename(f)[:-6]; role,rt=meta.get(sid,("?","?"))
    turns=0; inp=0; models=set(); calls=0; maxctx=0
    for line in open(f,errors="ignore"):
        if rt=="claude" and '"type":"assistant"' in line:
            try: d=json.loads(line)
            except: continue
            m=d.get("message",{}); models.add(m.get("model")); us=m.get("usage") or {}
            ctx=us.get("input_tokens",0)+us.get("cache_read_input_tokens",0)+us.get("cache_creation_input_tokens",0)
            maxctx=max(maxctx,ctx)
        if '"type":"result"' in line:
            try: d=json.loads(line); turns=d.get("num_turns",0)
            except: pass
        if rt=="codex" and ('"type":"item.completed"' in line and 'command_execution' in line): calls+=1
    S[(rt,role)].append((turns,maxctx,calls,tuple(sorted(x for x in models if x))))
for k,v in sorted(S.items()):
    t=[x[0] for x in v]; c=[x[1] for x in v]; cl=[x[2] for x in v]; ms=collections.Counter(x[3] for x in v).most_common(2)
    n=len(v); 
    print(k, "会话",n, "平均轮数", round(sum(t)/n,1), "平均峰值上下文K", round(sum(c)/n/1000), "codex平均命令数", round(sum(cl)/n,1), ms)
print("会话结束方式", sorted(ends.items(), key=lambda x:-x[1])[:14])
