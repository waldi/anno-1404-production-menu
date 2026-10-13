// Experimental History64 native production-panel bridge.
// Static bindings are recorded in Ghidra. Whole-panel behavior is UNVERIFIED.
// Remains inert unless the explicit ANNORE_HISTORY_NATIVE.flag exists beside it.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#define ANNORE_HISTORY64
#include <windows.h>
#include <tlhelp32.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <string>
#include <vector>
#include <set>
#include <algorithm>
#include "game_identity.h"
#include "ui_protocol.h"
static_assert(sizeof(void*) == 8, "History bridge requires AMD64");

// Same exports as the existing Gold proxy, forwarded to the AMD64 system DLL.
#define FWD(n) __pragma(comment(linker, "/export:" #n "=C:\\Windows\\System32\\d3d9." #n))
FWD(Direct3DCreate9) FWD(Direct3DCreate9Ex) FWD(D3DPERF_BeginEvent)
FWD(D3DPERF_EndEvent) FWD(D3DPERF_GetStatus) FWD(D3DPERF_QueryRepeatFrame)
FWD(D3DPERF_SetMarker) FWD(D3DPERF_SetOptions) FWD(D3DPERF_SetRegion)
FWD(DebugSetLevel) FWD(DebugSetMute) FWD(Direct3DShaderValidatorCreate9)

static uintptr_t image;
static uint8_t* legacy;
static UiShared* ui;
static HANDLE done;
static volatile LONG busy;
static bool faulted;
static void* observedDialog;
static bool dialogVisible;
using Dispatch = char (*)(void*, void*, void*, char);
static Dispatch original;

template<class T> static T& field(void* p, size_t offset) { return *(T*)((uint8_t*)p + offset); }
template<class T> static T function(uintptr_t rva) { return (T)(image + rva); }
static void diagnostic(const char* message) { OutputDebugStringA(message); }
static void* manager() { return *(void**)(image + 0x2097088); }
static void* registry() { return (void*)(image + 0x207b310); }
static int player() { void* e = *(void**)(image + 0x207bab0); return e ? field<int>(e, 0x134) : -1; }
static int current_island() { void* e = *(void**)(image + 0x207bab0); return e ? field<int>(e, 0x12c) : 0; }

// Borrowed, fully-sized CRD string arguments. Native text setters copy them;
// no game's allocator frees these source buffers. All 48 bytes are present.
struct NativeString {
    union { char small[16]; const void* pointer; } storage;
    uint64_t length, capacity, extra;
    uint32_t flags, padding;
    explicit NativeString(const char* text) : storage{}, length(strlen(text)), capacity(length <= 15 ? 15 : length), extra(0), flags(0), padding(0) {
        if (length <= 15) memcpy(storage.small, text, (size_t)length + 1); else storage.pointer = text;
    }
    explicit NativeString(const wchar_t* text) : storage{}, length(wcslen(text)), capacity(length <= 7 ? 7 : length), extra(0), flags(0), padding(0) {
        if (length <= 7) memcpy(storage.small, text, ((size_t)length + 1) * 2); else storage.pointer = text;
    }
};
static_assert(sizeof(NativeString) == 48, "History CRD string argument size");
static const char* event_text(void* p) {
    if (!p || field<uint64_t>(p, 0x10) > 4096) return "";
    return field<uint64_t>(p, 0x18) > 15 ? field<const char*>(p, 0) : (const char*)p;
}
static std::string key_text(void* p) {
    uint64_t length = field<uint64_t>(p, 0x10), capacity = field<uint64_t>(p, 0x18);
    if (length > 4096 || capacity < length) return {};
    const char* text = capacity > 15 ? field<const char*>(p, 0) : (const char*)p;
    return text ? std::string(text, (size_t)length) : std::string();
}
static int ascii_compare(const std::string& key, const char* text) {
    size_t i = 0;
    for (; i < key.size() && text[i]; ++i) {
        unsigned char a = key[i], b = (unsigned char)text[i];
        if (a >= 'a' && a <= 'z') a -= 32;
        if (b >= 'a' && b <= 'z') b -= 32;
        if (a != b) return a < b ? -1 : 1;
    }
    return i < key.size() ? 1 : text[i] ? -1 : 0;
}
static void* find_path(const char* path) {
    void* head = field<void*>(registry(), 0x1d8);
    if (!head) return NULL;
    void* node = field<void*>(head, 8);
    for (int depth = 0; node && node != head && depth < 64; ++depth) {
        if (field<uint8_t>(node, 0x19)) return NULL;
        int compare = ascii_compare(key_text((uint8_t*)node + 0x20), path);
        if (!compare) return field<void*>(node, 0x50);
        node = field<void*>(node, compare < 0 ? 0x10 : 0);
    }
    return NULL;
}
static std::vector<void*> children(void* widget) {
    void** begin = field<void**>(widget, 0xb8), **end = field<void**>(widget, 0xc0);
    uintptr_t bytes = (uintptr_t)end - (uintptr_t)begin;
    if (!begin || end < begin || bytes % 8 || bytes > 4096 * 8) return {};
    return std::vector<void*>(begin, end);
}
static void rebuild_paths() { function<void (*)(void*)>(0x620c50)(registry()); }
static void register_tree(void* widget, int depth, std::set<void*>& seen) {
    if (!widget || depth > 32 || seen.size() >= 4096 || !seen.insert(widget).second) return;
    function<void (*)(void*)>(0x62aa20)(widget);
    for (void* child : children(widget)) register_tree(child, depth + 1, seen);
}
static bool install_xml(void* parent, const char* filename) {
    void* mgr = manager(), *context = mgr ? field<void*>(mgr, 0x5a8) : NULL;
    if (!context) return false;
    // Build using the native empty-document constructor and native node pool.
    // Nodes remain pool-owned for the lifetime of the game GUI context.
    alignas(8) uint8_t doc[48] = {};
    function<void (*)(void*, void*)>(0x18f9f0)(doc, context);
    // The native file parser nests the widgets beneath its leading comment node.
    // Use an independent pool-owned tree and omit comments instead.
    void* root = field<void*>(doc, 8);
    if (!root) return false;
    {
        FILE* file = NULL;
        if (fopen_s(&file, filename, "rb") || !file) return false;
        fseek(file, 0, SEEK_END); long size = ftell(file); rewind(file);
        if (size <= 0 || size > 256 * 1024) { fclose(file); return false; }
        std::string xml((size_t)size, '\0');
        size_t got = fread(&xml[0], 1, xml.size(), file); fclose(file);
        if (got != xml.size()) return false;
        std::vector<void*> stack{root}; std::vector<std::string> names{std::string()};
        size_t pos = 0, textStart = 0; int nodes = 0;
        auto assign = function<void (*)(void*, const char*, size_t)>(0x05fc70);
        while (pos < xml.size()) {
            if (xml[pos] != '<') { ++pos; continue; }
            if (pos > textStart && stack.size() > 1) {
                std::string value = xml.substr(textStart, pos - textStart);
                size_t first = value.find_first_not_of(" \r\n\t"), last = value.find_last_not_of(" \r\n\t");
                if (first != std::string::npos) {
                    value = value.substr(first, last - first + 1);
                    for (const auto& entity : {std::pair<const char*, const char*>("&lt;", "<"), {"&gt;", ">"}, {"&amp;", "&"}}) {
                        size_t at = 0;
                        while ((at = value.find(entity.first, at)) != std::string::npos) {
                            value.replace(at, strlen(entity.first), entity.second); at += strlen(entity.second);
                        }
                    }
                    assign((uint8_t*)stack.back() + 0x68, value.c_str(), value.size());
                }
            }
            if (xml.compare(pos, 4, "<!--") == 0 || xml.compare(pos, 2, "<?") == 0) {
                bool comment = xml.compare(pos, 4, "<!--") == 0;
                size_t end = xml.find(comment ? "-->" : "?>", pos + 2);
                if (end == std::string::npos) return false;
                pos = end + (comment ? 3 : 2); textStart = pos; continue;
            }
            size_t end = xml.find('>', pos + 1);
            if (end == std::string::npos) return false;
            bool closing = xml[pos + 1] == '/', single = xml[end - 1] == '/';
            std::string name = xml.substr(pos + (closing ? 2 : 1), end - pos - (closing ? 2 : 1) - (single ? 1 : 0));
            size_t last = name.find_last_not_of(" \t\r\n");
            if (last == std::string::npos) return false;
            name.resize(last + 1);
            if (name.size() > 63 || name.find_first_of(" \t\r\n=\"'") != std::string::npos) return false;
            if (closing) {
                if (stack.size() <= 1 || names.back() != name) return false;
                stack.pop_back(); names.pop_back();
            } else {
                if (stack.size() >= 32 || ++nodes > 20000) return false;
                void* node = function<void* (*)(void*)>(0x0f96f0)((uint8_t*)context + 0x128);
                if (!node) return false;
                function<void (*)(void*, const char*, void*)>(0x0f9990)(node, name.c_str(), (uint8_t*)context + 0x120);
                assign((uint8_t*)node + 0x38, name.c_str(), name.size());
                function<void (*)(void*, void*)>(0x0f9680)(stack.back(), node);
                if (!single) { stack.push_back(node); names.push_back(name); }
            }
            pos = end + 1; textStart = pos;
        }
        if (stack.size() != 1 || !nodes) return false;
    }
    void* firstList = field<void*>(root, 0x10);
    void* first = firstList ? field<void*>(firstList, 0) : NULL;
    if (!first) return false;
    field<void*>(doc, 0x10) = first;
    auto before = children(parent);
    function<void (*)(void*, void*, void*, int)>(0x679bb0)(parent, parent, doc, -1);
    bool added = false; std::set<void*> seen;
    for (void* child : children(parent)) {
        if (std::find(before.begin(), before.end(), child) == before.end()) { register_tree(child, 0, seen); added = true; }
    }
    if (added) rebuild_paths();
    return added;
}
static uintptr_t text_setting(void* widget) {
    int type = field<int>(widget, 0x118);
    return type == 1 ? 0x988 : type == 2 ? 0xb60 : 0;
}
static bool set_text(void* widget, const wchar_t* text) {
    uintptr_t offset = text_setting(widget);
    if (!offset) return false;
    NativeString argument(text);
    void* setting = (uint8_t*)widget + offset;
    function<void (*)(void*, void*, char)>(0x6480d0)(setting, &argument, 1);
    if (field<int>(widget, 0x118) == 2) {
        function<void (*)(void*, int)>(0x6485d0)(setting, 1);
        function<void (*)(void*, int)>(0x648640)(setting, 4);
    }
    return true;
}
static void set_visible(void* widget, int visible) {
    uintptr_t* vt = field<uintptr_t*>(widget, 0);
    ((void (*)(void*, int, char, char))vt[0x158 / 8])(widget, visible, 1, 0);
}
static void click(void* widget, const char* event) {
    NativeString argument(event && *event ? event : "OnLeftClick");
    // Confirmed OnRender argument block is {0,widget,result}; unlike Gold's
    // four dwords, History passes three pointer-sized entries.
    uintptr_t args[3] = {0, (uintptr_t)widget, 0};
    original(widget, &argument, args, 1);
    if (args[2]) {
        uintptr_t* vt = *(uintptr_t**)args[2];
        ((void (*)(void*, int))vt[1])((void*)args[2], 1);
    }
}
static void publish(int row, int island, int owner, uint32_t control = 0) {
    if (!ui) return;
    uint32_t sequence = (uint32_t)ui->clickWrite;
    ui->clicks[sequence % UI_CLICK_CAPACITY] = {row, control, island, owner, GetTickCount()};
    MemoryBarrier(); InterlockedIncrement(&ui->clickWrite);
}
static void observe_dialog() {
    void* widget = find_path("SYSTEMMESSAGES/ANNORE_PROD5_DIALOG");
    if (!widget || !ui) return;
    if (widget != observedDialog) { observedDialog = widget; dialogVisible = false; }
    void* parent = field<void*>(widget, 0xd0);
    if (parent) {
        int* rect = (int*)((uint8_t*)parent + 0x2f8);
        if (rect[2] > rect[0] && rect[3] > rect[1]) {
            InterlockedExchange(&ui->viewportWidth, rect[2] - rect[0]);
            InterlockedExchange(&ui->viewportHeight, rect[3] - rect[1]);
        }
    }
    bool visible = field<int>(widget, 0xe8) == 1;
    if (visible != dialogVisible) { dialogVisible = visible; publish(visible ? UI_DIALOG_OPEN : UI_DIALOG_CLOSE, current_island(), player()); }
}
static void legacy_op() {
    if (!legacy) return;
    LONG operation = InterlockedCompareExchange((volatile LONG*)(legacy + 648), 0, 0);
    if (!operation) return;
    legacy[656 + 63] = 0; legacy[720 + 259] = 0; field<uint16_t>(legacy, 980 + 254) = 0;
    const char* target = (char*)legacy + 656, *arg = (char*)legacy + 720;
    bool success = false;
    if (operation == 11) { rebuild_paths(); success = true; }
    else {
        void* widget = find_path(target);
        if (!widget) { InterlockedExchange((volatile LONG*)(legacy + 652), 5); return; }
        if (operation == 10) success = install_xml(widget, arg);
        else if (operation == 2) success = set_text(widget, (wchar_t*)(legacy + 980));
        else if (operation == 4) { set_visible(widget, *arg == '1' ? 1 : 0); success = true; }
        else if (operation == 9) { click(widget, arg); success = true; }
    }
    field<uint32_t>(legacy, 1236) = success ? 1 : 0;
    InterlockedExchange((volatile LONG*)(legacy + 648), 0);
    MemoryBarrier(); InterlockedExchange((volatile LONG*)(legacy + 652), success ? 2 : 3);
}
static void ui_batch() {
    if (!ui || InterlockedCompareExchange(&ui->state, 2, 1) != 1) return;
    uint32_t result = 0;
    if (!find_path("SYSTEMMESSAGES/ANNORE_PROD5_DIALOG")) result |= UI_MISSING_DIALOG;
    if (!find_path("INFOBAR/ANNORE_PROD5_BUTTON")) result |= UI_MISSING_BUTTON;
    if (ui->count > UI_CAPACITY) result |= UI_FAILED_UPDATE;
    void* widgets[UI_CAPACITY] = {};
    for (uint32_t i = 0; i < ui->count && i < UI_CAPACITY; ++i) {
        UiUpdate& update = ui->updates[i]; update.target[63] = 0; update.text[127] = 0;
        void* widget = widgets[i] = find_path(update.target);
        if (!widget) { result |= UI_FAILED_UPDATE; continue; }
        int type = field<int>(widget, 0x118);
        if (update.flags & ~(UI_TEXT | UI_VISIBLE | UI_CHECKED | UI_CHECKED_ON | UI_RECT | UI_BAR | UI_ICON)) result |= UI_FAILED_UPDATE;
        if ((update.flags & UI_CHECKED_ON) && !(update.flags & UI_CHECKED)) result |= UI_FAILED_UPDATE;
        if ((update.flags & UI_CHECKED) && type != 2) result |= UI_FAILED_UPDATE;
        if ((update.flags & UI_TEXT) && !text_setting(widget)) result |= UI_FAILED_UPDATE;
        if ((update.flags & UI_ICON) && type != 1) result |= UI_FAILED_UPDATE;
        if ((update.flags & UI_BAR) && (type != 9 || update.barMax <= 1 || update.barValue < 0 || update.barValue > update.barMax)) result |= UI_FAILED_UPDATE;
        if ((update.flags & UI_RECT) && (update.rect[2] <= update.rect[0] || update.rect[3] <= update.rect[1])) result |= UI_FAILED_UPDATE;
    }
    for (uint32_t i = 0; !result && i < ui->count; ++i) {
        UiUpdate& update = ui->updates[i]; void* widget = widgets[i];
        if (update.flags & UI_RECT) {
            // Exact definition-reload mechanism: raw extent then stale alignment
            // stamp. UNVERIFIED whole-panel live geometry; no arbitrary offsets.
            memcpy((uint8_t*)widget + 0x2f8, update.rect, 16);
            field<int>(widget, 8) = *(int*)(image + 0x2097090) - 1;
        }
        if (update.flags & UI_BAR) { field<int>(widget, 0x7a8) = update.barValue; field<int>(widget, 0x7ac) = update.barMax; }
        if (update.flags & UI_ICON) field<int>(widget, 0x934) = update.icon;
        if (update.flags & UI_TEXT) set_text(widget, (wchar_t*)update.text);
        if (update.flags & UI_VISIBLE) set_visible(widget, update.visible ? 1 : 0);
        if (update.flags & UI_CHECKED) {
            field<uint8_t>(widget, 0xb50) = (update.flags & UI_CHECKED_ON) ? 1 : 0;
            function<void (*)(void*)>(0x645be0)(widget);
        }
    }
    if (!result) { ui->displayedPlayer = ui->player; memcpy(ui->displayedIslands, ui->rowIslands, sizeof(ui->rowIslands)); }
    ui->result = result; MemoryBarrier(); InterlockedExchange(&ui->state, 0); SetEvent(done);
}
static void capture_click(void* widget, const char* name) {
    if (widget && name && !strcmp(name, "OnLeftClick")) {
        const char* access = (char*)widget + 0x15d;
        // The shipped sources have exactly these ASCII IDs. Bound the read.
        if (memchr(access, 0, 65) && !strncmp(access, "ANNORE_ISLAND_ROW", 17)) {
            int row = atoi(access + 17);
            if (row >= 1 && row <= 32) publish(row, row <= 19 ? ui->displayedIslands[row - 1] : 0, ui->displayedPlayer, GetAsyncKeyState(VK_CONTROL) & 0x8000 ? 1 : 0);
        }
        if (!strcmp(access, "ANNORE_PREVIOUS") || !strcmp(access, "ANNORE_NEXT")) publish(!strcmp(access, "ANNORE_NEXT") ? UI_NEXT : UI_PREVIOUS, 0, ui->displayedPlayer);
    }
}
static void event_before() {
    legacy_op(); ui_batch();
}
static bool on_gui_thread() {
    void* mgr = manager();
    return mgr && field<DWORD>(mgr, 0x5d0) == GetCurrentThreadId();
}
static char hook(void* widget, void* event, void* args, char fallback) {
    const char* name = NULL;
    __try {
        if (!faulted && ui) {
            name = event_text(event);
            // Input dispatch may run outside the GUI thread. Capturing a small
            // click record only touches the shared queue; native widget work
            // below remains restricted to the GUI thread.
            capture_click(widget, name);
        }
    } __except (EXCEPTION_EXECUTE_HANDLER) { faulted = true; }
    bool work = false;
    __try { work = !faulted && ui && on_gui_thread() && InterlockedCompareExchange(&busy, 1, 0) == 0; }
    __except (EXCEPTION_EXECUTE_HANDLER) { faulted = true; }
    if (work) {
        __try { event_before(); }
        __except (EXCEPTION_EXECUTE_HANDLER) {
            faulted = true;
            if (legacy) { InterlockedExchange((volatile LONG*)(legacy + 648), 0); InterlockedExchange((volatile LONG*)(legacy + 652), 3); }
            if (ui) { ui->result = UI_FAILED_UPDATE; InterlockedExchange(&ui->state, 0); SetEvent(done); }
            diagnostic("AnnoRE History: native bridge exception; custom processing disabled\n");
        }
    }
    char result = original(widget, event, args, fallback);
    if (work) {
        __try { if (!faulted) observe_dialog(); }
        __except (EXCEPTION_EXECUTE_HANDLER) { faulted = true; }
        InterlockedExchange(&busy, 0);
    }
    return result;
}
static void jump(uint8_t* output, const void* target) {
    output[0] = 0xff; output[1] = 0x25; memset(output + 2, 0, 4);
    uintptr_t address = (uintptr_t)target; memcpy(output + 6, &address, 8);
}
struct SuspendedThreads {
    std::vector<HANDLE> handles;
    SuspendedThreads() { handles.reserve(512); }
    ~SuspendedThreads() { for (HANDLE thread : handles) { ResumeThread(thread); CloseHandle(thread); } }
    bool suspend(uintptr_t first, uintptr_t last) {
        HANDLE snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if (snapshot == INVALID_HANDLE_VALUE) return false;
        THREADENTRY32 entry = {}; entry.dwSize = sizeof(entry); bool ok = true;
        if (Thread32First(snapshot, &entry)) do {
            if (entry.th32OwnerProcessID != GetCurrentProcessId() || entry.th32ThreadID == GetCurrentThreadId()) continue;
            if (handles.size() == handles.capacity()) { ok = false; break; }
            HANDLE thread = OpenThread(THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT, FALSE, entry.th32ThreadID);
            if (!thread || SuspendThread(thread) == (DWORD)-1) { if (thread) CloseHandle(thread); ok = false; break; }
            handles.push_back(thread);
            CONTEXT context = {}; context.ContextFlags = CONTEXT_CONTROL;
            if (!GetThreadContext(thread, &context) || (context.Rip >= first && context.Rip < last)) { ok = false; break; }
        } while (Thread32Next(snapshot, &entry));
        CloseHandle(snapshot); return ok;
    }
};
static bool install_hook() {
    const uint8_t expected[18] = {0x48,0x8b,0xc4,0x44,0x88,0x48,0x20,0x4c,0x89,0x40,0x18,0x55,0x56,0x57,0x41,0x54,0x41,0x55};
    uint8_t* target = (uint8_t*)(image + 0x631b60);
    if (memcmp(target, expected, sizeof(expected))) return false;
    uint8_t* trampoline = (uint8_t*)VirtualAlloc(NULL, 64, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
    if (!trampoline) return false;
    memcpy(trampoline, expected, sizeof(expected)); jump(trampoline + sizeof(expected), target + sizeof(expected));
    DWORD old = 0;
    if (!VirtualProtect(trampoline, 64, PAGE_EXECUTE_READ, &old)) { VirtualFree(trampoline, 0, MEM_RELEASE); return false; }
    FlushInstructionCache(GetCurrentProcess(), trampoline, 64);
    original = (Dispatch)trampoline;
    bool installed = false;
    {
        SuspendedThreads threads;
        if (threads.suspend((uintptr_t)target, (uintptr_t)target + sizeof(expected)) && !memcmp(target, expected, sizeof(expected)) &&
            VirtualProtect(target, sizeof(expected), PAGE_EXECUTE_READWRITE, &old)) {
            jump(target, (void*)&hook); memset(target + 14, 0x90, 4);
            DWORD ignored = 0; VirtualProtect(target, sizeof(expected), old, &ignored);
            FlushInstructionCache(GetCurrentProcess(), target, sizeof(expected)); installed = true;
        }
    }
    if (!installed) { original = NULL; VirtualFree(trampoline, 0, MEM_RELEASE); }
    return installed;
}
static void launch_companion(HMODULE self) {
    wchar_t directory[32768], executable[32768], command[32768];
    if (!GetModuleFileNameW(self, directory, 32768)) return;
    wchar_t* slash = wcsrchr(directory, L'\\'); if (!slash) return; *slash = 0;
    if (_snwprintf_s(executable, 32768, _TRUNCATE, L"%s\\AnnoRECompanion.exe", directory) < 0 ||
        _snwprintf_s(command, 32768, _TRUNCATE, L"\"%s\" --parent-pid %lu", executable, GetCurrentProcessId()) < 0) return;
    STARTUPINFOW startup = {}; startup.cb = sizeof(startup); PROCESS_INFORMATION process = {};
    if (CreateProcessW(executable, command, NULL, NULL, FALSE, CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP, NULL, directory, &startup, &process)) {
        CloseHandle(process.hThread); CloseHandle(process.hProcess);
    }
}
static DWORD WINAPI initialize(void* parameter) {
    HMODULE self = (HMODULE)parameter;
    wchar_t path[32768]; if (!GetModuleFileNameW(NULL, path, 32768) || IdentifyAnnoFile(path) != AnnoEdition::History64) return 0;
    image = (uintptr_t)GetModuleHandleW(NULL);
    wchar_t marker[32768]; if (!GetModuleFileNameW(self, marker, 32768)) return 0;
    wchar_t* slash = wcsrchr(marker, L'\\'); if (!slash) return 0; *slash = 0;
    if (wcscat_s(marker, L"\\ANNORE_HISTORY_NATIVE.flag") || GetFileAttributesW(marker) == INVALID_FILE_ATTRIBUTES) return 0;
    HANDLE legacyMap = CreateFileMappingW(INVALID_HANDLE_VALUE, NULL, PAGE_READWRITE, 0, 4096, L"AnnoREHistoryBridge");
    if (!legacyMap || GetLastError() == ERROR_ALREADY_EXISTS) { if (legacyMap) CloseHandle(legacyMap); return 0; }
    legacy = (uint8_t*)MapViewOfFile(legacyMap, FILE_MAP_ALL_ACCESS, 0, 0, 4096);
    HANDLE uiMap = CreateFileMappingA(INVALID_HANDLE_VALUE, NULL, PAGE_READWRITE, 0, sizeof(UiShared), UI_MAP_NAME);
    if (!uiMap || GetLastError() == ERROR_ALREADY_EXISTS) { if (uiMap) CloseHandle(uiMap); return 0; }
    ui = (UiShared*)MapViewOfFile(uiMap, FILE_MAP_ALL_ACCESS, 0, 0, sizeof(UiShared));
    done = CreateEventA(NULL, FALSE, FALSE, UI_DONE_NAME);
    if (!legacy || !ui || !done) return 0;
    ZeroMemory(legacy, 4096); ZeroMemory(ui, sizeof(*ui));
    field<uint32_t>(legacy, 0) = 0x4f4e4e41; field<uint32_t>(legacy, 4) = 8;
    ui->version = UI_VERSION; ui->size = sizeof(*ui); ui->ownerPid = GetCurrentProcessId();
    MemoryBarrier(); ui->magic = UI_MAGIC;
    if (!install_hook()) { diagnostic("AnnoRE History: prologue/thread preflight refused native hook\n"); return 0; }
    launch_companion(self);
    return 0;
}
BOOL WINAPI DllMain(HMODULE module, DWORD reason, void*) {
    if (reason == DLL_PROCESS_ATTACH) {
        DisableThreadLibraryCalls(module);
        HANDLE thread = CreateThread(NULL, 0, initialize, module, 0, NULL);
        if (thread) CloseHandle(thread);
    }
    return TRUE;
}
