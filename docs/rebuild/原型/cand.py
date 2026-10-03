import json, time, subprocess, urllib.parse, os
HERE = os.path.dirname(os.path.abspath(__file__))
d = json.load(open(os.path.join(HERE, "data.json"), encoding="utf-8"))
starred = set(d["starred"])
QUERIES = [
 ("seo",  "SEO",       "seo ai agent skills in:description pushed:>2026-08-01 stars:>150"),
 ("seo",  "SEO",       "marketing skills claude in:description pushed:>2026-08-01 stars:>200"),
 ("ec",   "电商",      "shopify mcp in:name,description pushed:>2026-08-01 stars:>100"),
 ("ec",   "电商",      "电商 agent in:description pushed:>2026-07-01 stars:>100"),
 ("xhs",  "小红书",    "xiaohongshu in:name,description pushed:>2026-08-01 stars:>150"),
 ("wx",   "公众号",    "微信公众号 in:description pushed:>2026-08-01 stars:>150"),
 ("dy",   "抖音",      "douyin OR 抖音 in:description pushed:>2026-08-01 stars:>200"),
]
out = []; seen = set()
for sid, label, q in QUERIES:
    url = "https://api.github.com/search/repositories?q=" + urllib.parse.quote(q) + "&sort=stars&order=desc&per_page=8"
    r = subprocess.run(["curl", "-sS", "--max-time", "20", "-H", "Accept: application/vnd.github+json", "-A", "fox-stars-lab-proto", url], capture_output=True, text=True)
    try: j = json.loads(r.stdout)
    except Exception: print("ERR", sid, r.stdout[:120], r.stderr[:120]); time.sleep(7); continue
    items = j.get("items") or []
    print("==", label, "|", q[:50], "|", len(items), "条", ("| " + str(j.get("message"))[:80]) if "message" in j else "")
    for i in items:
        n = i["full_name"]
        if n.lower() in seen: continue
        seen.add(n.lower())
        mine = n.lower() in starred
        print("   ", "[已收藏]" if mine else "        ", n, "★%d" % i["stargazers_count"], "|", (i["description"] or "")[:70])
        out.append({"n": n, "s": i["stargazers_count"], "d": (i["description"] or "")[:160], "u": i["html_url"], "p": (i["pushed_at"] or "")[:10], "sc": sid, "mine": mine, "l": i.get("language") or ""})
    time.sleep(7)
json.dump(out, open(os.path.join(HERE, "cand.json"), "w", encoding="utf-8"), ensure_ascii=False)
print("候选共", len(out), "其中已收藏", sum(1 for c in out if c["mine"]))
