import sqlite3, json, re, sys, os
HERE = os.path.dirname(os.path.abspath(__file__))
DB = sys.argv[1] if len(sys.argv) > 1 else sys.exit("用法：python3 extract.py <dev 库的副本.sqlite3>")
db = sqlite3.connect(DB); db.row_factory = sqlite3.Row

SCEN = [
 ("xhs", "小红书",        ["小红书","xiaohongshu","rednote","xhs"]),
 ("wx",  "公众号·微信",   ["公众号","微信","wechat","weixin"]),
 ("dy",  "抖音·短视频",   ["抖音","短视频","douyin","tiktok","shorts","reels"]),
 ("img", "海报·封面·设计", ["海报","封面","poster","logo","figma","illustration","图片生成","image generation","图像生成","text-to-image","文生图","设计稿","视觉设计","graphic design","ui design","图文卡片","信息图","infographic"]),
 ("vid", "视频·字幕·语音", ["视频","字幕","语音","video","subtitle","whisper","tts","asr","transcri","剪辑","配音"]),
 ("ppt", "PPT·文档",      ["ppt","幻灯","slides","presentation","pptx","docx","excel","pdf","办公文档","文档处理","文档转换"]),
 ("wr",  "写作·文案",      ["写作","文案","copywriting","writing","博客","newsletter","自媒体","小说","novel"]),
 ("res", "调研·爬虫",      ["调研","爬虫","scrap","crawl","research","采集","抓取","web search","deep research"]),
 ("data","数据分析",       ["数据分析","analytics","dashboard","数据可视","sql","报表"]),
 ("kb",  "知识库·笔记",    ["知识库","obsidian","notion","rag","笔记","knowledge base","second brain","wiki"]),
 ("auto","自动化·工作流",  ["自动化","automation","workflow","n8n","zapier","工作流","rpa"]),
 ("ec",  "电商",           ["电商","ecommerce","e-commerce","shopify","淘宝","天猫","京东","带货","商品"]),
 ("seo", "SEO·增长",       ["seo","增长","growth hack","营销","marketing","广告","投放","获客","私域"]),
]
def hit(h, kw):
    if kw.isascii():
        return re.search(r"(?<![a-z0-9])" + re.escape(kw.strip()) + r"(?![a-z0-9])", h) is not None
    return kw in h
def scen_tags(h):
    scored = []
    for sid, label, kws in SCEN:
        n = sum(1 for k in kws if hit(h, k))
        if n: scored.append((n, sid))
    scored.sort(key=lambda x: -x[0])
    return [s for _, s in scored[:3]]

FORM_ZH = {"app":"应用","cli":"命令行","skill":"技能","mcp":"MCP","api":"接口","library":"代码库","service":"服务","collection":"合集","other":""}
MAT_ZH = {"mature":"成熟","stable":"成熟","usable":"可用","beta":"可用","early":"早期","prototype":"原型","experimental":"实验"}
def form_from_topics(tp, name):
    t = " ".join(tp).lower() + " " + name.lower()
    if re.search(r"(?<![a-z])mcp(?![a-z])", t): return "MCP"
    if "skill" in t: return "技能"
    if "plugin" in t: return "插件"
    if re.search(r"(?<![a-z])cli(?![a-z])", t): return "命令行"
    return ""

def first_sentence(text, full_name, limit=52):
    s = (text or "").strip().replace("\n", " ")
    s = re.sub(r"^" + re.escape(full_name) + r"\s*[（(][^）)]*[）)]?\s*", "", s)
    s = re.sub(r"^" + re.escape(full_name) + r"\s*(是|为)?\s*", "", s)
    s = re.sub(r"^[^，。]{0,40}?\s(是一个|是一款|是一套|是一种|是)\s*", lambda m: m.group(0) if False else "", s, count=0) if False else s
    cut = re.split(r"[。！？\n]", s)[0]
    if len(cut) > limit:
        cut = cut[:limit].rstrip("，、；: ") + "…"
    return cut

rows = db.execute("""
 SELECT r.id, r.full_name, r.description, r.language, r.stars_count, r.starred_at, r.pushed_at, r.topics_json,
        d.summary_zh, d.keywords_json, d.one_liner_zh, d.card_json, d.prompt_version
 FROM repositories r LEFT JOIN repo_ai_documents d ON d.repo_id = r.id
 WHERE r.sync_status='active' ORDER BY r.starred_at DESC""").fetchall()

BAD = {"how_to_try": 0, "one_liner_zh": 0, "what_it_is": 0, "capabilities": 0, "problems_solved": 0}
def txt(v, key=None):
    if isinstance(v, str): return v.strip()
    if v is None: return ""
    if key: BAD[key] += 1
    if isinstance(v, list): return "；".join(txt(x) for x in v if x)
    if isinstance(v, dict): return "；".join(txt(x) for x in v.values() if x)
    return str(v)

def jl(v):
    try: return json.loads(v) if v else []
    except Exception: return []

items = []
for r in rows:
    tp = jl(r["topics_json"])[:6]
    card = {}
    real = (r["prompt_version"] == "card-v1")
    if real:
        try: card = json.loads(r["card_json"] or "{}")
        except Exception: card = {}
    summary = (r["summary_zh"] or "").strip()
    desc = (r["description"] or "").strip()
    one = ""; sm = ""; caps = []; probs = []; how = ""; form = ""; mat = ""; cn = None
    if real:
        one = txt(card.get("one_liner_zh"), "one_liner_zh") or txt(r["one_liner_zh"])
        sm = txt(card.get("what_it_is"), "what_it_is") or summary
        raw_caps = card.get("capabilities")
        if raw_caps is not None and not isinstance(raw_caps, list): BAD["capabilities"] += 1; raw_caps = [raw_caps]
        for c in raw_caps or []:
            caps.append(txt(c.get("name_zh") if isinstance(c, dict) else c))
        caps = [c for c in caps if c][:5]
        probs = [txt(p) for p in (card.get("problems_solved") if isinstance(card.get("problems_solved"), list) else [card.get("problems_solved")] if card.get("problems_solved") else [])][:3]
        how = txt(card.get("how_to_try"), "how_to_try")
        form = FORM_ZH.get(str(card.get("form") or "other"), "")
        mat = MAT_ZH.get(str(card.get("maturity") or ""), "")
        cn = card.get("zh_friendly")
    elif summary:
        one = first_sentence(summary, r["full_name"])
        sm = summary
        caps = [str(k) for k in jl(r["keywords_json"])][:6]
    if len(sm) > 360:
        cut = sm[:360]
        i = max(cut.rfind("。"), cut.rfind("；"))
        sm = (cut[:i+1] if i > 120 else cut.rstrip() + "…")
    if not form: form = form_from_topics(tp, r["full_name"])
    hay = " ".join([r["full_name"], desc, " ".join(tp), summary, one, " ".join(caps)]).lower()
    items.append({
        "id": r["id"], "n": r["full_name"], "d": desc[:170], "l": r["language"] or "",
        "s": r["stars_count"] or 0, "t": (r["starred_at"] or "")[:10], "p": (r["pushed_at"] or "")[:10],
        "tp": tp, "z": one, "sm": sm, "cap": caps, "pr": probs, "how": how, "form": form, "mat": mat,
        "cn": (1 if cn else 0) if cn is not None else -1, "real": 1 if real else 0, "has": 1 if one else 0,
        "sc": scen_tags(hay),
    })

counts = {sid: sum(1 for i in items if sid in i["sc"]) for sid, _, _ in SCEN}
print("真卡里类型不符 schema 的字段（张数）:", BAD)
print("总数", len(items), "有中文说明", sum(i["has"] for i in items), "真卡", sum(i["real"] for i in items))
print("场景计数(全部492):", {l: counts[s] for s, l, _ in SCEN})
print("场景计数(有中文说明):", {l: sum(1 for i in items if i["has"] and s in i["sc"]) for s, l, _ in SCEN})
print("近7天(相对最新一条):", end=" ")
mx = max(i["t"] for i in items); print(mx)

# 榜单
ranks = []
for src in ("github_search", "hn_show"):
    snap = db.execute("SELECT id FROM ranking_snapshots WHERE source_id=? ORDER BY fetched_at DESC LIMIT 1", (src,)).fetchone()
    if not snap: continue
    for e in db.execute("SELECT rank, full_name, description, html_url, stars_count FROM ranking_entries WHERE snapshot_id=? ORDER BY rank", (snap[0],)):
        hay = ((e["full_name"] or "") + " " + (e["description"] or "")).lower()
        ranks.append({"src": src, "r": e["rank"], "n": e["full_name"], "d": (e["description"] or "")[:150], "u": e["html_url"], "s": e["stars_count"] or 0, "sc": scen_tags(hay)})
print("榜单条目", len(ranks), "其中有场景标签", sum(1 for r in ranks if r["sc"]))

out = {"scen": [{"id": s, "n": l} for s, l, _ in SCEN], "items": items, "ranks": ranks,
       "starred": [i["n"].lower() for i in items], "asof": mx}
json.dump(out, open(os.path.join(HERE, "data.json"), "w", encoding="utf-8"), ensure_ascii=False, separators=(",", ":"))
print("data.json 大小:", os.path.getsize(os.path.join(HERE, "data.json")) // 1024, "KB")
