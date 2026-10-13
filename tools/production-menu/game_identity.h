#pragma once
#include <windows.h>
#include <bcrypt.h>
#include <stdint.h>
#include <string.h>
#pragma comment(lib, "bcrypt.lib")

// Identification is separate from support. A known History binary must never
// silently fall back to the Gold32 reader or the Gold32 GUI hook.
enum class AnnoEdition { Unknown, Gold32, History64 };
inline AnnoEdition IdentifyAnnoFile(const wchar_t* path) {
    HANDLE file = CreateFileW(path, GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                              NULL, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL);
    if (file == INVALID_HANDLE_VALUE) return AnnoEdition::Unknown;
    BCRYPT_ALG_HANDLE algorithm = NULL;
    BCRYPT_HASH_HANDLE hash = NULL;
    uint8_t digest[32] = {}, chunk[65536];
    bool ok = BCryptOpenAlgorithmProvider(&algorithm, BCRYPT_SHA256_ALGORITHM, NULL, 0) >= 0;
    if (ok) ok = BCryptCreateHash(algorithm, &hash, NULL, 0, NULL, 0, 0) >= 0;
    DWORD count = 0;
    while (ok) {
        if (!ReadFile(file, chunk, sizeof(chunk), &count, NULL)) { ok = false; break; }
        if (!count) break;
        if (BCryptHashData(hash, chunk, count, 0) < 0) ok = false;
    }
    if (ok) ok = BCryptFinishHash(hash, digest, sizeof(digest), 0) >= 0;
    if (hash) BCryptDestroyHash(hash);
    if (algorithm) BCryptCloseAlgorithmProvider(algorithm, 0);
    CloseHandle(file);
    static const uint8_t gold[32] = {
        0x7a,0x16,0x93,0xfb,0x5b,0x46,0xe2,0x4d,0x74,0x70,0xbf,0x32,0x8e,0x52,0x83,0xae,
        0x1b,0x06,0x65,0x28,0x4f,0x5c,0xcc,0x4b,0x22,0xbf,0x38,0xb9,0xdf,0xdf,0x40,0x07};
    static const uint8_t history[32] = {
        0x53,0xcc,0x20,0xb9,0xec,0x1e,0xde,0x15,0xc8,0xa7,0x08,0xd5,0x2b,0xc2,0xd0,0x53,
        0xc2,0x5f,0x7c,0x6c,0x12,0xb5,0x9e,0x6b,0xec,0xc7,0x3c,0x46,0xe7,0x9c,0x51,0x42};
    if (!ok) return AnnoEdition::Unknown;
    if (memcmp(digest, gold, sizeof(gold)) == 0) return AnnoEdition::Gold32;
    if (memcmp(digest, history, sizeof(history)) == 0) return AnnoEdition::History64;
    return AnnoEdition::Unknown;
}

inline bool IsVerifiedGoldProcess(HANDLE process) {
    wchar_t path[32768];
    DWORD length = sizeof(path) / sizeof(path[0]);
    if (!QueryFullProcessImageNameW(process, 0, path, &length)) return false;
    const wchar_t* name = wcsrchr(path, L'\\');
    if (_wcsicmp(name ? name + 1 : path, L"Addon.exe") != 0 ||
        IdentifyAnnoFile(path) != AnnoEdition::Gold32) return false;
    // Check the loaded PE at the fixed GOG base too, before following offsets.
    uint8_t header[4096] = {};
    SIZE_T got = 0;
    if (!ReadProcessMemory(process, (const void*)0x400000, header, sizeof(header), &got) ||
        got != sizeof(header) || header[0] != 'M' || header[1] != 'Z') return false;
    uint32_t pe = 0;
    memcpy(&pe, header + 60, sizeof(pe));
    if (pe > sizeof(header) - 96) return false;
    uint32_t signature = 0, imageBase = 0;
    uint16_t machine = 0, magic = 0, flags = 0;
    memcpy(&signature, header + pe, 4);
    memcpy(&machine, header + pe + 4, 2);
    memcpy(&magic, header + pe + 24, 2);
    memcpy(&imageBase, header + pe + 52, 4);
    memcpy(&flags, header + pe + 94, 2);
    return signature == 0x4550 && machine == 0x14c && magic == 0x10b &&
        imageBase == 0x400000 && !(flags & 0x40);
}
