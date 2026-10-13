#pragma once
#include <windows.h>
#include <stdint.h>

// Independent, versioned transport. The legacy 4096-byte bridge stays intact.
// One producer (companion), one consumer (game GUI thread). Never overwrite a
// submitted batch until the consumer acknowledges it by returning state to 0.
#ifdef ANNORE_HISTORY64
#define UI_MAP_NAME "AnnoREHistoryUiV4"
#define UI_DONE_NAME "AnnoREHistoryUiV4Done"
#else
#define UI_MAP_NAME "AnnoREUiV4"
#define UI_DONE_NAME "AnnoREUiV4Done"
#endif
#define UI_MAGIC 0x31554941u
#define UI_VERSION 4u
#define UI_CAPACITY 192u
#define UI_CLICK_CAPACITY 64u
#define UI_TEXT 1u
#define UI_VISIBLE 2u
#define UI_CHECKED 4u
#define UI_CHECKED_ON 8u // value bit; valid only with UI_CHECKED
#define UI_RECT 16u
#define UI_BAR 32u
#define UI_ICON 64u
#define UI_PREVIOUS -2
#define UI_NEXT -3
#define UI_DIALOG_OPEN 0
#define UI_DIALOG_CLOSE -1
#define UI_MISSING_DIALOG 1u
#define UI_MISSING_BUTTON 2u
#define UI_FAILED_UPDATE 4u

#pragma pack(push, 4)
struct UiUpdate {
    char target[64];
    uint16_t text[128];
    uint32_t flags;
    int32_t visible;
    int32_t barValue, barMax, icon;
    int32_t rect[4]; // parent-relative left/top/right/bottom
};
struct UiClick { int32_t row; uint32_t ctrl; int32_t island, player; uint32_t tick; };
struct UiShared {
    uint32_t magic, version, size, ownerPid;
    volatile LONG state; // 0 idle/acknowledged, 1 submitted, 2 executing
    uint32_t count, result, sequence;
    volatile LONG clickWrite;
    volatile LONG viewportWidth, viewportHeight;
    int32_t player, displayedPlayer;
    int32_t rowIslands[19], displayedIslands[19];
    UiClick clicks[UI_CLICK_CAPACITY];
    UiUpdate updates[UI_CAPACITY];
};
#pragma pack(pop)
static_assert(sizeof(UiUpdate) == 356, "UI wire layout changed");
static_assert(sizeof(UiShared) == 69836, "UI wire layout changed");
