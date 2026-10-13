// Deterministic transport/presentation tests; no game process or injection.
#define main companion_main_for_test
#define ANNORE_UI_TEST
#include "AnnoRECompanion.cpp"
#undef main
#include <assert.h>

static void complete(UiShared& shared, uint32_t result = 0) {
    if (!result) {
        shared.displayedPlayer = shared.player;
        memcpy(shared.displayedIslands, shared.rowIslands, sizeof(shared.rowIslands));
    }
    shared.result = result;
    InterlockedExchange(&shared.state, 0);
}

int main() {
#ifdef ANNORE_HISTORY64
    assert(ui_text("M\xC3\xBCnchen") == L"M\u00FCnchen");
    assert(ui_text("\xFF") == L"?");
#endif
    ProductionShortcut shortcut;
    assert(!shortcut.pressed(true, true, true)); // no startup-held toggle
    assert(!shortcut.pressed(false, false, true));
    assert(shortcut.pressed(true, true, true));
    assert(!shortcut.pressed(true, true, true)); // holding does not repeat
    assert(!shortcut.pressed(false, false, false));
    assert(!shortcut.pressed(true, true, false)); // other apps unaffected
    assert(!shortcut.pressed(true, true, true)); // focusing while held
    assert(!shortcut.pressed(true, false, true));
    assert(shortcut.pressed(true, true, true));
    UiShared shared = {};
    UiPresenter ui(&shared);
    ui.text("root/title", L"first");
    ui.visible("root/title", true);
    assert(ui.submit(false));
    assert(shared.count == 1 && shared.state == 1);
    assert(shared.updates[0].flags == (UI_TEXT | UI_VISIBLE));
    assert(wcscmp((wchar_t*)shared.updates[0].text, L"first") == 0);

    // Latest desired state coalesces without overwriting an unacknowledged batch.
    ui.text("root/title", L"second");
    assert(!ui.submit(false));
    assert(wcscmp((wchar_t*)shared.updates[0].text, L"first") == 0);
    assert(!ui.acknowledge());
    complete(shared);
    assert(ui.acknowledge());
    assert(ui.submit(false));
    assert(wcscmp((wchar_t*)shared.updates[0].text, L"second") == 0);
    complete(shared);
    assert(ui.acknowledge());
    assert(!ui.submit(false)); // unchanged presentation produces no commands

    // Failed delivery must not poison the applied cache; replay full state.
    ui.text("root/title", L"third");
    assert(ui.submit(false));
    complete(shared, UI_FAILED_UPDATE);
    assert(ui.acknowledge());
    assert(ui.submit(false) && shared.count == 1);
    complete(shared);
    ui.acknowledge();
    assert(ui.submit(true) && shared.count == 0); // health probe has no setters
    complete(shared);
    ui.acknowledge();

    // Aggregation never scans game memory; multiple settlements sum cached data.
    ProductionSnapshot snapshot;
    snapshot.generation = 1;
    snapshot.player = 0;
    snapshot.settlements[3].name = "A";
    snapshot.settlements[3].population = 200;
    snapshot.settlements[3].produced["SaltFish"] = {2, 1};
    snapshot.settlements[7].name = "B";
    snapshot.settlements[7].population = 200;
    snapshot.settlements[7].produced["SaltFish"] = {3, 3};
    auto lines = production_lines(snapshot, {3, 7});
    bool found = false;
    for (const auto& line : lines)
        if (line.find(L"SaltFish") != std::wstring::npos) {
            assert(line.find(L"Need: 2  Produced: 5 (4/5 active)") != std::wstring::npos);
            found = true;
        }
    assert(found);
    snapshot.settlements[3].potential["SaltFish"] = 2.0;
    snapshot.settlements[7].potential["SaltFish"] = 6.0;
    snapshot.settlements[3].current["SaltFish"] = 1.0; // 50% of 2 t/min
    snapshot.settlements[7].current["SaltFish"] = 4.5; // 75% of 6 t/min
    snapshot.settlements[3].potential["Wood"] = 1.5;
    auto rates = product_rows(snapshot, {3, 7});
    bool fishRate = false, unknownDemand = false;
    for (const auto& row : rates) {
        if (row.name == "SaltFish") {
            assert(row.known && row.demand == 4.0 && row.potential == 8.0);
            assert(row.current == 5.5); // Sum per-building rates, not average percentages.
            fishRate = true;
        }
        if (row.name == "Wood") { assert(row.known && row.demand == 0 && row.potential == 1.5); unknownDemand = true; }
    }
    assert(fishRate && unknownDemand);
    productFilter = 1;
    for (const auto& row : product_rows(snapshot, {3,7})) assert(good_group(row.name) == 1);
    productFilter = 2;
    auto constructionRows = product_rows(snapshot, {3,7});
    assert(constructionRows.size() == 1 && constructionRows[0].name == "Wood");
    productFilter = 3;
    assert(product_rows(snapshot, {3,7}).empty());
    productFilter = 0;
    // Orient demand works without any peasants; missing producers still get rows.
    ProductionSnapshot orient;
    orient.settlements[16].tiers[4] = 72;
    orient.globalTiers[4] = 72;
    auto orientRows = product_rows(orient, {16});
    assert(orientRows.size() == 1 && orientRows[0].name == "Dates");
    assert(std::fabs(orientRows[0].demand - 0.48) < 1e-9 && orientRows[0].known);
    orient.settlements[17].tiers[5] = 600;
    orient.globalTiers[5] = 600;
    orient.settlements[17].inputs["Flour"] = 4;
    orient.settlements[17].potentialInputs["Flour"] = 8;
    orientRows = product_rows(orient, {16, 17});
    bool dates = false, flour = false, milk = false;
    for (const auto& row : orientRows) {
        if (row.name == "Dates") { assert(std::fabs(row.demand - 3.48) < 1e-9); dates = true; }
        if (row.name == "Flour") { assert(row.demand == 4 && row.potentialDemand == 8 && row.potential == 0); flour = true; }
        if (row.name == "Milk") { assert(std::fabs(row.demand - 600.0 / 666 * 1.5) < 1e-9); milk = true; }
    }
    assert(dates && flour && milk);
    // Global unlocks are evaluated before selection; combining islands cannot unlock a need.
    orient.globalTiers[4] = 146;
    orientRows = product_rows(orient, {16});
    assert(orientRows.size() == 2);
    for (const auto& rate : demandRates) {
        int all[11], unlocked[11];
        std::fill(all, all + 11, 5000); std::fill(unlocked, unlocked + 11, 5000);
        assert(resident_demand(rate, all, unlocked) > 0);
        int empty[11] = {};
        assert(resident_demand(rate, empty, unlocked) == 0);
    }
    assert(bar_fill(2, 3) == 6667 && bar_fill(3, 3) == 10000);
    assert(bar_fill(0, 0) == 0 && bar_fill(0, 3) == 0);
    assert(GoodsIcon("SaltFish") == 607 && GoodsIcon("Cider") == 8084);
    present(ui, snapshot, {3, 7}, {3});
    assert(ui.submit(false) && shared.count == UI_CAPACITY); // large grids stream bounded batches
    assert(shared.rowIslands[0] == 3 && shared.rowIslands[1] == 7);
    unsigned checkedButtons = 0;
    for (uint32_t i = 0; i < shared.count; ++i) {
        const UiUpdate& u = shared.updates[i];
        if (!(u.flags & UI_CHECKED)) continue;
        ++checkedButtons;
        if (strstr(u.target, "ROW01")) {
            assert(u.flags & UI_CHECKED_ON);
            assert(wcscmp((wchar_t*)u.text, L"A") == 0);
        } else if (strstr(u.target, "FILTER0")) assert(u.flags & UI_CHECKED_ON);
        else assert(!(u.flags & UI_CHECKED_ON));
    }
    assert(checkedButtons == 23);
    complete(shared); ui.acknowledge();
    // Even identical text must publish a changed displayed-player identity.
    unsigned gridBatches = 1;
    while (ui.submit(false)) { assert(shared.count <= UI_CAPACITY); complete(shared); ui.acknowledge(); ++gridBatches; }
    assert(gridBatches > 1);
    ui.selectionMap(1, {3, 7});
    assert(ui.submit(false) && shared.count == 0);
    complete(shared); ui.acknowledge();
    assert(shared.displayedPlayer == 1);
    present(ui, snapshot, {3, 7}, {7});
    assert(ui.submit(false) && shared.count < 60); // no full selector refresh
    bool uncheckedA = false, checkedB = false;
    for (uint32_t i = 0; i < shared.count; ++i) {
        const UiUpdate& u = shared.updates[i];
        if (!(u.flags & UI_CHECKED)) continue;
        if (strstr(u.target, "ROW01")) uncheckedA = !(u.flags & UI_CHECKED_ON);
        if (strstr(u.target, "ROW02")) checkedB = (u.flags & UI_CHECKED_ON) != 0;
    }
    assert(uncheckedA && checkedB);
    complete(shared); ui.acknowledge();
    present(ui, snapshot, {3, 7}, {7});
    assert(!ui.submit(false)); // repeated single-selection click stays checked
    present(ui, snapshot, {3, 7}, {3, 7});
    assert(ui.submit(false));
    for (uint32_t i = 0; i < shared.count; ++i)
        if (shared.updates[i].flags & UI_CHECKED)
            assert(shared.updates[i].flags & UI_CHECKED_ON);
    complete(shared); ui.acknowledge();

    // IDs captured by the GUI survive reordered lists; queued Ctrl-clicks survive.
    std::vector<int> selected;
    bool manual = false;
    uint32_t cursor = 0;
    shared.clicks[0] = {1, 0, 3, 0};
    shared.clicks[1] = {2, 1, 7, 0};
    shared.clickWrite = 2;
    assert(consume_selection(&shared, {7, 3}, 0, selected, manual, cursor));
    assert(manual && selected == std::vector<int>({3, 7}));
    shared.clicks[2] = {1, 1, 3, 0};
    shared.clickWrite = 3;
    consume_selection(&shared, {7, 3}, 0, selected, manual, cursor);
    assert(selected == std::vector<int>({7}));
    shared.clicks[3] = {1, 0, 3, 1}; // stale click from another player
    shared.clickWrite = 4;
    consume_selection(&shared, {7, 3}, 0, selected, manual, cursor);
    assert(selected == std::vector<int>({7}));

    // Unsigned queue sequence wrap and bounded overflow recovery.
    selected.clear(); manual = false;
    cursor = UINT32_MAX;
    shared.clicks[UINT32_MAX % UI_CLICK_CAPACITY] = {1, 0, 3, 0};
    shared.clickWrite = 0;
    consume_selection(&shared, {3, 7}, 0, selected, manual, cursor);
    assert(cursor == 0 && selected == std::vector<int>({3}));
    cursor = 0;
    for (uint32_t n = 6; n < 70; ++n)
        shared.clicks[n % UI_CLICK_CAPACITY] = {1, 0, n == 69 ? 7 : 3, 0};
    shared.clickWrite = 70;
    consume_selection(&shared, {3, 7}, 0, selected, manual, cursor);
    assert(cursor == 70 && selected == std::vector<int>({7}));

    // Reopening discards a previous multiselection and uses the captured hover.
    selected = {3, 7}; manual = true; cursor = 0;
    shared.clicks[0] = {UI_DIALOG_CLOSE, 0, 0, 0};
    shared.clicks[1] = {UI_DIALOG_OPEN, 0, 7, 0};
    shared.clickWrite = 2;
    consume_selection(&shared, {3, 7}, 0, selected, manual, cursor);
    assert(manual && selected == std::vector<int>({7}));
    shared.clicks[2] = {UI_DIALOG_OPEN, 0, 0, 0}; // water: no stale selection
    shared.clickWrite = 3;
    consume_selection(&shared, {3, 7}, 0, selected, manual, cursor);
    assert(manual && selected.empty());
    shared.clicks[3] = {UI_DIALOG_OPEN, 0, 3, 1}; // another player's captured hover
    shared.clickWrite = 4;
    consume_selection(&shared, {3, 7}, 0, selected, manual, cursor);
    assert(manual && selected.empty());

    // Frame margins and inner padding survive different resolutions, including
    // all 19 settlement rows at 720p and 600px tall windows.
    for (auto dimensions : {std::pair<int, int>{1920, 1080}, {1280, 1024}, {1280, 720}, {800, 600}}) {
        for (int count : {2, 19}) {
            auto layout = MakeDialogLayout(dimensions.first, dimensions.second, count);
            int frameWidth = UiMin(1280, dimensions.first - 20);
            int frameHeight = UiMin(1024, dimensions.second - 20);
            assert(layout.frame.left == (dimensions.first - frameWidth) / 2);
            assert(layout.frame.top == (dimensions.second - frameHeight) / 2);
            assert(layout.frame.right - layout.frame.left == frameWidth);
            assert(layout.frame.bottom - layout.frame.top == frameHeight);
            assert(layout.title.left == 30 && layout.title.top == 30);
            assert(layout.close.right == frameWidth - 30);
            assert(layout.close.bottom - layout.close.top == 48);
            assert(layout.settlements[0].bottom - layout.settlements[0].top == 48);
            assert(layout.help.right == frameWidth - 30);
            assert(layout.settlements[count - 1].bottom <= frameHeight - 30);
            assert(layout.production[13].bottom <= frameHeight - 30);
            assert(layout.production[0].left > layout.settlements[0].right);
        }
    }

    // Large desired state drains exactly once across acknowledged batches.
    ui.clear();
    for (unsigned i = 0; i <= UI_CAPACITY; ++i)
        ui.text("target" + std::to_string(i), L"text");
    assert(ui.submit(false) && shared.count == UI_CAPACITY);
    complete(shared); ui.acknowledge();
    assert(ui.submit(false) && shared.count == 1);
    complete(shared); ui.acknowledge();
    assert(!ui.submit(false));
    puts("UI transport and presentation tests passed");
    return 0;
}
