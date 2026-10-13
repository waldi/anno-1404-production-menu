#pragma once
// Verified GOG reader extracted without changing its pointer/layout semantics.
// Addon.exe static addresses and production layouts. The companion only asks
// for PROCESS_VM_READ, so failures here cannot write to the game.
#define ADDR_CURRENT_ISLAND 0x012D54F8u
#define ADDR_CURRENT_PLAYER 0x012D54F4u
#define ADDR_GOP 0x012D5A90u
#define GOP_ISLAND_MAP 0x7F5Cu
#define ISLAND_ID_OFF 0x3FF0u
#define ISLAND_POP_OFF 0x2CE0u
#define ISLAND_WAREHOUSES 0x3F78u
#define ISLAND_SETTLEMENTS 0x39B8u
#define CISLAND_VTABLE 0x010FFD00u
#define WARE_NAMES 0x0127B820u
#define VT_PROPERTY_BUILDING 0x0110018Cu
#define BLD_PLAYER_SLOT 0xC0u
#define BLD_DATA_CONTAINER 0xC4u
#define BLD_ISLAND 0xD4u
#define DC_FACTORY 0x34u
#define DC_WARE_PRODUCTION 0x34u
#define DC_STANDARD 0x74u
#define DS_NAME 0x08u
#define DWP_CYCLE_MS 0x08u
#define DWP_PRODUCT 0x0Cu
#define DWP_AMOUNT 0x14u
#define PP_PRODUCTIVITY 0x10u
#define PP_STOPPED 0x14u
#define PROBE_MIN 0x01400000u
#define PROBE_MAX 0x7FFF0000u

struct Region { uintptr_t base, size; };
static bool valid_address(uintptr_t p) { return p >= 0x00400000u && p < PROBE_MAX; }

class GameMemory {
public:
    explicit GameMemory(HANDLE h) : h_(h) {}
    bool read(uintptr_t addr, void* out, SIZE_T size) const {
        SIZE_T got = 0;
        return valid_address(addr) && ReadProcessMemory(h_, (LPCVOID)addr, out, size, &got)
            && got == size;
    }
    template<class T> bool get(uintptr_t addr, T& out) const { return read(addr, &out, sizeof(out)); }
    uintptr_t ptr(uintptr_t addr) const { return u32(addr); }
    uint32_t u32(uintptr_t addr) const { uint32_t v = 0; return get(addr, v) ? v : 0; }
    int32_t i32(uintptr_t addr) const { int32_t v = 0; return get(addr, v) ? v : 0; }
    std::vector<Region> regions() const {
        std::vector<Region> out;
        uintptr_t p = 0;
        MEMORY_BASIC_INFORMATION mbi;
        while (p < PROBE_MAX && VirtualQueryEx(h_, (LPCVOID)p, &mbi, sizeof(mbi))) {
            uintptr_t base = (uintptr_t)mbi.BaseAddress;
            SIZE_T size = mbi.RegionSize;
            if (size == 0) break;
            DWORD prot = mbi.Protect & 0xff;
            bool readable = prot == PAGE_READONLY || prot == PAGE_READWRITE
                || prot == PAGE_EXECUTE_READ || prot == PAGE_EXECUTE_READWRITE;
            if (mbi.State == MEM_COMMIT && readable && !(mbi.Protect & PAGE_GUARD)
                && size <= 64 * 1024 * 1024)
                out.push_back({base, (uintptr_t)size});
            p = base + size;
        }
        return out;
    }
private:
    HANDLE h_;
};

static bool valid_heap_ptr(uintptr_t p) { return p >= PROBE_MIN && p < PROBE_MAX; }

static std::string game_cstr(const GameMemory& m, uintptr_t addr, size_t cap = 64) {
    if (!valid_address(addr)) return std::string();
    std::vector<char> buf(cap + 1, 0);
    if (!m.read(addr, buf.data(), cap)) return std::string();
    return std::string(buf.data());
}

static std::string inline_name(const GameMemory& m, uintptr_t addr) {
    if (!addr) return std::string();
    char buf[65] = {0};
    if (!m.read(addr + DS_NAME, buf, 64)) return std::string();
    return std::string(buf);
}

static std::map<int32_t, uintptr_t> find_islands(const GameMemory& m) {
    std::map<int32_t, uintptr_t> out;
    uintptr_t gop = m.u32(ADDR_GOP);
    if (!valid_heap_ptr(gop)) return out;
    uintptr_t hdr = gop + GOP_ISLAND_MAP;
    uint32_t count = m.u32(hdr + 0x10), root = m.u32(hdr + 4);
    std::vector<uintptr_t> seen;
    std::function<void(uintptr_t, int)> walk = [&](uintptr_t n, int depth) {
        if (!valid_heap_ptr(n) || n == hdr || depth > 64) return;
        if (std::find(seen.begin(), seen.end(), n) != seen.end()) return;
        seen.push_back(n);
        walk(m.u32(n + 8), depth + 1);
        out[m.i32(n + 0x10)] = m.u32(n + 0x14);
        walk(m.u32(n + 0xC), depth + 1);
    };
    if (root && count && count < 4096) walk(root, 0);
    if (out.size() != count) out.clear();
    return out;
}

static std::string settlement_name(const GameMemory& m, uintptr_t island, int slot) {
    if (!valid_heap_ptr(island) || slot < 0 || slot >= 8) return std::string();
    uint16_t raw[16] = {0};
    if (!m.read(island + ISLAND_SETTLEMENTS + slot * 0x30, raw, sizeof(raw))) return std::string();
    std::string out;
    for (int i = 0; i < 16 && raw[i]; ++i) {
        if (raw[i] < 32 || raw[i] > 0x2000) return std::string();
        // Settlement names in the game are normally Latin-1/Windows-1252.
        out.push_back(raw[i] < 128 ? (char)raw[i] : '?');
    }
    return out;
}

struct BuildingInfo { std::string product, type; int productivity = 0; bool paused = false; double rate = 0, current = 0; std::map<std::string, double> inputs, potentialInputs; };
static bool set_text(Bridge* b, const char* target, const wchar_t* text);
static void log_line(const char* text);

static std::string rtti_class(const GameMemory& m, uint32_t vt) {
    if (vt < 0x400000 || vt > 0x1400000) return std::string();
    uint32_t col = m.u32(vt - 4), td = col ? m.u32(col + 12) : 0;
    if (!td) return std::string();
    std::string n = game_cstr(m, td + 8);
    return n.rfind(".?AVCProperty", 0) == 0 ? n : std::string();
}

static uintptr_t production_property(const GameMemory& m, uintptr_t building) {
    for (uintptr_t off = 0x40; off < 0x84; off += 4) {
        uintptr_t p = m.u32(building + off);
        if (!valid_heap_ptr(p)) continue;
        std::string cls = rtti_class(m, m.u32(p));
        if (cls == ".?AVCPropertyProduction@@" || cls == ".?AVCPropertyFactory@@"
            || cls == ".?AVCPropertyFarm@@" || cls == ".?AVCPropertyShipProduction@@") return p;
    }
    return 0;
}

static std::vector<uintptr_t> find_buildings(const GameMemory& m,
                                              const std::map<int32_t, uintptr_t>& islands,
                                              int32_t wantedIsland, int wantedPlayer) {
    std::vector<uintptr_t> out;
    for (const Region& r : m.regions()) {
        std::vector<uint8_t> data(r.size);
        SIZE_T got = 0;
        if (!m.read(r.base, data.data(), data.size())) continue;
        for (size_t i = 0; i + 4 <= data.size(); i += 4) {
            uint32_t v = *(const uint32_t*)(data.data() + i);
            if (v != VT_PROPERTY_BUILDING) continue;
            uintptr_t owner = r.base + i + 4;
            uintptr_t building = m.u32(owner);
            uintptr_t island = valid_heap_ptr(building) ? m.u32(building + BLD_ISLAND) : 0;
            int slot = (int)m.u32(building + BLD_PLAYER_SLOT);
            if (slot < 0 || slot >= 8) continue;
            // `islands` is keyed by island id, while the building stores the
            // CIsland pointer. Match against the map value, not its key.
            if (std::find_if(islands.begin(), islands.end(), [&](const auto& x) {
                    return x.second == island && (wantedIsland < 0 || x.first == wantedIsland);
                }) != islands.end() && slot == wantedPlayer)
                out.push_back(building);
        }
    }
    return out;
}

static bool describe_building(const GameMemory& m, uintptr_t building,
                              const std::vector<std::string>& wares, BuildingInfo& out) {
    uintptr_t dc = m.u32(building + BLD_DATA_CONTAINER);
    if (!valid_heap_ptr(dc)) return false;
    uintptr_t wp = m.u32(dc + DC_WARE_PRODUCTION);
    uintptr_t prop = production_property(m, building);
    if (!valid_heap_ptr(wp) || !valid_heap_ptr(prop)) return false;
    uint32_t product = m.u32(wp + DWP_PRODUCT), cycle = m.u32(wp + DWP_CYCLE_MS);
    uint32_t amount = m.u32(wp + DWP_AMOUNT), productivity = m.u32(prop + PP_PRODUCTIVITY);
    uint8_t stopped = 0; m.read(prop + PP_STOPPED, &stopped, 1);
    out.product = product < wares.size() && !wares[product].empty() ? wares[product] : std::to_string(product);
    out.type = inline_name(m, m.u32(dc + DC_STANDARD));
    out.productivity = (int)productivity; out.paused = stopped != 0;
    // Potential is capacity at 100% productivity, independent of current yield.
    // Explicitly paused buildings remain excluded from available capacity.
    out.rate = stopped || !cycle ? 0.0 : (amount / 1000.0) * (60000.0 / cycle);
    out.current = out.rate * productivity / 100.0;
    uintptr_t factory = m.u32(dc + 0x08);
    if (factory && cycle && !stopped) {
        for (uintptr_t off : {0x08u, 0x14u}) {
            uint32_t ware = m.u32(factory + off), input = m.u32(factory + off + 8);
            if (ware > 0 && ware < wares.size() && input) {
                out.inputs[wares[ware]] += input / 1000.0 * 60000.0 / cycle * productivity / 100.0;
                out.potentialInputs[wares[ware]] += input / 1000.0 * 60000.0 / cycle;
            }
        }
    }
    return true;
}


static uintptr_t game_world(const GameMemory& m) { return m.u32(ADDR_GOP); }
static int game_player(const GameMemory& m) { return m.i32(ADDR_CURRENT_PLAYER); }
static int game_current_island(const GameMemory& m) { return m.i32(ADDR_CURRENT_ISLAND); }
static uintptr_t building_island(const GameMemory& m, uintptr_t p) { return m.u32(p + BLD_ISLAND); }
static int island_population(const GameMemory& m, uintptr_t p, int tier, int player) { return m.i32(p + ISLAND_POP_OFF + tier * 0x20 + player * 4); }
static int global_population(const GameMemory& m, uintptr_t world, int tier, int player) { return m.i32(world + 0x7C78 + (tier + 1) * 0x20 + player * 4); }
static std::vector<std::string> good_names(const GameMemory& m) {
    std::vector<std::string> wares(68);
    for (int i = 0; i < 68; ++i) wares[i] = game_cstr(m, m.u32(WARE_NAMES + i * 4));
    return wares;
}
