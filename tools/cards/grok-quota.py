#!/usr/bin/env python3
"""读 Grok 订阅的「每周额度」：已用百分比和重置时间。

Fox 的 Grok 是订阅制，要看的是额度百分比，不是美元。Grok 没有能在无界面模式下查额度的命令，
所以这里在一个伪终端里启动它的界面，输入 /usage，把「Usage limit」那一页的文字读出来，然后退出。
不会调用模型，不花额度；会多出一条空会话记录，无害。

用法：python3 tools/cards/grok-quota.py        # 输出一行摘要
      python3 tools/cards/grok-quota.py --raw  # 同时打印读到的原始屏幕文字
"""
import fcntl
import os
import pty
import re
import select
import signal
import struct
import sys
import termios
import time

GROK = os.path.expanduser("~/.grok/bin/grok")
ANSI = re.compile(r"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07]*\x07|\x1b[()][0-9A-B]|\x1b[=>]")


def read_screen(cols=140, rows=50):
    pid, fd = pty.fork()
    if pid == 0:
        os.chdir("/tmp")
        env = dict(os.environ, TERM="xterm-256color", COLUMNS=str(cols), LINES=str(rows))
        os.execvpe(GROK, [GROK, "--no-alt-screen"], env)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
    out = b""

    def drain(seconds):
        nonlocal out
        end = time.time() + seconds
        while time.time() < end:
            ready, _, _ = select.select([fd], [], [], 0.3)
            if ready:
                try:
                    chunk = os.read(fd, 65536)
                except OSError:
                    return
                if not chunk:
                    return
                out += chunk

    try:
        drain(5.0)  # 等界面起来
        os.write(fd, b"/usage")
        drain(1.5)
        os.write(fd, b"\r")
        drain(7.0)  # 等额度页加载
        os.write(fd, b"\x03")
        drain(0.5)
        os.write(fd, b"\x03")
        drain(1.0)
    except OSError:
        pass
    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    text = ANSI.sub("", out.decode("utf-8", "replace")).replace("\r", "")
    return text


def main():
    text = read_screen()
    # 终端画面里的光标定位被去掉后，整屏会挤成一行，所以直接在整段文字里找
    flat = re.sub(r"[│╭╮╰╯─]+", " ", text)
    flat = re.sub(r"\s+", " ", flat)
    match = re.search(
        r"(Weekly limit[^█░%]*?)\s*[█░]+\s*(\d+)\s*%\s*Resets:\s*([A-Za-z]+ \d+,? \d+:\d+)", flat
    )
    if match:
        label = match.group(1).strip()
        print(f"{label}：已用 {match.group(2)}%，重置时间 {match.group(3)}")
    else:
        hits = re.findall(r"[^.]{0,60}(?:limit|Resets|remaining)[^.]{0,80}", flat, re.I)
        print("没解析出「每周额度」，相关片段：")
        for hit in hits[-6:]:
            print("  ", hit.strip())
    if "--raw" in sys.argv:
        print("\n---- 原始屏幕文字（末 1500 字）----")
        print(flat[-1500:])


if __name__ == "__main__":
    main()
