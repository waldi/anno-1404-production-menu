"""Hardware DATA write-breakpoint: find WHO writes a given address.

    python tools/research/annowatch.py 0x24048fdc [0x24044b14 ...] [--hits N] [--secs S]
    python tools/research/annowatch.py --stock SaltFish Cider      (resolve live nodes)

THIS ATTACHES A DEBUGGER AND SETS CPU DEBUG REGISTERS. It writes no game
memory. The game is frozen only while a trap is being handled, and the
registers are cleared and the process detached on exit.

Why this exists: chasing a writer through the call graph failed -- resident
consumption does not go through CWarehouseChangeGoods, and static xrefs never
found the real path. Watching the DATA found it in one run. Reach for this
whenever "who changes this field?" is the question.

Up to 4 addresses (four debug registers). Data breakpoints are TRAPS, so EIP
points just AFTER the storing instruction -- subtract a few bytes when looking
it up in Ghidra. The stack is reported as a raw scan of plausible .text return
addresses, because this binary does not keep EBP frame pointers in the hot code.

THE WOW64 TRAP that cost two game sessions: a 32-bit process delivers debug
exceptions to a 64-bit debugger as STATUS_WX86_BREAKPOINT (0x4000001F) and
STATUS_WX86_SINGLE_STEP (0x4000001E), NOT the 64-bit EXCEPTION_* codes. Match
only the latter and your own trap falls through as unhandled -- Windows then
kills the game. Use Wow64Get/SetThreadContext too, and
DebugSetProcessKillOnExit(FALSE) so the game survives if this script dies.
"""
import ctypes as C
import ctypes.wintypes as W
import sys, struct, time, collections

sys.path.insert(0, "tools")
from annomem import Mem, find_islands, settlement_name
import annoprod as ap

_argv = sys.argv[1:]
MAX_HITS, TIMEOUT_S = 20, 45.0
if "--hits" in _argv:
    i = _argv.index("--hits"); MAX_HITS = int(_argv[i + 1]); del _argv[i:i + 2]
if "--secs" in _argv:
    i = _argv.index("--secs"); TIMEOUT_S = float(_argv[i + 1]); del _argv[i:i + 2]
STOCK_MODE = "--stock" in _argv
if STOCK_MODE:
    _argv.remove("--stock")
def _is_addr(a):
    try:
        int(a, 0)
        return True
    except ValueError:
        return False


EXPLICIT = [int(a, 0) for a in _argv if _is_addr(a)]
WATCH_WARE_NAMES = [a for a in _argv if not _is_addr(a)]

k32 = C.WinDLL("kernel32", use_last_error=True)

DBG_CONTINUE = 0x00010002
DBG_EXCEPTION_NOT_HANDLED = 0x80010001
EXCEPTION_DEBUG_EVENT = 1
CREATE_THREAD_DEBUG_EVENT = 2
EXIT_PROCESS_DEBUG_EVENT = 5
BREAKPOINT_CODES = (0x80000003, 0x4000001F)
SINGLE_STEP_CODES = (0x80000004, 0x4000001E)
PROCESS_ALL_ACCESS = 0x1F0FFF
THREAD_ALL_ACCESS = 0x1F03FF
WOW64_CONTEXT_FULL = 0x00010007
WOW64_CONTEXT_DEBUG_REGISTERS = 0x00010010
RESUME_FLAG = 0x10000
TH32CS_SNAPTHREAD = 0x00000004
EVENT_NAMES = {1: "EXCEPTION", 2: "CREATE_THREAD", 3: "CREATE_PROCESS",
               4: "EXIT_THREAD", 5: "EXIT_PROCESS", 6: "LOAD_DLL",
               7: "UNLOAD_DLL", 8: "OUTPUT_DEBUG_STRING", 9: "RIP"}
TEXT_LO, TEXT_HI = 0x00401000, 0x01000000


class EXCEPTION_RECORD64(C.Structure):
    _fields_ = [("ExceptionCode", W.DWORD), ("ExceptionFlags", W.DWORD),
                ("ExceptionRecord", C.c_ulonglong),
                ("ExceptionAddress", C.c_ulonglong),
                ("NumberParameters", W.DWORD), ("__pad", W.DWORD),
                ("ExceptionInformation", C.c_ulonglong * 15)]


class EXCEPTION_DEBUG_INFO(C.Structure):
    _fields_ = [("ExceptionRecord", EXCEPTION_RECORD64), ("dwFirstChance", W.DWORD)]


class CREATE_THREAD_DEBUG_INFO(C.Structure):
    _fields_ = [("hThread", C.c_ulonglong), ("lpThreadLocalBase", C.c_ulonglong),
                ("lpStartAddress", C.c_ulonglong)]


class EXIT_PROCESS_DEBUG_INFO(C.Structure):
    _fields_ = [("dwExitCode", W.DWORD)]


class DEBUG_EVENT_U(C.Union):
    _fields_ = [("Exception", EXCEPTION_DEBUG_INFO),
                ("CreateThread", CREATE_THREAD_DEBUG_INFO),
                ("ExitProcess", EXIT_PROCESS_DEBUG_INFO),
                ("pad", C.c_ubyte * 176)]


class DEBUG_EVENT(C.Structure):
    _fields_ = [("dwDebugEventCode", W.DWORD), ("dwProcessId", W.DWORD),
                ("dwThreadId", W.DWORD), ("u", DEBUG_EVENT_U)]


class WOW64_FLOATING_SAVE_AREA(C.Structure):
    _fields_ = [("ControlWord", W.DWORD), ("StatusWord", W.DWORD),
                ("TagWord", W.DWORD), ("ErrorOffset", W.DWORD),
                ("ErrorSelector", W.DWORD), ("DataOffset", W.DWORD),
                ("DataSelector", W.DWORD), ("RegisterArea", C.c_ubyte * 80),
                ("Cr0NpxState", W.DWORD)]


class WOW64_CONTEXT(C.Structure):
    _fields_ = [("ContextFlags", W.DWORD),
                ("Dr0", W.DWORD), ("Dr1", W.DWORD), ("Dr2", W.DWORD),
                ("Dr3", W.DWORD), ("Dr6", W.DWORD), ("Dr7", W.DWORD),
                ("FloatSave", WOW64_FLOATING_SAVE_AREA),
                ("SegGs", W.DWORD), ("SegFs", W.DWORD), ("SegEs", W.DWORD),
                ("SegDs", W.DWORD), ("Edi", W.DWORD), ("Esi", W.DWORD),
                ("Ebx", W.DWORD), ("Edx", W.DWORD), ("Ecx", W.DWORD),
                ("Eax", W.DWORD), ("Ebp", W.DWORD), ("Eip", W.DWORD),
                ("SegCs", W.DWORD), ("EFlags", W.DWORD), ("Esp", W.DWORD),
                ("SegSs", W.DWORD), ("ExtendedRegisters", C.c_ubyte * 512)]


class THREADENTRY32(C.Structure):
    _fields_ = [("dwSize", W.DWORD), ("cntUsage", W.DWORD),
                ("th32ThreadID", W.DWORD), ("th32OwnerProcessID", W.DWORD),
                ("tpBasePri", C.c_long), ("tpDeltaPri", C.c_long),
                ("dwFlags", W.DWORD)]


k32.OpenProcess.restype = W.HANDLE
k32.OpenThread.restype = W.HANDLE
k32.CreateToolhelp32Snapshot.restype = W.HANDLE

# ---- locate the node to watch, using the read-only inspector -------------
m = Mem()
wares = ap.name_table(m, ap.WARE_NAMES, 68)
islands = dict(find_islands(m))
watch = []          # (label, address of node value field, current value)
if EXPLICIT and not STOCK_MODE:
    for a in EXPLICIT:
        watch.append((f"0x{a:08x}", a, m.u32(a)))
for iid, island in islands.items():
    if watch and not STOCK_MODE:
        break
    for slot in range(8):
        wh = m.u32(island + ap.ISLAND_WAREHOUSES + slot * 4)
        if not wh or wh < 0x01400000 or (m.u32(wh + ap.WH_STORAGE) or 0) <= 999:
            continue
        name = settlement_name(m, island, slot) or f"i{iid}"
        hdr = wh + ap.WH_PRODUCTS_MAP
        # walk the rb-tree collecting node addresses, not just values
        nodes = {}

        def rec(n, d=0):
            if not n or d > 48:
                return
            rec(m.u32(n + 8), d + 1)
            nodes[m.u32(n + 0x10)] = n
            rec(m.u32(n + 0xC), d + 1)

        rec(m.u32(hdr + 4))
        want = [wares.index(n) for n in WATCH_WARE_NAMES if n in wares] or [8, 13]
        for w in want:
            if w in nodes:
                watch.append((f"{name}/{wares[w]}", nodes[w] + 0x14,
                              m.u32(nodes[w] + 0x14)))
if not watch:
    sys.exit("nothing to watch - pass addresses, or --stock WareName ...")
watch = watch[:4]           # only 4 debug registers
print("watching:")
for label, addr, val in watch:
    print(f"  {label:28s} node value @ 0x{addr:08x} = {val}")

pid = find_pid = None
sys.path.insert(0, "tools")
from annomem import find_pid as _fp
pid = _fp("Addon.exe")
if isinstance(pid, (list, tuple)):
    pid = pid[0] if pid else None
pid = int(pid)
print(f"pid {pid}")

hproc = k32.OpenProcess(PROCESS_ALL_ACCESS, False, pid)


def read(addr, n):
    buf = (C.c_ubyte * n)()
    got = C.c_size_t(0)
    if not k32.ReadProcessMemory(hproc, C.c_void_p(addr), buf, n, C.byref(got)):
        return None
    return bytes(buf[:got.value])


def u32(a):
    b = read(a, 4)
    return struct.unpack("<I", b)[0] if b and len(b) == 4 else None


def thread_ids():
    snap = k32.CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)
    out = []
    te = THREADENTRY32()
    te.dwSize = C.sizeof(te)
    if k32.Thread32First(snap, C.byref(te)):
        while True:
            if te.th32OwnerProcessID == pid:
                out.append(te.th32ThreadID)
            if not k32.Thread32Next(snap, C.byref(te)):
                break
    k32.CloseHandle(snap)
    return out


def set_drs(ht):
    ctx = WOW64_CONTEXT()
    ctx.ContextFlags = WOW64_CONTEXT_DEBUG_REGISTERS
    if not k32.Wow64GetThreadContext(ht, C.byref(ctx)):
        return False
    dr7 = 0
    for i, (label, addr, val) in enumerate(watch):
        setattr(ctx, f"Dr{i}", addr)
        dr7 |= (1 << (i * 2))                       # Ln enable
        dr7 |= (0b01 << (16 + i * 4))               # RWn = write
        dr7 |= (0b11 << (18 + i * 4))               # LENn = 4 bytes
    ctx.Dr7 = dr7
    ctx.Dr6 = 0
    ctx.ContextFlags = WOW64_CONTEXT_DEBUG_REGISTERS
    return bool(k32.Wow64SetThreadContext(ht, C.byref(ctx)))


def arm_tid(tid):
    ht = k32.OpenThread(THREAD_ALL_ACCESS, False, tid)
    if not ht:
        return False
    ok = set_drs(ht)
    k32.CloseHandle(ht)
    return ok


def disarm_all():
    for tid in thread_ids():
        ht = k32.OpenThread(THREAD_ALL_ACCESS, False, tid)
        if not ht:
            continue
        ctx = WOW64_CONTEXT()
        ctx.ContextFlags = WOW64_CONTEXT_DEBUG_REGISTERS
        if k32.Wow64GetThreadContext(ht, C.byref(ctx)):
            ctx.Dr0 = ctx.Dr1 = ctx.Dr2 = ctx.Dr3 = 0
            ctx.Dr6 = ctx.Dr7 = 0
            ctx.ContextFlags = WOW64_CONTEXT_DEBUG_REGISTERS
            k32.Wow64SetThreadContext(ht, C.byref(ctx))
        k32.CloseHandle(ht)


if not k32.DebugActiveProcess(pid):
    sys.exit(f"DebugActiveProcess failed: {C.get_last_error()}")
k32.DebugSetProcessKillOnExit(False)
print("attached")

hits = []
seen = collections.Counter()
armed = False
start = time.time()
evt = DEBUG_EVENT()
exited = False
try:
    while len(hits) < MAX_HITS and time.time() - start < TIMEOUT_S:
        if not k32.WaitForDebugEvent(C.byref(evt), 500):
            continue
        code = evt.dwDebugEventCode
        seen[EVENT_NAMES.get(code, str(code))] += 1
        status = DBG_CONTINUE
        if code == EXCEPTION_DEBUG_EVENT:
            rec = evt.u.Exception.ExceptionRecord
            ecode, eaddr = rec.ExceptionCode, rec.ExceptionAddress
            if ecode in BREAKPOINT_CODES and not armed:
                tids = thread_ids()
                n = sum(1 for t in tids if arm_tid(t))
                armed = True
                print(f"data breakpoints armed on {n}/{len(tids)} threads")
            elif ecode in SINGLE_STEP_CODES:
                ht = k32.OpenThread(THREAD_ALL_ACCESS, False, evt.dwThreadId)
                ctx = WOW64_CONTEXT()
                ctx.ContextFlags = WOW64_CONTEXT_FULL | WOW64_CONTEXT_DEBUG_REGISTERS
                k32.Wow64GetThreadContext(ht, C.byref(ctx))
                which = [i for i in range(4) if ctx.Dr6 & (1 << i)]
                stack = read(ctx.Esp, 0x120) or b""
                rets = []
                for off in range(0, len(stack) - 4, 4):
                    v = struct.unpack_from("<I", stack, off)[0]
                    if TEXT_LO <= v < TEXT_HI:
                        rets.append((off, v))
                hits.append({"eip": ctx.Eip, "dr6": ctx.Dr6, "which": which,
                             "esp": ctx.Esp, "eax": ctx.Eax, "ecx": ctx.Ecx,
                             "edx": ctx.Edx, "esi": ctx.Esi, "edi": ctx.Edi,
                             "vals": [u32(a) for _, a, _ in watch],
                             "rets": rets[:10], "tid": evt.dwThreadId})
                ctx.Dr6 = 0
                ctx.ContextFlags = WOW64_CONTEXT_DEBUG_REGISTERS
                k32.Wow64SetThreadContext(ht, C.byref(ctx))
                k32.CloseHandle(ht)
            elif ecode in BREAKPOINT_CODES:
                pass
            else:
                status = DBG_EXCEPTION_NOT_HANDLED
        elif code == CREATE_THREAD_DEBUG_EVENT and armed:
            ht = W.HANDLE(evt.u.CreateThread.hThread)
            set_drs(ht)
        elif code == EXIT_PROCESS_DEBUG_EVENT:
            ec = evt.u.ExitProcess.dwExitCode
            print(f"TARGET EXITED, exit code 0x{ec & 0xffffffff:08x}")
            exited = True
            k32.ContinueDebugEvent(evt.dwProcessId, evt.dwThreadId, status)
            break
        k32.ContinueDebugEvent(evt.dwProcessId, evt.dwThreadId, status)
finally:
    if not exited:
        disarm_all()
        k32.DebugActiveProcessStop(pid)
        print("disarmed and detached")

print("\nevents:", dict(seen))
print(f"{len(hits)} hits\n")
for h in hits:
    hit_labels = ", ".join(watch[i][0] for i in h["which"] if i < len(watch))
    print(f"  EIP=0x{h['eip']:08x}  Dr6={h['dr6']:x} [{hit_labels}]  "
          f"vals={h['vals']}")
    print(f"      eax=0x{h['eax']:08x} ecx=0x{h['ecx']:08x} edx=0x{h['edx']:08x} "
          f"esi=0x{h['esi']:08x} edi=0x{h['edi']:08x}")
    print("      stack: " + " ".join(f"+{o:02x}:0x{v:08x}" for o, v in h["rets"]))
if hits:
    print("\nwriting instructions (EIP is just AFTER the store):")
    for eip, n in collections.Counter(h["eip"] for h in hits).most_common():
        print(f"  0x{eip:08x}  x{n}")
