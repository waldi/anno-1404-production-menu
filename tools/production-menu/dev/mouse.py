"""Drive the real mouse over the Anno window, in GUI client coordinates.

    python tools/production-menu/dev/mouse.py click X Y            # left click
    python tools/production-menu/dev/mouse.py wheel X Y N          # N notches, negative = down
    python tools/production-menu/dev/mouse.py drag X1 Y1 X2 Y2     # left-button drag in 10 steps
    python tools/production-menu/dev/mouse.py move X Y

Coordinates are the GUI's own space (the window client area, 1280x1024 in the current
setup), the same numbers annobridge reports at element+0x8C. Uses SetCursorPos + SendInput; the
window is brought to the foreground first (the game ignores the mouse while inactive).
Read-only with respect to game memory.
"""
import ctypes
import ctypes.wintypes as W
import sys
import time

u = ctypes.windll.user32
u.SetProcessDPIAware()

MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_WHEEL = 0x0002, 0x0004, 0x0800
MOUSEEVENTF_MOVE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_VIRTUALDESK = 0x0001, 0x8000, 0x4000
SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN = 76, 77, 78, 79


class MOUSEINPUT(ctypes.Structure):
    _fields_ = [("dx", ctypes.c_long), ("dy", ctypes.c_long), ("mouseData", W.DWORD),
                ("dwFlags", W.DWORD), ("time", W.DWORD), ("dwExtraInfo", ctypes.POINTER(ctypes.c_ulong))]


class INPUT(ctypes.Structure):
    class _U(ctypes.Union):
        _fields_ = [("mi", MOUSEINPUT)]
    _anonymous_ = ("u",)
    _fields_ = [("type", W.DWORD), ("u", _U)]


def send(flags, data=0, dx=0, dy=0):
    inp = INPUT(type=0)
    inp.mi = MOUSEINPUT(dx, dy, data & 0xFFFFFFFF, flags, 0, None)
    if u.SendInput(1, ctypes.byref(inp), ctypes.sizeof(INPUT)) != 1:
        raise OSError("SendInput failed")


def game_window():
    hit = []

    @ctypes.WINFUNCTYPE(ctypes.c_bool, W.HWND, W.LPARAM)
    def cb(hwnd, _):
        n = ctypes.create_unicode_buffer(256)
        u.GetWindowTextW(hwnd, n, 256)
        if u.IsWindowVisible(hwnd) and n.value.startswith("ANNO 1404"):
            hit.append(hwnd)
        return True
    u.EnumWindows(cb, 0)
    if not hit:
        sys.exit("no visible 'ANNO 1404' window")
    return hit[0]


def to_screen(hwnd, x, y):
    pt = W.POINT(int(x), int(y))
    u.ClientToScreen(hwnd, ctypes.byref(pt))
    return pt.x, pt.y


def move_to(hwnd, x, y):
    """SetCursorPos plus a one-pixel relative nudge. The game reads the OS cursor (GetCursorPos
    + ScreenToClient) but only while its window is the foreground window; see foreground()."""
    u.SetCursorPos(*to_screen(hwnd, x, y)); time.sleep(0.03)
    send(MOUSEEVENTF_MOVE, 0, 1, 1); send(MOUSEEVENTF_MOVE, 0, -1, -1)
    time.sleep(0.05)


def foreground(hwnd):
    """SetForegroundWindow is refused for a background process unless it just sent input:
    tap ALT first (the game logs an OnKeyPressed_ALT, harmless). Without this the game stays
    inactive and ignores the mouse entirely (2026-09-26: every event went to the root)."""
    u.keybd_event(0x12, 0, 0, 0); u.SetForegroundWindow(hwnd); u.keybd_event(0x12, 0, 2, 0)
    time.sleep(0.25)
    return u.GetForegroundWindow() == hwnd


def main():
    a = sys.argv[1:]
    if not a:
        print(__doc__); return 2
    hwnd = game_window()
    if not foreground(hwnd):
        sys.exit("could not bring the game to the foreground")
    cmd, args = a[0], [int(v) for v in a[1:]]
    if cmd == "move":
        move_to(hwnd, *args); time.sleep(0.05)
    elif cmd == "click":
        move_to(hwnd, *args); time.sleep(0.12)
        send(MOUSEEVENTF_LEFTDOWN); time.sleep(0.08); send(MOUSEEVENTF_LEFTUP)
    elif cmd == "wheel":
        x, y, n = args
        move_to(hwnd, x, y); time.sleep(0.12)
        for _ in range(abs(n)):
            send(MOUSEEVENTF_WHEEL, 120 if n > 0 else -120); time.sleep(0.05)
    elif cmd == "drag":
        x1, y1, x2, y2 = args
        move_to(hwnd, x1, y1); time.sleep(0.15)
        send(MOUSEEVENTF_LEFTDOWN); time.sleep(0.1)
        for i in range(1, 11):
            move_to(hwnd, x1 + (x2 - x1) * i / 10, y1 + (y2 - y1) * i / 10)
            time.sleep(0.04)
        time.sleep(0.1); send(MOUSEEVENTF_LEFTUP)
    else:
        print(__doc__); return 2
    print(f"{cmd} {args} done")
    return 0


if __name__ == "__main__":
    sys.exit(main())
