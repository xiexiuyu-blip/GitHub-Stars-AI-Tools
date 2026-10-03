#!/usr/bin/env python3
"""给界面做验收用的「假后端」代理。

把 vite 开发服务器（默认 127.0.0.1:1420）原样转发，只在首页里先塞一段脚本（shim.js），
让页面以为自己跑在 Tauri 里：所有 invoke 命令由 shim.js 里的假数据回答。
这样可以在浏览器里看新界面、读控制台报错、换窗口宽度，不碰 Fox 的开发版、开发库和钥匙串。

用法：python3 tools/dev-ui/mock-host/serve.py [端口，默认 38413] [上游，默认 http://127.0.0.1:1420]
只监听 127.0.0.1。不转发 websocket（热更新在这里不起作用，改完代码刷新页面即可）。
"""
import http.server
import os
import re
import socketserver
import sys
import urllib.error
import urllib.request

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 38413
UPSTREAM = (sys.argv[2] if len(sys.argv) > 2 else "http://127.0.0.1:1420").rstrip("/")
HERE = os.path.dirname(os.path.abspath(__file__))
SHIM_URL = "/__mock__/shim.js"


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, fmt, *args):  # 安静一点
        pass

    def _send(self, status, headers, body):
        self.send_response(status)
        for key, value in headers:
            self.send_header(key, value)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path.split("?")[0] == "/__mock__/data.json":
            data_path = os.path.normpath(os.path.join(HERE, "..", "..", "..", "docs", "rebuild", "原型", "data.json"))
            with open(data_path, "rb") as handle:
                return self._send(200, [("Content-Type", "application/json; charset=utf-8")], handle.read())
        if self.path.split("?")[0] == SHIM_URL:
            with open(os.path.join(HERE, "shim.js"), "rb") as handle:
                body = handle.read()
            return self._send(200, [("Content-Type", "application/javascript; charset=utf-8")], body)
        try:
            request = urllib.request.Request(UPSTREAM + self.path, headers={"Accept": self.headers.get("Accept", "*/*")})
            with urllib.request.urlopen(request, timeout=30) as response:
                body = response.read()
                content_type = response.headers.get("Content-Type", "application/octet-stream")
                status = response.status
        except urllib.error.HTTPError as error:
            return self._send(error.code, [("Content-Type", "text/plain; charset=utf-8")], str(error).encode())
        except Exception as error:  # 上游没起来
            return self._send(502, [("Content-Type", "text/plain; charset=utf-8")], f"上游不可用：{error}".encode())
        if "text/html" in content_type:
            html = body.decode("utf-8")
            tag = f'<script src="{SHIM_URL}"></script>'
            if re.search(r"<head[^>]*>", html, re.I):
                html = re.sub(r"(<head[^>]*>)", r"\1" + tag, html, count=1, flags=re.I)
            else:
                html = tag + html
            body = html.encode("utf-8")
        return self._send(status, [("Content-Type", content_type)], body)


class Server(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True
    allow_reuse_address = True


if __name__ == "__main__":
    print(f"假后端代理：http://127.0.0.1:{PORT}  →  {UPSTREAM}", flush=True)
    Server(("127.0.0.1", PORT), Handler).serve_forever()
