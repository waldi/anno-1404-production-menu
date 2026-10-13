#define WIN32_LEAN_AND_MEAN
#include "game_identity.h"
#include <stdio.h>
int wmain(int argc, wchar_t** argv) {
    wchar_t self[MAX_PATH];
    if (!GetModuleFileNameW(NULL, self, MAX_PATH) || IdentifyAnnoFile(self) != AnnoEdition::Unknown) return 1;
    if (IdentifyAnnoFile(L"this-file-does-not-exist.exe") != AnnoEdition::Unknown) return 2;
    if (argc > 1 && IdentifyAnnoFile(argv[1]) != AnnoEdition::Gold32) return 3;
    if (argc > 2 && IdentifyAnnoFile(argv[2]) != AnnoEdition::History64) return 4;
    // Our test executable is unknown, so no version-specific process read is allowed.
    if (IsVerifiedGoldProcess(GetCurrentProcess())) return 5;
    puts("Game fingerprint identification and unsupported-process rejection passed");
    return 0;
}
