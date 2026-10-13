#pragma once
#include <tlhelp32.h>

// Runtime consumer of History findings recorded in Ghidra/findings.json.
// No target calls or writes. Native panel transport is handled separately.
static uintptr_t history_module_base(HANDLE process) {
    HANDLE snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, GetProcessId(process));
    if (snapshot == INVALID_HANDLE_VALUE) return 0;
    MODULEENTRY32W entry = {}; entry.dwSize = sizeof(entry);
    uintptr_t base = 0;
    if (Module32FirstW(snapshot, &entry)) {
        do {
            if (_wcsicmp(entry.szModule, L"Anno1404Addon.exe") == 0) {
                base = (uintptr_t)entry.modBaseAddr; break;
            }
        } while (Module32NextW(snapshot, &entry));
    }
    CloseHandle(snapshot);
    return base;
}

static bool verified_history_process(HANDLE process) {
    wchar_t path[32768]; DWORD length = sizeof(path) / sizeof(path[0]);
    if (!QueryFullProcessImageNameW(process, 0, path, &length) ||
        IdentifyAnnoFile(path) != AnnoEdition::History64) return false;
    uintptr_t base = history_module_base(process);
    uint8_t header[4096] = {}; SIZE_T got = 0;
    if (!base || !ReadProcessMemory(process, (void*)base, header, sizeof(header), &got) || got != sizeof(header)) return false;
    uint32_t pe = 0; memcpy(&pe, header + 60, 4);
    if (header[0] != 'M' || header[1] != 'Z' || pe > sizeof(header) - 26) return false;
    uint32_t signature = 0; uint16_t machine = 0, magic = 0;
    memcpy(&signature, header + pe, 4); memcpy(&machine, header + pe + 4, 2); memcpy(&magic, header + pe + 24, 2);
    char string[15] = {};
    return signature == 0x4550 && machine == 0x8664 && magic == 0x20b &&
        ReadProcessMemory(process, (void*)(base + 0x1903900), string, sizeof(string), &got) &&
        got == sizeof(string) && memcmp(string, "m_Productivity", sizeof(string)) == 0;
}

struct Region { uintptr_t base, size; };
static bool valid_address(uintptr_t p) { return p >= 0x10000 && p < 0x0000800000000000ull; }
static bool valid_heap_ptr(uintptr_t p) { return valid_address(p); }

class GameMemory {
    HANDLE handle;
    uintptr_t base;
public:
    explicit GameMemory(HANDLE h) : handle(h), base(history_module_base(h)) {}
    uintptr_t module() const { return base; }
    bool read(uintptr_t address, void* output, SIZE_T size) const {
        if (!valid_address(address) || size > 0x0000800000000000ull - address) return false;
        SIZE_T got = 0;
        return ReadProcessMemory(handle, (void*)address, output, size, &got) && got == size;
    }
    template<class T> bool get(uintptr_t address, T& output) const { return read(address, &output, sizeof(output)); }
    uintptr_t ptr(uintptr_t address) const { uint64_t v = 0; return get(address, v) ? (uintptr_t)v : 0; }
    uint32_t u32(uintptr_t address) const { uint32_t v = 0; return get(address, v) ? v : 0; }
    int32_t i32(uintptr_t address) const { int32_t v = 0; return get(address, v) ? v : 0; }
    std::vector<Region> regions() const {
        std::vector<Region> result; MEMORY_BASIC_INFORMATION info = {}; uintptr_t cursor = 0;
        while (VirtualQueryEx(handle, (void*)cursor, &info, sizeof(info))) {
            uintptr_t start = (uintptr_t)info.BaseAddress, end = start + info.RegionSize;
            if (end <= cursor) break;
            DWORD protection = info.Protect & 0xff;
            bool readable = protection == PAGE_READONLY || protection == PAGE_READWRITE || protection == PAGE_WRITECOPY ||
                protection == PAGE_EXECUTE_READ || protection == PAGE_EXECUTE_READWRITE || protection == PAGE_EXECUTE_WRITECOPY;
            if (info.State == MEM_COMMIT && info.Type == MEM_PRIVATE && readable && !(info.Protect & PAGE_GUARD)) {
                // Bounded chunks cover large committed regions too.
                for (uintptr_t p = start; p < end;) {
                    uintptr_t size = std::min<uintptr_t>(1024 * 1024, end - p);
                    result.push_back({p, size}); p += size;
                }
            }
            cursor = end;
        }
        return result;
    }
};

static uintptr_t game_world(const GameMemory& m) { return m.ptr(m.module() + 0x2097040); }
static uintptr_t game_engine(const GameMemory& m) { return m.ptr(m.module() + 0x207bab0); }
static int game_player(const GameMemory& m) { return m.i32(game_engine(m) + 0x134); }
static int game_current_island(const GameMemory& m) { return m.i32(game_engine(m) + 0x12c); }
static uintptr_t building_island(const GameMemory& m, uintptr_t p) { return m.ptr(p + 0x170); }
static int island_population(const GameMemory& m, uintptr_t p, int tier, int player) { return m.i32(p + 0x5470 + tier * 32 + player * 4); }
static int global_population(const GameMemory& m, uintptr_t world, int tier, int player) { return m.i32(world + 0xea04 + tier * 32 + player * 4); }

static std::map<int32_t, uintptr_t> find_islands(const GameMemory& m) {
    std::map<int32_t, uintptr_t> result;
    uintptr_t world = game_world(m), header = m.ptr(world + 0xecc0);
    uint64_t count = m.ptr(world + 0xecc8);
    if (!world || !header || count == 0 || count >= 4096 || m.ptr(world) != m.module() + 0x19432d0) return result;
    std::set<uintptr_t> seen; bool valid = true;
    std::function<void(uintptr_t, int)> walk = [&](uintptr_t node, int depth) {
        if (node == header) return;
        uint8_t raw[48] = {};
        if (!node || depth > 64 || seen.size() >= count || !seen.insert(node).second || !m.read(node, raw, sizeof(raw)) || raw[0x19]) { valid = false; return; }
        uint64_t left = 0, right = 0, island = 0; int32_t id = 0;
        memcpy(&left, raw, 8); memcpy(&right, raw + 0x10, 8); memcpy(&id, raw + 0x20, 4); memcpy(&island, raw + 0x28, 8);
        if (m.ptr(island) != m.module() + 0x1901d68 || m.i32(island + 0x67f8) != id || result.count(id)) { valid = false; return; }
        walk((uintptr_t)left, depth + 1); result[id] = (uintptr_t)island; walk((uintptr_t)right, depth + 1);
    };
    walk(m.ptr(header + 8), 0);
    if (!valid || result.size() != count || game_world(m) != world || m.ptr(world + 0xecc0) != header || m.ptr(world + 0xecc8) != count) result.clear();
    return result;
}

static std::string settlement_name(const GameMemory& m, uintptr_t island, int player) {
    if (player < 0 || player >= 8) return {};
    uintptr_t address = island + 0x6148 + player * 0x30;
    uint8_t header[32]; if (!m.read(address, header, sizeof(header))) return {};
    uint64_t length = 0, capacity = 0; memcpy(&length, header + 0x10, 8); memcpy(&capacity, header + 0x18, 8);
    if (!length || length > 1024 || capacity < length) return {};
    std::vector<wchar_t> characters((size_t)length);
    uintptr_t data = capacity > 7 ? m.ptr(address) : address;
    if (!m.read(data, characters.data(), length * 2)) return {};
    int bytes = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, characters.data(), (int)length, NULL, 0, NULL, NULL);
    if (bytes <= 0) return {};
    std::string result(bytes, 0);
    WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, characters.data(), (int)length, &result[0], bytes, NULL, NULL);
    return result;
}

static std::vector<std::string> good_names(const GameMemory& m) {
    std::vector<std::string> names(68);
    for (int i = 0; i < 68; ++i) {
        uintptr_t address = m.ptr(m.module() + 0x1f69c60 + i * 8); char name[64] = {};
        if (address && m.read(address, name, sizeof(name)) && memchr(name, 0, sizeof(name))) names[i] = name;
    }
    return names;
}

struct BuildingInfo { std::string product, type; int productivity = 0; bool paused = false; double rate = 0, current = 0; std::map<std::string, double> inputs, potentialInputs; };
static bool set_text(Bridge* b, const char* target, const wchar_t* text);
static void log_line(const char* text);

static uintptr_t production_property(const GameMemory& m, uintptr_t building) {
    for (uintptr_t slot : {0x98u, 0xa0u}) {
        uintptr_t property = m.ptr(building + slot), vt = m.ptr(property);
        if (property && m.ptr(property + 8) == building &&
            (vt == m.module() + 0x1902440 || vt == m.module() + 0x1902358 || vt == m.module() + 0x19039d8)) return property;
    }
    return 0;
}

static std::vector<uintptr_t> find_buildings(const GameMemory& m, const std::map<int32_t, uintptr_t>& islands, int32_t wantedIsland, int wantedPlayer) {
    std::vector<uintptr_t> result; std::set<uintptr_t> allowed;
    for (const auto& island : islands) if (wantedIsland < 0 || island.first == wantedIsland) allowed.insert(island.second);
    uint64_t target = m.module() + 0x19011a0;
    for (const auto& region : m.regions()) {
        std::vector<uint8_t> data(region.size); if (!m.read(region.base, data.data(), data.size())) continue;
        for (size_t i = 0; i + 8 <= data.size(); i += 8) {
            uint64_t value = 0; memcpy(&value, data.data() + i, 8); if (value != target) continue;
            uintptr_t building = region.base + i;
            if (m.i32(building + 0x148) == wantedPlayer && allowed.count(building_island(m, building)) && production_property(m, building)) result.push_back(building);
        }
    }
    return result;
}

static bool describe_building(const GameMemory& m, uintptr_t building, const std::vector<std::string>& wares, BuildingInfo& out) {
    uintptr_t container = m.ptr(building + 0x150), wp = m.ptr(container + 0x68), property = production_property(m, building);
    if (!container || !property || m.ptr(wp) != m.module() + 0x1976448) return false;
    uint32_t product = m.u32(wp + 0x14), cycle = m.u32(wp + 0x10), amount = m.u32(wp + 0x1c);
    uint32_t productivity = m.u32(property + 0x18); uint8_t stopped = 0;
    if (!m.get(property + 0x1c, stopped) || !product || product >= wares.size() || wares[product].empty() || !cycle) return false;
    out.product = wares[product]; out.type = out.product; out.productivity = (int)productivity; out.paused = stopped != 0;
    out.rate = stopped ? 0.0 : amount / 1000.0 * 60000.0 / cycle;
    out.current = out.rate * productivity / 100.0;
    uintptr_t factory = m.ptr(container + 0x10);
    if (factory && !stopped && m.ptr(property) == m.module() + 0x1902358) {
        for (uintptr_t offset : {0x10u, 0x1cu}) {
            uint32_t good = m.u32(factory + offset), input = m.u32(factory + offset + 8);
            if (good > 0 && good < wares.size() && input && !wares[good].empty()) {
                out.inputs[wares[good]] += input / 1000.0 * 60000.0 / cycle * productivity / 100.0;
                out.potentialInputs[wares[good]] += input / 1000.0 * 60000.0 / cycle;
            }
        }
    }
    return true;
}
