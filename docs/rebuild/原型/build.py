import json, sys
import os
HERE = os.path.dirname(os.path.abspath(__file__))
SP = HERE
d = json.load(open(SP + "/data.json", encoding="utf-8"))
d["cand"] = json.load(open(SP + "/cand.json", encoding="utf-8"))
html = open(SP + "/template.html", encoding="utf-8").read()
payload = "const DATA = " + json.dumps(d, ensure_ascii=False, separators=(",", ":")).replace("</", "<\\/") + ";"
assert "/*__DATA__*/" in html
open(SP + "/fox-stars-lab-prototype-v1.html", "w", encoding="utf-8").write(html.replace("/*__DATA__*/", payload))
import os; print("已构建", os.path.getsize(SP + "/fox-stars-lab-prototype-v1.html") // 1024, "KB")

wrapped = '<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover"><style>[hidden]{display:none!important}body{margin:0}</style></head><body>' + open(SP + "/fox-stars-lab-prototype-v1.html", encoding="utf-8").read() + '</body></html>'
open(SP + "/test.html", "w", encoding="utf-8").write(wrapped)
print("已生成本地测试外壳 test.html")
