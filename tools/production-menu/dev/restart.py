"""Hands-free restart of Anno 1404 Venice with the annobridge DLL and a loaded save.

    python tools/production-menu/dev/restart.py                 # newest autosave
    python tools/production-menu/dev/restart.py --save RE01     # Documents/ANNO 1404 Venice/Savegames/RE01.sww
    python tools/production-menu/dev/restart.py --no-install    # keep the d3d9.dll already in the game dir

Sequence: close Addon.exe, copy tools/production-menu/d3d9.dll and
AnnoRECompanion.exe next to Addon.exe (the game copy INSIDE this repo, never another
install), start it, wait for the bridge, wait for
the intro movie to finish, post the savegame through MainControl_LoadSavegameFile,
wait for the loading screen's "Set sail!" button, dispatch OnLeftClick to it, wait for
the in-game HUD.

Intro movie: the main menu's GUI tree is already live while the intro plays, so the
menu being reachable is NOT a readiness signal. Posting the load during the movie
works but leaves the game with a broken mouse (no cursor over the world, no map drag,
movie frames bleeding into the loaded game). Measured 2026-09-18: the intro takes
about 120 s; --intro-wait defaults to 135 s after process start.
"""

import argparse
import glob
import os
import shutil
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
BRIDGE = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(BRIDGE))
GAME_DIR = os.path.join(REPO, "Anno 1404 Gold Edition")
SAVE_DIR = os.path.join(os.path.expanduser("~"), "Documents", "ANNO 1404 Venice", "Savegames")
SEND = [sys.executable, os.path.join(HERE, "send.py")]


def send(*args, quiet=False):
    r = subprocess.run(SEND + list(args), capture_output=True, text=True, errors="replace")
    out = (r.stdout + r.stderr).strip()
    if not quiet:
        print("   " + out.replace("\n", "\n   "))
    return out


def game_running():
    r = subprocess.run(["tasklist", "/FI", "IMAGENAME eq Addon.exe"], capture_output=True, text=True)
    return "Addon.exe" in r.stdout


def companion_running():
    r = subprocess.run(["tasklist", "/FI", "IMAGENAME eq AnnoRECompanion.exe"],
                       capture_output=True, text=True)
    return "AnnoRECompanion.exe" in r.stdout


def wait_for(desc, pred, timeout, interval=2.0):
    print(f"-- waiting for {desc} (up to {timeout:.0f}s)")
    t0 = time.time()
    while time.time() - t0 < timeout:
        if pred():
            print(f"   ok after {time.time() - t0:.0f}s")
            return True
        time.sleep(interval)
    print(f"   TIMEOUT after {timeout:.0f}s")
    return False


def element_live(name):
    return "-> 2 done" in send("--children", name, quiet=True)


def element_visible(fullpath):
    out = send("--inspect", fullpath, "--at", "0x9C", "--count", "1", quiet=True)
    return "+0x009C" in out and out.rstrip().endswith("0x00000001")


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--save", help="savegame name or path (default: newest autosave_*.sww)")
    p.add_argument("--no-install", action="store_true", help="do not copy d3d9.dll into the game dir")
    p.add_argument("--intro-wait", type=float, default=135.0,
                   help="seconds after launch before the load is posted (intro movie, ~120s)")
    p.add_argument("--no-load", action="store_true", help="stop at the main menu")
    a = p.parse_args()

    if a.save:
        save = a.save if os.path.isabs(a.save) else os.path.join(SAVE_DIR, a.save)
        if not save.lower().endswith(".sww"):
            save += ".sww"
    else:
        autos = glob.glob(os.path.join(SAVE_DIR, "autosave_*.sww"))
        if not autos:
            sys.exit("no autosave found in " + SAVE_DIR)
        save = max(autos, key=os.path.getmtime)
    if not a.no_load and not os.path.isfile(save):
        sys.exit("no such savegame: " + save)

    exe = os.path.join(GAME_DIR, "Addon.exe")
    if not os.path.isfile(exe):
        sys.exit("Addon.exe not found in " + GAME_DIR)

    if companion_running():
        print("-- closing AnnoRECompanion.exe")
        subprocess.run(["powershell", "-NoProfile", "-Command",
                        "Stop-Process -Name AnnoRECompanion -Force -ErrorAction SilentlyContinue"])
        if not wait_for("companion exit", lambda: not companion_running(), 10, 0.5):
            sys.exit("companion did not exit")

    if game_running():
        print("-- closing Addon.exe")
        subprocess.run(["powershell", "-NoProfile", "-Command",
                        "Stop-Process -Name Addon -Force -ErrorAction SilentlyContinue"])
        if not wait_for("process exit", lambda: not game_running(), 30, 1.0):
            sys.exit("game did not exit")

    if not a.no_install:
        files = (
            ("d3d9.dll", os.path.join(BRIDGE, "d3d9.dll")),
            ("AnnoRECompanion.exe", os.path.join(BRIDGE, "AnnoRECompanion.exe")),
            ("production_dialog_v5.dlg", os.path.join(BRIDGE, "examples", "production_dialog_v5.dlg")),
            ("production_button.dlg", os.path.join(BRIDGE, "examples", "production_button.dlg")),
        )
        for filename, src in files:
            dst = os.path.join(GAME_DIR, filename)
            if not os.path.isfile(src):
                sys.exit(f"{filename} not built; run tools/production-menu/build.ps1 first")
            shutil.copyfile(src, dst)
            print(f"-- installed {os.path.getsize(src)} bytes -> {dst}")

    print("-- starting", exe)
    t_launch = time.time()
    # Detach: the game must survive this script (or the shell that runs it) being killed.
    # Without these flags, stopping a backgrounded restart.py took Addon.exe down with it.
    flags = subprocess.DETACHED_PROCESS | subprocess.CREATE_NEW_PROCESS_GROUP
    subprocess.Popen([exe], cwd=GAME_DIR, creationflags=flags, close_fds=True,
                     stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    if not wait_for("bridge (magic ok)", lambda: "magic     ok" in send("--status", quiet=True), 90, 3.0):
        sys.exit("bridge never came up - is d3d9.dll loading? DirectX 9 mode required")
    wait_for("main menu tree", lambda: element_live("MAIN_MENU"), 90, 3.0)

    remaining = a.intro_wait - (time.time() - t_launch)
    if remaining > 0:
        print(f"-- intro movie: waiting {remaining:.0f}s more (total {a.intro_wait:.0f}s after launch)")
        time.sleep(remaining)

    if a.no_load:
        print("-- main menu reached; --no-load given, done")
        return 0

    print("-- posting load of", save)
    send("--load", save)
    # Plain name on purpose: the registry's full-path map is only rebuilt when a runtime
    # element is created, so screens loaded afterwards (the loading screen) are not in it.
    # A plain name matches while the element dispatches events, i.e. while it is on screen.
    if not wait_for("Set sail! button", lambda: element_visible("START_MISSION"), 180, 3.0):
        sys.exit("loading screen button never appeared")
    time.sleep(2)
    send("--click", "START_MISSION")
    if not wait_for("in-game HUD (INFOBAR)", lambda: element_live("INFOBAR"), 120, 3.0):
        sys.exit("HUD never appeared after Set sail")
    print("-- in game.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
