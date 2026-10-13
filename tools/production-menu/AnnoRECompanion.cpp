// Native external companion for annobridge.
// This is deliberately a separate process: game-memory readers live here,
// outside Addon.exe, while the DLL remains a small GUI bridge.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#pragma comment(lib, "user32.lib")
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <vector>
#include <map>
#include <set>
#include <string>
#include <algorithm>
#include <sstream>
#include <iomanip>
#include <cmath>
#include <functional>
#include <thread>
#include <mutex>
#include <condition_variable>
#include <atomic>
#include <chrono>
#include "ui_protocol.h"
#include "dialog_layout.h"
#include "demand_rates.h"
#include "game_identity.h"
#ifndef ANNORE_UI_TEST
#ifdef ANNORE_HISTORY64
static_assert(sizeof(void*) == 8, "History companion requires the x64 toolset");
#else
static_assert(sizeof(void*) == 4, "Gold companion backend requires x86; do not rebuild this reader for History");
#endif
#endif

#ifdef ANNORE_HISTORY64
#define BRIDGE_NAME "AnnoREHistoryBridge"
#else
#define BRIDGE_NAME "AnnoREBridge"
#endif
#define BRIDGE_SIZE 4096
#define BRIDGE_MAGIC 0x4F4E4E41u
#define BRIDGE_VERSION 8
#define OP_XML 10
#define OP_REBUILDPATHS 11
#define OP_SETTEXT 2
#define OP_CLICK 9
#define OPST_PENDING 1
#define OPST_DONE 2
#define OPST_WAITSRC 5
#define O_MAGIC 0
#define O_VERSION 4
#define O_OPREQ 648
#define O_OPSTATUS 652
#define O_OPTARGET 656
#define O_OPARG 720
#define O_OPTEXT 980
#define O_OPRESULT 1236
#define O_SELECTION_SEQ 2400
#define O_SELECTION_ROW 2404
#define O_SELECTION_CTRL 2408
#define OP_SETVISIBLE 4

#pragma pack(push, 4)
struct Bridge {
    uint32_t magic;
    uint32_t version;
    uint8_t pad0[O_OPREQ - 8];
    uint32_t opRequest;
    uint32_t opStatus;
    char opTarget[64];
    char opArg[260];
    uint16_t opText[128];
    uint32_t opResult;
};
#pragma pack(pop)

#ifdef ANNORE_HISTORY64
#include "history_reader.h"
#else
#include "gold_reader.h"
#endif

static std::string format_need(double v, bool known) {
    if (!known) return "?";
    std::ostringstream s;
    if (std::fabs(v - std::round(v)) < 0.005) s << (int)std::round(v);
    else { s << std::fixed << std::setprecision(2) << v; }
    return s.str();
}

static bool g_consoleSnapshot = false;

static std::wstring ui_text(const std::string& value) {
#ifdef ANNORE_HISTORY64
    if (value.empty()) return {};
    int length = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, value.data(), (int)value.size(), NULL, 0);
    if (length <= 0) return L"?";
    std::wstring result(length, 0);
    MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, value.data(), (int)value.size(), &result[0], length);
    return result;
#else
    return std::wstring(value.begin(), value.end());
#endif
}

#include "goods_icons.h"
static int productPage = 0;
static int productFilter = 0;
static int good_group(const std::string& good) {
    for (const auto& rate : demandRates) if (good == rate.good) return 1;
    for (const char* material : {"Wood","Tools","Stone","Glass","Mosaic"})
        if (good == material) return 2;
    return 3;
}
struct SettlementSnapshot {
    std::string name;
    int population = 0;
    int tiers[11] = {};
    std::map<std::string, double> inputs;
    std::map<std::string, double> potentialInputs;
    std::map<std::string, double> potential;
    std::map<std::string, double> current;
    std::map<std::string, std::pair<int, int>> produced;
};
struct ProductionSnapshot {
    uint64_t generation = 0;
    int player = -1;
    int globalTiers[11] = {};
    std::map<int, SettlementSnapshot> settlements;
};

// Discovery and all expensive game reads belong exclusively to this worker.
// Snapshots contain values, never game pointers. Re-discovery automatically
// handles new/demolished buildings and save changes without stale pointer caches.
class SnapshotWorker {
    GameMemory mem;
    std::mutex mutex;
    std::condition_variable wake;
    bool stopping = false;
    ProductionSnapshot latest;
    std::thread thread; // constructed last: run() may access all preceding members
public:
    explicit SnapshotWorker(HANDLE game) : mem(game), thread([this] { run(); }) {}
    ~SnapshotWorker() {
        { std::lock_guard<std::mutex> lock(mutex); stopping = true; }
        wake.notify_all();
        thread.join();
    }
    ProductionSnapshot snapshot() {
        std::lock_guard<std::mutex> lock(mutex);
        return latest;
    }
private:
    void run() {
        uint64_t generation = 0;
        for (;;) {
            auto started = GetTickCount64();
            ProductionSnapshot next;
            next.player = game_player(mem);
            auto islands = find_islands(mem);
            uintptr_t world = game_world(mem);
            if (next.player >= 0 && next.player < 8) {
                for (int tier = 0; tier < 11; ++tier)
                    next.globalTiers[tier] = std::max(0, global_population(mem, world, tier, next.player));
                for (const auto& island : islands) {
                    std::string name = settlement_name(mem, island.second, next.player);
                    if (name.empty()) continue;
                    auto& settlement = next.settlements[island.first];
                    settlement.name = name;
                    for (int tier = 0; tier < 11; ++tier)
                        settlement.tiers[tier] = std::max(0, island_population(mem, island.second, tier, next.player));
                    settlement.population = std::max(0, island_population(mem, island.second, 7, next.player));
                }
                auto wares = good_names(mem);
                // One discovery pass for ALL settlements, irrespective of selection.
                if (!next.settlements.empty()) {
                    for (uintptr_t building : find_buildings(mem, islands, -1, next.player)) {
                        uintptr_t addr = building_island(mem, building);
                        auto island = std::find_if(islands.begin(), islands.end(),
                            [&](const auto& item) { return item.second == addr; });
                        if (island == islands.end()) continue;
                        auto settlement = next.settlements.find(island->first);
                        if (settlement == next.settlements.end()) continue;
                        BuildingInfo info;
                        if (!describe_building(mem, building, wares, info)) continue;
                        auto& count = settlement->second.produced[info.product];
                        settlement->second.potential[info.product] += info.rate;
                        settlement->second.current[info.product] += info.current;
                        for (const auto& input : info.inputs) settlement->second.inputs[input.first] += input.second;
                        for (const auto& input : info.potentialInputs) settlement->second.potentialInputs[input.first] += input.second;
                        ++count.first;
                        if (!info.paused) ++count.second;
                    }
                }
            }
            // Reject scans spanning a world/player change. A read-only scan is
            // not an atomic game snapshot; this catches common save transitions.
            if (world != game_world(mem) || next.player != game_player(mem)
                || islands != find_islands(mem))
                next.settlements.clear();
#ifdef ANNORE_HISTORY64
            // Reject divergent/non-atomic population reads rather than display
            // unverified totals. Same check as the exploratory History probe.
            for (int tier = 0; tier < 11; ++tier) {
                int64_t sum = 0;
                for (const auto& island : islands) sum += island_population(mem, island.second, tier, next.player);
                if (sum != next.globalTiers[tier]) next.settlements.clear();
            }
#endif
            next.generation = ++generation;
            auto elapsed = GetTickCount64() - started;
            if (generation == 1 || elapsed >= 500) {
                char msg[160];
                sprintf_s(msg, "snapshot: generation=%llu settlements=%u scan_ms=%llu",
                    (unsigned long long)generation, (unsigned)next.settlements.size(),
                    (unsigned long long)elapsed);
                log_line(msg);
            }
            std::unique_lock<std::mutex> lock(mutex);
            latest = std::move(next);
            if (wake.wait_for(lock, std::chrono::seconds(1), [this] { return stopping; })) return;
        }
    }
};

static std::vector<std::wstring> production_lines(const ProductionSnapshot& snapshot,
                                                   const std::vector<int>& wanted) {
    std::vector<std::wstring> lines;
    std::map<std::string, std::pair<int, int>> produced;
    int pop = 0;
    for (int id : wanted) {
        auto settlement = snapshot.settlements.find(id);
        if (settlement == snapshot.settlements.end()) continue;
        pop += settlement->second.population;
        for (const auto& item : settlement->second.produced) {
            auto& total = produced[item.first];
            total.first += item.second.first;
            total.second += item.second.second;
        }
    }
    if (wanted.empty()) {
        lines.push_back(L"[FONT Tooltip][COLOR FFFFFFFF]Hover an island or select a settlement.");
        return lines;
    }
    if (produced.empty()) {
        lines.push_back(L"[FONT Tooltip][COLOR FFFFFFFF]No production buildings found for the selected settlement(s).");
        return lines;
    }
    std::vector<std::string> goods;
    for (const auto& p : produced) goods.push_back(p.first);
    if (pop > 0) { if (std::find(goods.begin(), goods.end(), "SaltFish") == goods.end()) goods.push_back("SaltFish");
                   if (std::find(goods.begin(), goods.end(), "Cider") == goods.end()) goods.push_back("Cider"); }
    std::sort(goods.begin(), goods.end());
    for (size_t i = 0; i < goods.size() && i < 12; ++i) {
        auto p = produced[goods[i]];
        bool known = goods[i] == "SaltFish" || goods[i] == "Cider";
        double need = goods[i] == "SaltFish" ? pop / 200.0 : pop / 340.0;
        std::ostringstream text;
        text << "[FONT IngameMessages][COLOR " << (known ? (p.second >= need ? "FFAAFFAA" : "FFFFAA00") : "FF655747")
             << "]" << goods[i] << "[DEFAULTCOLOR]  [COLOR FFFFFFFF]Need: "
             << format_need(need, known) << "  Produced: " << p.first;
        if (p.second != p.first) text << " (" << p.second << "/" << p.first << " active)";
        text << "[DEFAULTCOLOR]";
        std::string narrow = text.str();
        lines.push_back(ui_text(narrow));
    }
    return lines;
}

// Desired presentation is coalesced while a batch is in flight. Only successful
// acknowledgements enter the applied cache; a timeout NEVER releases ownership.
struct ProductRow { std::string name; double demand = 0, potentialDemand = 0, potential = 0, current = 0; bool known = false; };
static std::vector<ProductRow> product_rows(const ProductionSnapshot& snapshot, const std::vector<int>& selected) {
    std::map<std::string, ProductRow> goods;
    for (int id : selected) {
        auto it = snapshot.settlements.find(id); if (it == snapshot.settlements.end()) continue;
        for (const auto& p : it->second.produced) goods[p.first].name = p.first;
        for (const auto& p : it->second.potential) { goods[p.first].name = p.first; goods[p.first].potential += p.second; }
        for (const auto& p : it->second.current) { goods[p.first].name = p.first; goods[p.first].current += p.second; }
        int tiers[11]; std::copy(it->second.tiers, it->second.tiers + 11, tiers);
        // Retain compatibility with legacy value-only snapshot callers.
        tiers[7] = it->second.population;
        for (const auto& rate : demandRates) {
            double demand = resident_demand(rate, tiers, snapshot.globalTiers);
            if (demand > 0) { auto& row = goods[rate.good]; row.name = rate.good; row.demand += demand; row.potentialDemand += demand; }
        }
        for (const auto& input : it->second.inputs) {
            auto& row = goods[input.first]; row.name = input.first; row.demand += input.second;
        }
        for (const auto& input : it->second.potentialInputs) {
            auto& row = goods[input.first]; row.name = input.first; row.potentialDemand += input.second;
        }
    }
    std::vector<ProductRow> rows;
    for (auto& g : goods) {
        auto& r = g.second;
        if (productFilter && good_group(r.name) != productFilter) continue;
        r.known = true;
        rows.push_back(r);
    }
    std::stable_sort(rows.begin(), rows.end(), [](const ProductRow& a, const ProductRow& b) {
        bool missingA = a.known && a.potential + .0001 < a.demand;
        bool missingB = b.known && b.potential + .0001 < b.demand;
        return missingA != missingB ? missingA : a.name < b.name;
    });
    return rows;
}
static int bar_fill(double value, double maximum) {
    return maximum > 0 ? (int)std::round(std::max(0.0, std::min(1.0, value / maximum)) * 10000) : 0;
}
static std::wstring rate_text(double value) {
    std::wostringstream out; out << std::fixed << std::setprecision(2) << value << L" t/min"; return out.str();
}
static std::wstring good_name(const std::string& name) {
    if (name == "SaltFish") return L"Fish";
    std::wstring out;
    for (size_t i = 0; i < name.size(); ++i) {
        if (i && name[i] >= 'A' && name[i] <= 'Z' && name[i-1] >= 'a' && name[i-1] <= 'z') out += L' ';
        out += (wchar_t)name[i];
    }
    return out;
}

class UiPresenter {
    UiShared* shared;
    std::map<std::string, UiUpdate> desired, applied, submitted;
    bool pending = false;
    ULONGLONG sentAt = 0;
    uint32_t sequence = 0;
    uint32_t desiredClickTick = 0, submittedClickTick = 0;
    int32_t player = -1;
    int32_t rowIslands[19] = {};
public:
    uint32_t lastResult = 0;
    explicit UiPresenter(UiShared* ui) : shared(ui) {}
    void clear() { desired.clear(); }
    void invalidate() { applied.clear(); }
    void selectionClicked(uint32_t tick) { desiredClickTick = tick; }
    void selectionMap(int slot, const std::vector<int>& ids) {
        player = slot;
        ZeroMemory(rowIslands, sizeof(rowIslands));
        for (size_t i = 0; i < ids.size() && i < 19; ++i) rowIslands[i] = ids[i];
    }
    void text(const std::string& target, const std::wstring& value) {
        UiUpdate& u = desired[target];
        strncpy_s(u.target, target.c_str(), _TRUNCATE);
        wcsncpy_s((wchar_t*)u.text, 128, value.c_str(), _TRUNCATE);
        u.flags |= UI_TEXT;
    }
    void visible(const std::string& target, bool value) {
        UiUpdate& u = desired[target];
        strncpy_s(u.target, target.c_str(), _TRUNCATE);
        u.visible = value ? 1 : 0;
        u.flags |= UI_VISIBLE;
    }
    void checked(const std::string& target, bool value) {
        UiUpdate& u = desired[target];
        strncpy_s(u.target, target.c_str(), _TRUNCATE);
        u.flags |= UI_CHECKED;
        if (value) u.flags |= UI_CHECKED_ON;
        else u.flags &= ~UI_CHECKED_ON;
    }
    void rect(const std::string& target, const UiRect& value) {
        UiUpdate& u = desired[target];
        strncpy_s(u.target, target.c_str(), _TRUNCATE);
        u.rect[0] = value.left; u.rect[1] = value.top;
        u.rect[2] = value.right; u.rect[3] = value.bottom;
        u.flags |= UI_RECT;
    }
    void bar(const std::string& target, int value) {
        UiUpdate& u = desired[target]; strncpy_s(u.target, target.c_str(), _TRUNCATE);
        u.flags |= UI_BAR; u.barValue = value; u.barMax = 10000;
    }
    void icon(const std::string& target, int guid) {
        UiUpdate& u = desired[target]; strncpy_s(u.target, target.c_str(), _TRUNCATE);
        u.flags |= UI_ICON; u.icon = guid;
    }
    bool acknowledge() {
        if (!pending || InterlockedCompareExchange(&shared->state, 0, 0) != 0) return false;
        MemoryBarrier();
        lastResult = shared->result;
        if (!lastResult) for (const auto& item : submitted) applied[item.first] = item.second;
        else applied.clear();
        pending = false;
        char msg[160];
        sprintf_s(msg, "ui: sequence=%u updates=%u ack_ms=%llu click_to_ack_ms=%u result=%u",
            shared->sequence, shared->count,
            (unsigned long long)(GetTickCount64() - sentAt),
            submittedClickTick ? GetTickCount() - submittedClickTick : 0, lastResult);
        log_line(msg);
        return true;
    }
    bool busy() const { return pending; }
    bool submit(bool probe) {
        if (pending || InterlockedCompareExchange(&shared->state, 0, 0) != 0) return false;
        uint32_t count = 0;
        submitted.clear();
        for (const auto& item : desired) {
            auto old = applied.find(item.first);
            if (old != applied.end() && memcmp(&old->second, &item.second, sizeof(UiUpdate)) == 0)
                continue;
            if (count == UI_CAPACITY) break; // Drain large grids in acknowledged batches.
            shared->updates[count++] = item.second;
            submitted[item.first] = item.second;
        }
        bool mapChanged = player != shared->displayedPlayer ||
            memcmp(rowIslands, shared->displayedIslands, sizeof(rowIslands)) != 0;
        if (!count && !probe && !mapChanged) return false;
        shared->count = count;
        shared->result = 0;
        shared->sequence = ++sequence;
        shared->player = player;
        memcpy(shared->rowIslands, rowIslands, sizeof(rowIslands));
        sentAt = GetTickCount64();
        submittedClickTick = desiredClickTick;
        desiredClickTick = 0;
        pending = true;
        MemoryBarrier();
        InterlockedExchange(&shared->state, 1);
        return true;
    }
};

static void present(UiPresenter& ui, const ProductionSnapshot& snapshot,
                    const std::vector<int>& ids, const std::vector<int>& selected,
                    int viewportWidth = 1280, int viewportHeight = 1024) {
    ui.clear();
    ui.selectionMap(snapshot.player, ids);
    const std::string root = "SYSTEMMESSAGES/ANNORE_PROD5_DIALOG/";
    auto layout = MakeDialogLayout(viewportWidth, viewportHeight, (int)ids.size());
    ui.rect("SYSTEMMESSAGES/ANNORE_PROD5_DIALOG", {0,0,viewportWidth,viewportHeight});
    ui.rect(root + "CARD1", {0,0,viewportWidth,viewportHeight});
    ui.visible(root + "CARD1", true);
    ui.rect(root + "CARD2", layout.frame);
    ui.visible(root + "CARD2", true);
    const int originX = layout.frame.left, originY = layout.frame.top;
    auto place = [&](UiRect& rect) { rect.left += originX; rect.right += originX; rect.top += originY; rect.bottom += originY; };
    for (UiRect* rect : {&layout.title,&layout.help,&layout.close,&layout.settlementsHeader,&layout.productionHeader}) place(*rect);
    for (auto& rect : layout.settlements) place(rect);
    for (auto& rect : layout.production) place(rect);
    ui.rect(root + "TITLE", layout.title);
    ui.text(root + "HELP", L"[FONT Tooltip][COLOR FF382719]Ctrl-click settlements to combine. Potential assumes inputs and warehouse space.");
    ui.rect(root + "HELP", layout.help);
    ui.text(root + "CLOSE", L"Close");
    ui.rect(root + "CLOSE", layout.close);
    ui.text(root + "SETTLEMENTS_HEADER", L"[FONT Tooltip][COLOR FF382719]Settlements");
    ui.rect(root + "SETTLEMENTS_HEADER", layout.settlementsHeader);
    ui.text(root + "PRODUCTION_HEADER", L"[FONT Tooltip][COLOR FF382719]Blue: resident + factory demand estimate   Green: output at 100%   (t/min)");
    ui.rect(root + "PRODUCTION_HEADER", layout.productionHeader);
    for (int row = 1; row <= 19; ++row) {
        char name[32]; sprintf_s(name, "ANNORE_ISLAND_ROW%02d", row);
        std::string text;
        if (row <= (int)ids.size()) {
            int id = ids[row - 1];
            text = snapshot.settlements.at(id).name;
        }
        ui.text(root + name, ui_text(text));
        ui.visible(root + name, row <= (int)ids.size());
        ui.checked(root + name, row <= (int)ids.size() &&
            std::find(selected.begin(), selected.end(), ids[row - 1]) != selected.end());
        ui.rect(root + name, layout.settlements[row - 1]);
    }
    std::string title = "Production";
    if (selected.size() == 1) title += ": " + snapshot.settlements.at(selected[0]).name;
    else if (!selected.empty()) title += ": " + std::to_string(selected.size()) + " settlements";
    ui.text(root + "TITLE", L"[FONT H2][COLOR FF382719]" + ui_text(title));
    auto products = product_rows(snapshot, selected);
    const wchar_t* filterLabels[] = {L"All", L"Consumer goods", L"Construction materials", L"Other"};
    const int filterIcons[] = {1130,607,8114,32020};
    const int captionWidths[] = {26,112,174,42};
    const int filterWidth = (layout.production[0].right - layout.production[0].left - 24) / 4;
    for (int filter = 0; filter < 4; ++filter) {
        std::string target = root + "FILTER" + std::to_string(filter);
        int x = layout.production[0].left + filter * (filterWidth + 8);
        ui.rect(target, {x,originY + 154,x + filterWidth,originY + 202});
        ui.text(target, L"");
        ui.checked(target, productFilter == filter);
        int groupLeft = x + (filterWidth - captionWidths[filter] - 30) / 2;
        std::string iconTarget = root + "FILTERICON" + std::to_string(filter);
        std::string textTarget = root + "FILTERTEXT" + std::to_string(filter);
        ui.icon(iconTarget, filterIcons[filter]);
        ui.rect(iconTarget, {groupLeft,originY + 167,groupLeft + 22,originY + 189});
        ui.rect(textTarget, {groupLeft + 30,originY + 154,groupLeft + 30 + captionWidths[filter],originY + 202});
        ui.text(textTarget, std::wstring(L"[FONT Tooltip][COLOR FFF4E9C8]") + filterLabels[filter]);
    }
    const std::string grid = root + "RESOURCE_SCROLL/";
    ui.rect(root + "RESOURCE_SCROLL", {layout.production[0].left,originY + 218,layout.production[0].right,layout.frame.bottom - 30});
    const int areaLeft = 0, areaRight = layout.production[0].right - layout.production[0].left - 24;
    const int left = areaLeft, right = areaRight;
    const int columnGap = 20, columnWidth = (areaRight - areaLeft - columnGap) / 2;
    const int rowHeight = 116;
    for (int i = 0; i < 68; ++i) {
        char name[16]; sprintf_s(name, "ROW%02d", i + 1);
        std::string target = grid + name;
        int index = i;
        bool visible = index < (int)products.size();
        std::string suffix = std::to_string(i + 1);
        for (const char* prefix : {"CARD", "ICON", "DEMPOT", "SUPPOT", "DEMLABEL", "DEMBASE", "SUPBASE", "SUPCUR", "SUPLABEL", "DEMCUR"})
            ui.visible(grid + prefix + suffix, visible);
        if (!visible) {
            for (const char* prefix : {"CARD", "ICON", "DEMPOT", "SUPPOT", "DEMLABEL", "DEMBASE", "SUPBASE", "SUPCUR", "SUPLABEL", "DEMCUR"})
                ui.rect(grid + prefix + suffix, {0,0,1,1});
            continue;
        }
        const int left = areaLeft + (i % 2) * (columnWidth + columnGap);
        const int right = left + columnWidth;
        const auto& r = products[index]; int y = (i / 2) * rowHeight;
        int barLeft = left + 97;
        int barRight = right - 32;
        // Keep product identity in a steady left column and stack two compact
        // labeled meters in the remaining space.
        ui.rect(grid + "CARD" + suffix, {left, y, right, y + rowHeight - 2});
        int icon = GoodsIcon(r.name);
        ui.icon(grid + "ICON" + suffix, icon); ui.visible(grid + "ICON" + suffix, icon >= 0);
        ui.rect(grid + "ICON" + suffix, {left + 45, y + 38, left + 85, y + 78});
        double maximum = std::max(std::max(r.current, r.potentialDemand), std::max(r.demand, r.potential));
        int demandFill = r.known ? bar_fill(r.demand, maximum) : 0;
        int outputFill = bar_fill(r.potential, maximum);
        ui.bar(grid + "DEMPOT" + suffix, bar_fill(r.potentialDemand, maximum));
        ui.bar(grid + "SUPPOT" + suffix, outputFill);
        ui.bar(grid + "SUPCUR" + suffix, bar_fill(r.current, maximum));
        ui.visible(grid + "DEMCUR" + suffix, true);
        ui.bar(grid + "DEMCUR" + suffix, demandFill);
        ui.rect(grid + "DEMCUR" + suffix, {barLeft, y + 34, barRight, y + 54});
        ui.rect(grid + "SUPCUR" + suffix, {barLeft, y + 60, barRight, y + 80});
        ui.rect(grid + "SUPLABEL" + suffix, {barLeft + 4, y + 60, barRight - 4, y + 80});
        ui.text(grid + "SUPLABEL" + suffix, L"[FONT Tooltip][COLOR FFF4E9C8]" + std::to_wstring((int)std::ceil(r.potentialDemand)) + L" / " + std::to_wstring((int)std::ceil(r.potential)));
        ui.rect(grid + "DEMBASE" + suffix, {barLeft, y + 34, barRight, y + 54});
        ui.rect(grid + "SUPBASE" + suffix, {barLeft, y + 60, barRight, y + 80});
        ui.rect(grid + "DEMPOT" + suffix, {barLeft, y + 34, barRight, y + 54});
        ui.rect(grid + "SUPPOT" + suffix, {barLeft, y + 60, barRight, y + 80});
        ui.rect(grid + "DEMLABEL" + suffix, {barLeft + 4, y + 34, barRight - 4, y + 54});
        const wchar_t* demandTextColor = L"FFF4E9C8";
        const wchar_t* outputTextColor = L"FFF4E9C8";
        ui.text(grid + "DEMLABEL" + suffix, L"[FONT Tooltip][COLOR " + std::wstring(demandTextColor) + L"]" + std::to_wstring((int)std::ceil(r.demand)) + L" / " + std::to_wstring((int)std::ceil(r.current)));
        std::wstring status;
        if (!r.known) status = L"[COLOR FF655747]Demand unknown";
        else if (r.potential + .0001 < r.demand) status = L"[COLOR FF992F20]Missing " + rate_text(r.demand - r.potential);
        else if (!maximum) status = L"[COLOR FF382719]No demand / capacity";
        else if (std::fabs(r.potential - r.demand) < .0001) status = L"[COLOR FF305B22]Balanced";
        else status = L"[COLOR FF305B22]Capacity +" + rate_text(r.potential - r.demand);
    }
    for (const char* name : {"PREVIOUS","NEXT","PAGE"}) ui.visible(root + name, false);
    ui.visible(root + "EMPTY", products.empty());
    ui.rect(root + "EMPTY", {layout.production[0].left,originY + 218,layout.production[0].right,originY + 264});
    ui.text(root + "EMPTY", selected.empty() ? L"Select an owned settlement." : L"No goods match this filter.");

}

static void log_line(const char* text) {
#ifdef ANNORE_UI_TEST
    return; // tests must not write the runtime log outside the repository
#endif
    if (g_consoleSnapshot) { fprintf(stderr, "%s\n", text); return; }
    char path[MAX_PATH] = {0};
#ifdef ANNORE_HISTORY64
    // Native experiment logs stay beside the installed candidate. Console
    // snapshots never reach this branch and remain read-only diagnostics.
    if (!GetModuleFileNameA(NULL, path, MAX_PATH)) return;
    char* slash = strrchr(path, '\\'); if (!slash) return; slash[1] = 0;
    strcat_s(path, "annobridge-history.log");
#else
    if (!GetTempPathA(MAX_PATH, path)) return;
    strcat_s(path, "annobridge.log");
#endif
    FILE* f = NULL;
    if (fopen_s(&f, path, "a") != 0 || !f) return;
    SYSTEMTIME t; GetLocalTime(&t);
    fprintf(f, "[%02d:%02d:%02d] companion: %s\n", t.wHour, t.wMinute, t.wSecond, text);
    fclose(f);
}

static void copy_ascii(char* dst, size_t cap, const char* src) {
    strncpy_s(dst, cap, src ? src : "", _TRUNCATE);
}

static bool run_op(Bridge* b, uint32_t op, const char* target,
                   const char* arg, const wchar_t* text, DWORD timeoutMs) {
    // A timed-out operation may still be executing on the GUI thread. Do not
    // overwrite its payload. The lifecycle probe will discover any late install.
    uint32_t previousStatus = (uint32_t)InterlockedCompareExchange((volatile LONG*)&b->opStatus, 0, 0);
    if (InterlockedCompareExchange((volatile LONG*)&b->opRequest, 0, 0) != 0 ||
        previousStatus == OPST_PENDING || previousStatus == OPST_WAITSRC) return false;
    copy_ascii(b->opTarget, sizeof(b->opTarget), target);
    copy_ascii(b->opArg, sizeof(b->opArg), arg);
    if (text) {
        wcsncpy_s((wchar_t*)b->opText, 128, text, _TRUNCATE);
    } else {
        b->opText[0] = 0;
    }
    b->opResult = 0;
    MemoryBarrier();
    b->opStatus = OPST_PENDING;
    InterlockedExchange((volatile LONG*)&b->opRequest, (LONG)op);
    DWORD start = GetTickCount();
    for (;;) {
        Sleep(20);
        uint32_t status = (uint32_t)InterlockedCompareExchange((volatile LONG*)&b->opStatus, 0, 0);
        if (status != OPST_PENDING && status != OPST_WAITSRC)
            return status == OPST_DONE;
        if (GetTickCount() - start >= timeoutMs) return false;
    }
}

static bool set_text(Bridge* b, const char* target, const wchar_t* text) {
    return run_op(b, OP_SETTEXT, target, "", text, 3000);
}

struct ProductionShortcut {
    bool held = true; // A held shortcut at startup must be released first.
    bool pressed(bool control, bool q, bool gameForeground) {
        bool down = control && q;
        bool trigger = down && !held && gameForeground;
        held = down; // Track unfocused keys too; focusing must not toggle.
        return trigger;
    }
};

static bool install_button(Bridge* b, const char* buttonPath) {
    if (!run_op(b, OP_XML, "INFOBAR", buttonPath, NULL, 15000)) return false;
    set_text(b, "INFOBAR/ANNORE_PROD5_BUTTON",
             L"[FONT Tooltip][COLOR FFFFFFFF][CENTER]Production");
    log_line("production trigger installed");
    return true;
}

static bool install_panel(Bridge* b, const char* dialogPath) {
    // Install the dialog once. During HUD transitions INFOBAR can be rebuilt
    // while SYSTEMMESSAGES survives; reinstalling both created duplicate dialogs
    // and mismatched selector rows.
    if (!run_op(b, OP_XML, "SYSTEMMESSAGES", dialogPath, NULL, 15000)) return false;
    set_text(b, "SYSTEMMESSAGES/ANNORE_PROD5_DIALOG/TITLE",
             L"[FONT Tooltip][COLOR FFFFFFFF]Production");
    set_text(b, "SYSTEMMESSAGES/ANNORE_PROD5_DIALOG/HELP",
             L"[FONT Tooltip][COLOR FFFFFFFF]Hover an island; click Production or press Ctrl+Q to toggle this dialog.");
    return true;
}

static bool module_dir(char* out, size_t cap) {
    char path[MAX_PATH] = {0};
    HMODULE self = NULL;
    if (!GetModuleHandleExA(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
                            (LPCSTR)&module_dir, &self)) return false;
    if (!GetModuleFileNameA(self, path, MAX_PATH)) return false;
    char* slash = strrchr(path, '\\');
    if (!slash) return false;
    *slash = 0;
    return strcpy_s(out, cap, path) == 0;
}

static bool consume_selection(UiShared* ui, const std::vector<int>& ids, int player,
                              std::vector<int>& selected, bool& manual,
                              uint32_t& lastSeq, uint32_t* clickTick = NULL) {
    uint32_t write = (uint32_t)InterlockedCompareExchange(&ui->clickWrite, 0, 0);
    if (write == lastSeq) return false;
    if (write - lastSeq > UI_CLICK_CAPACITY) {
        log_line("selection queue overflow; retaining newest clicks");
        lastSeq = write - UI_CLICK_CAPACITY;
    }
    while (lastSeq != write) {
        MemoryBarrier();
        UiClick click = ui->clicks[lastSeq++ % UI_CLICK_CAPACITY];
        // Rows 20..23 are reserved filter controls; settlement rows use 1..19.
        // The existing bridge already transports these button clicks.
        if (click.row >= 20 && click.row <= 31 && click.player == player) {
            productFilter = (click.row - 20) % 4;
            if (clickTick) *clickTick = click.tick;
            continue;
        }
        if (click.row == UI_DIALOG_CLOSE) {
            selected.clear(); manual = false;
            continue;
        }
        if (click.row == UI_NEXT || click.row == UI_PREVIOUS) {
            productPage = std::max(0, productPage + (click.row == UI_NEXT ? 1 : -1));
            if (clickTick) *clickTick = click.tick;
            continue;
        }
        if (click.row == UI_DIALOG_OPEN) {
            productPage = 0;
            // Opening owns a fresh selection even when hovering water or a
            // settlement belonging to another player; don't restore old choices.
            selected.clear(); manual = true;
            if (click.player == player && std::find(ids.begin(), ids.end(), click.island) != ids.end())
                selected.push_back(click.island);
            if (clickTick) *clickTick = click.tick;
            continue;
        }
        if (click.player != player || std::find(ids.begin(), ids.end(), click.island) == ids.end()) continue;
        if (clickTick) *clickTick = click.tick;
        int id = click.island;
        if (!click.ctrl) { selected.assign(1, id); manual = true; }
        else {
            auto item = std::find(selected.begin(), selected.end(), id);
            if (item == selected.end()) selected.push_back(id);
            else selected.erase(item);
            manual = !selected.empty();
        }
    }
    log_line("selection consumed");
    return true;
}

int main(int argc, char** argv) {
    DWORD parentPid = 0;
    bool consoleSnapshot = false;
    for (int i = 1; i < argc; ++i) if (_stricmp(argv[i], "--snapshot") == 0) consoleSnapshot = true;
    for (int i = 1; i + 1 < argc; ++i) {
        if (_stricmp(argv[i], "--parent-pid") == 0) parentPid = strtoul(argv[i + 1], NULL, 10);
    }
    if (!parentPid) return 2;
    HANDLE parent = parentPid ? OpenProcess(SYNCHRONIZE, FALSE, parentPid) : NULL;
    HANDLE game = parentPid ? OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_INFORMATION,
                                           FALSE, parentPid) : NULL;
    if (parentPid && (!parent || !game)) {
        if (game) CloseHandle(game);
        if (parent) CloseHandle(parent);
        return 2;
    }
#ifdef ANNORE_HISTORY64
    bool supportedProcess = verified_history_process(game);
#else
    bool supportedProcess = IsVerifiedGoldProcess(game);
#endif
    if (!supportedProcess) {
#ifdef ANNORE_HISTORY64
        fprintf(stderr, "Unsupported game binary: History64 only supports the verified Anno1404Addon.exe.\n");
#else
        fprintf(stderr, "Unsupported game binary: the Gold32 reader only supports the verified GOG Addon.exe.\n");
#endif
        CloseHandle(game); CloseHandle(parent);
        return 4;
    }
    if (consoleSnapshot) {
        g_consoleSnapshot = true;
        int result = 6;
        {
            SnapshotWorker worker(game);
            ULONGLONG deadline = GetTickCount64() + 30000;
            ProductionSnapshot snapshot;
            while (!snapshot.generation && GetTickCount64() < deadline) { Sleep(50); snapshot = worker.snapshot(); }
            auto quoted = [](const std::string& text) {
                std::ostringstream output; output << '"';
                for (unsigned char c : text) {
                    if (c == '"' || c == '\\') output << '\\' << (char)c;
                    else if (c < 32) { char escape[8]; sprintf_s(escape, "\\u%04x", c); output << escape; }
                    else output << (char)c;
                }
                output << '"'; return output.str();
            };
            std::ostringstream json;
#ifdef ANNORE_HISTORY64
            json << "{\"edition\":\"history-edition\",";
#else
            json << "{\"edition\":\"gog\",";
#endif
            json << "\"rate_kind\":\"productivity estimate, not measured deliveries\",\"player\":" << snapshot.player << ",\"settlements\":[";
            bool first = true;
            for (const auto& entry : snapshot.settlements) {
                if (!first) json << ','; first = false;
                const auto& settlement = entry.second;
                json << "{\"id\":" << entry.first << ",\"name\":" << quoted(settlement.name) << ",\"population_by_tier\":[";
                for (int tier = 0; tier < 11; ++tier) { if (tier) json << ','; json << settlement.tiers[tier]; }
                json << "],\"goods\":["; bool firstGood = true;
                for (const auto& good : settlement.produced) {
                    if (!firstGood) json << ','; firstGood = false;
                    auto current = settlement.current.find(good.first), potential = settlement.potential.find(good.first);
                    json << "{\"good\":" << quoted(good.first) << ",\"buildings\":" << good.second.first << ",\"unpaused\":" << good.second.second
                         << ",\"current_t_min\":" << (current == settlement.current.end() ? 0 : current->second)
                         << ",\"potential_t_min\":" << (potential == settlement.potential.end() ? 0 : potential->second) << '}';
                }
                json << "]}";
            }
            json << "]}";
            puts(json.str().c_str());
            if (!snapshot.settlements.empty()) result = 0;
        }
        CloseHandle(game); CloseHandle(parent);
        return result;
    }
#if defined(ANNORE_HISTORY64) && !defined(ANNORE_HISTORY_NATIVE_EXPERIMENT)
    // A complete reader is not proof that the native bridge is ready.
    // Keep engineering builds fail-closed until the x64 bridge is validated.
    fprintf(stderr, "History native GUI bridge is not yet available. Use --snapshot for read-only diagnostics.\n");
    CloseHandle(game); CloseHandle(parent);
    return 7;
#endif

    char dir[MAX_PATH], dialogPath[MAX_PATH], buttonPath[MAX_PATH];
    if (!module_dir(dir, sizeof(dir)) ||
        sprintf_s(dialogPath, sizeof(dialogPath), "%s\\production_dialog_v5.dlg", dir) < 0 ||
        sprintf_s(buttonPath, sizeof(buttonPath), "%s\\production_button.dlg", dir) < 0) {
        log_line("cannot resolve production XML path");
        if (game) CloseHandle(game);
        if (parent) CloseHandle(parent);
        return 3;
    }
    char preferences[MAX_PATH];
    sprintf_s(preferences, "%s\\AnnoRECompanion.ini", dir);
    productFilter = std::max(0, std::min(3, (int)GetPrivateProfileIntA("Production", "ResourceFilter", 0, preferences)));

    HANDLE map = NULL;
    Bridge* b = NULL;
    for (int i = 0; i < 120 && !b; ++i) {
        map = OpenFileMappingA(FILE_MAP_ALL_ACCESS, FALSE, BRIDGE_NAME);
        if (map) b = (Bridge*)MapViewOfFile(map, FILE_MAP_ALL_ACCESS, 0, 0, BRIDGE_SIZE);
        if (!b) Sleep(250);
    }
    if (!b || b->magic != BRIDGE_MAGIC || b->version != BRIDGE_VERSION) {
        log_line("bridge unavailable or incompatible");
        if (b) UnmapViewOfFile(b);
        if (map) CloseHandle(map);
        if (game) CloseHandle(game);
        if (parent) CloseHandle(parent);
        return 4;
    }

    HANDLE uiMap = OpenFileMappingA(FILE_MAP_ALL_ACCESS, FALSE, UI_MAP_NAME);
    UiShared* ui = uiMap ? (UiShared*)MapViewOfFile(uiMap, FILE_MAP_ALL_ACCESS, 0, 0, sizeof(UiShared)) : NULL;
    HANDLE uiDone = OpenEventA(SYNCHRONIZE, FALSE, UI_DONE_NAME);
    if (!ui || !uiDone || ui->magic != UI_MAGIC || ui->version != UI_VERSION ||
        ui->size != sizeof(UiShared) || ui->ownerPid != parentPid) {
        log_line("UI transport incompatible; install matching DLL and companion");
        if (ui) UnmapViewOfFile(ui);
        if (uiMap) CloseHandle(uiMap);
        if (uiDone) CloseHandle(uiDone);
        UnmapViewOfFile(b); CloseHandle(map);
        if (game) CloseHandle(game);
        if (parent) CloseHandle(parent);
        return 5;
    }
    log_line("started: asynchronous snapshots and batched presentation");
    {
        SnapshotWorker worker(game);
        GameMemory mem(game);
        UiPresenter presenter(ui);
        bool manualSelection = false;
        std::vector<int> ids, selected, drawnSelection, drawnIds;
        uint32_t lastSelectionSeq = (uint32_t)ui->clickWrite;
        uint64_t drawnGeneration = UINT64_MAX;
        int drawnPlayer = -1;
        int drawnWidth = 0, drawnHeight = 0, drawnPage = -1;
        int drawnFilter = -1, savedFilter = productFilter;
        ULONGLONG nextProbe = 0, nextPublish = 0;
        ProductionShortcut shortcut;
        for (;;) {
            if (parent && WaitForSingleObject(parent, 0) == WAIT_OBJECT_0) break;
            ULONGLONG now = GetTickCount64();
            DWORD foregroundPid = 0;
            GetWindowThreadProcessId(GetForegroundWindow(), &foregroundPid);
            if (shortcut.pressed((GetAsyncKeyState(VK_CONTROL) & 0x8000) != 0,
                                 (GetAsyncKeyState('Q') & 0x8000) != 0,
                                 foregroundPid == parentPid)) {
                // Reuse the native trigger: selection capture and open/close
                // behavior stay identical to clicking the Production button.
                run_op(b, OP_CLICK, "INFOBAR/ANNORE_PROD5_BUTTON", "", NULL, 100);
            }
            if (presenter.acknowledge() && presenter.lastResult) {
                // Every install follows a fresh GUI-thread probe. A late legacy
                // command cannot cause a blind retry and duplicate dialog tree.
                if (presenter.lastResult & UI_MISSING_DIALOG)
                    install_panel(b, dialogPath);
                if (presenter.lastResult & UI_MISSING_BUTTON)
                    install_button(b, buttonPath);
                presenter.invalidate();
                nextPublish = GetTickCount64() + 1000;
            }
            auto snapshot = worker.snapshot();
            std::vector<int> newIds;
            for (const auto& item : snapshot.settlements) newIds.push_back(item.first);
            ids = std::move(newIds);
            if (drawnPlayer != snapshot.player) { selected.clear(); manualSelection = false; }
            uint32_t clickTick = 0;
            consume_selection(ui, ids, snapshot.player, selected, manualSelection, lastSelectionSeq, &clickTick);
            if (savedFilter != productFilter) {
                char value[12]; sprintf_s(value, "%d", productFilter);
                if (WritePrivateProfileStringA("Production", "ResourceFilter", value, preferences)) savedFilter = productFilter;
                else log_line("could not persist resource filter");
            }
            if (clickTick) presenter.selectionClicked(clickTick);
            selected.erase(std::remove_if(selected.begin(), selected.end(), [&](int id) {
                return snapshot.settlements.find(id) == snapshot.settlements.end();
            }), selected.end());
            if (!manualSelection) {
                selected.clear();
                int hovered = game_current_island(mem);
                if (std::find(ids.begin(), ids.end(), hovered) != ids.end()) selected.push_back(hovered);
            }
            int width = (int)InterlockedCompareExchange(&ui->viewportWidth, 0, 0);
            int height = (int)InterlockedCompareExchange(&ui->viewportHeight, 0, 0);
            if (width <= 0) width = 1280;
            if (height <= 0) height = 1024;
            if (snapshot.generation != drawnGeneration || selected != drawnSelection || ids != drawnIds ||
                width != drawnWidth || height != drawnHeight || productPage != drawnPage || productFilter != drawnFilter) {
                present(presenter, snapshot, ids, selected, width, height);
                drawnGeneration = snapshot.generation;
                drawnPlayer = snapshot.player;
                drawnSelection = selected; drawnIds = ids;
                drawnWidth = width; drawnHeight = height; drawnPage = productPage;
                drawnFilter = productFilter;
            }
            bool probe = now >= nextProbe;
            if (now >= nextPublish && presenter.submit(probe) && probe) nextProbe = now + 2000;
            // Acknowledgement wakes immediately; click polling is independent of
            // scan duration and never waits on an outstanding batch.
            WaitForSingleObject(uiDone, 10);
        }
    } // join worker before closing the game handle
    UnmapViewOfFile(ui);
    CloseHandle(uiMap);
    CloseHandle(uiDone);

    log_line("exiting");
    UnmapViewOfFile(b);
    CloseHandle(map);
    if (game) CloseHandle(game);
    if (parent) CloseHandle(parent);
    return 0;
}
