// annobridge - show an externally supplied number in Anno 1404's own GUI.
//
// v2 adds runtime widget creation: the DLL asks the game's own factory to build a
// native widget and parent it into an existing frame. No .dlg, no /compilegui, no
// RDA repacking.
// v6 adds three game calls on a named element, from docs/gui-rendering.md:
//   OnCreate       (vtable +0x58)  -- builds the BACKGROUND object so real art renders
//   SetText        (TextSetting_SetTextW / GuiWndLabel_SetTextW) -- arbitrary text
//   SetBackground  (vtable +0xBC)  -- give an element a named background layout
//
// v8 adds the XML route (docs/gui-rendering.md "The XML layer at runtime"):
//   --xml FILE      build a whole dialog subtree from a .dlg fragment through the game's
//                   own CXMLExtNode / CConfigDocTemplate objects and
//                   GuiElement_InstantiateXmlSubtree (0x00DBA940)
//   --rebuild-paths GuiRegistry_RebuildPathMap, so game-created children get full paths
//   --skin-index    now goes through GuiWnd_ApplyTemplateByIndex (0x00D6DE00), which
//                   saves/restores the names and rebuilds the background itself
//
// Loads into Addon.exe as a proxy d3d9.dll, hooks the GUI event dispatcher, and
// drives a widget matched by ACCESSID *or* PATHID. Values come from a named
// shared-memory block any external process can write.
//
// THIS IS NOT READ-ONLY. It runs inside the game process and writes to game
// memory, and calls game functions. See AGENTS.md and docs/gui-system.md.
//
// Offsets are valid verbatim: Addon.exe has no ASLR, image base 0x400000.

#include <windows.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "ui_protocol.h"
#include "game_identity.h"
static_assert(sizeof(void*) == 4, "Gold GUI bridge requires the x86 toolset; History needs its own bridge");

static UiShared* g_ui = NULL;
static HANDLE g_uiMap = NULL, g_uiDone = NULL;
static void* g_observedDialog = NULL;
static bool g_dialogWasVisible = false;

// ---- forwarded d3d9 exports -------------------------------------------------
#define FWD(n) __pragma(comment(linker, "/export:" #n "=C:\\Windows\\SysWOW64\\d3d9." #n))
FWD(Direct3DCreate9)
FWD(Direct3DCreate9Ex)
FWD(D3DPERF_BeginEvent)
FWD(D3DPERF_EndEvent)
FWD(D3DPERF_GetStatus)
FWD(D3DPERF_QueryRepeatFrame)
FWD(D3DPERF_SetMarker)
FWD(D3DPERF_SetOptions)
FWD(D3DPERF_SetRegion)
FWD(DebugSetLevel)
FWD(DebugSetMute)
FWD(Direct3DShaderValidatorCreate9)

// ---- game addresses (Addon.exe, image base 0x400000, no ASLR) ---------------
#define ADDR_DISPATCH      0x00D6C0D0  // GuiElement_DispatchNamedEvent
#define ADDR_CREATE        0x00DB72C0  // GuiRuntime_CreateElement  (__stdcall)
#define ADDR_REGISTRY_GET  0x00418FE0  // GuiRegistry_Get -> singleton 0x012D8F78
#define ADDR_FIND_BY_NAME  0x00D51140  // GuiRegistry_FindElementByName (__thiscall)
#define ADDR_STR_CTOR      0x00B09160  // StlpString_CtorFromCStr (__thiscall)
#define ADDR_STR_DTOR      0x00B088A0  // StlpString_Dtor (__thiscall)
#define ADDR_WSTR_CTOR     0x00B074C0  // wide string ctor from wchar_t* (__thiscall), 0x30-byte object
#define ADDR_WSTR_DTOR     0x00B06AE0  // wide string dtor (__thiscall)
#define ADDR_TEMPLATE_APPLY 0x00DB6740 // GuiTemplate_Apply (__thiscall: ecx=template)
#define ADDR_LABEL_SETTEXTW 0x00D7BC40 // GuiWndLabel_SetTextW(this=label, wstring*, char notify)
#define ADDR_TS_SETTEXTW    0x00D8BAB0 // TextSetting_SetTextW(this=TextSetting, wstring*, char notify)
#define ADDR_TS_SETFONT     0x00D8B1F0 // TextSetting_SetFont(this=TextSetting, CFontContainer*)
#define ADDR_TS_SETHALIGN   0x00D8B220 // TextSetting_SetHAlign(this, 0 left | 1 centre | 2 right)
#define ADDR_TS_SETVALIGN   0x00D8B270 // TextSetting_SetVAlign(this, 0 top | 4 middle | 8 bottom)
#define ADDR_TE_GETFONT     0x00D46310 // TextEngine_GetFontByName(this=g_pTextEngine, stlp_string* name) -> CFontContainer*
#define ADDR_G_TEXTENGINE   0x012D3038 // g_pTextEngine
// v8: the runtime XML layer (all __thiscall unless noted; RET sizes read off the disassembly)
#define ADDR_G_GUIMANAGER      0x012D334C // g_pGuiManager; +0x42C = CConfigDocContext*
#define ADDR_XMLDOC_CTOR_EMPTY 0x00484EC0 // XmlDoc_ConstructEmpty(doc, ctx)            RET 4
#define ADDR_XMLNODE_CTOR      0x0042CD80 // XmlNode_Construct(node)                    __fastcall(ecx)
#define ADDR_XMLNODE_INIT      0x0042C160 // XmlNode_InitWithName(node, name, pool)    RET 8
#define ADDR_XMLNODE_APPEND    0x0042C0D0 // XmlNode_AppendChild(parent, node)          RET 4
#define ADDR_INSTANTIATE_XML   0x00DBA940 // GuiElement_InstantiateXmlSubtree(parent, doc, idx)  __stdcall RET 0xC
#define ADDR_REBUILD_PATHS     0x00D53BF0 // GuiRegistry_RebuildPathMap(registry)      __fastcall(ecx)
#define ADDR_APPLY_TEMPLATE_IX 0x00D6DE00 // GuiWnd_ApplyTemplateByIndex(elem, idx)     RET 4
#define ADDR_REGISTER_ELEM     0x00D6DB10 // GuiWnd_RegisterInRegistry(elem)            __fastcall(ecx): all-elements list (path map source), ACCESSID map, GROUP
#define ADDR_FRAME_WRAP_MODAL  0x00D8C470 // GuiWndFrame_WrapInModalBackground(frame, char on)  RET 4: reparents a FRAME_MODAL/FRAME_POPUP frame into a CGUIWndModalBackground, as the .cdf loader does
#define TYPE_FRAME 11
// v8.4 tooltips: manager singleton DAT_01304660 (ToolTipManager_Get 0x0048C750). Per frame
// ToolTipManager_Update applies a pending request once (ToolTipManager_ApplyRequest: guid ->
// text -> ToolTipManager_SetText + ToolTipManager_LayoutStandard), records the show time at
// +0x64 and the current element at +0x60. Re-running SetText + LayoutStandard with our own
// wchar_t* right after that replaces the text for the rest of the show.
#define ADDR_TOOLTIP_MGR_PTR   0x01304660 // CToolTipManager*
#define ADDR_TOOLTIP_SETTEXT   0x00D8A870 // ToolTipManager_SetText(mgr, const wchar_t*)      __thiscall RET 4
#define ADDR_TOOLTIP_LAYOUT    0x00D89810 // ToolTipManager_LayoutStandard(mgr, request*)     __thiscall RET 4
#define TTM_CUR_ELEMENT 0x60
#define TTM_SHOW_TICK   0x64
#define TTM_REQUEST     0x70
#define XMLNODE_SIZE 0x5C                 // sizeof(CXMLExtNode<CRDString>), from XmlNodePool_Grow
#define XMLDOC_SIZE  0x20                 // CConfigDocTemplate<CRDString> is 0x1C; round up

// CGUIWnd base, from GuiElement_ReadDefinitionNode (0x00D6E1A0) / GuiWnd_Construct
#define OFF_VISIBLE   0x09C   // u8 visible flag (read back after SetVisible)
#define OFF_TYPE      0x0C4   // element type enum
#define OFF_PATHID    0x0C8   // char[0x41]
#define OFF_ACCESSID  0x109   // char[0x41]
#define OFF_BACKGROUND 0x199  // char[0x104] BACKGROUND layout name
#define OFF_EXTENT    0x2A4   // CRect {left, top, right, bottom} - CONFIRMED live
#define OFF_TEMPLATE_IDX 0x14C // index into the registry's template vector
#define OFF_BG_OBJECT 0x580   // CBackgroundLayout*; 0 = "flat rectangle" fallback

// vtable slots, verified across Label/ProgressBar/Frame/Desktop vtables
#define VT_ONCREATE       (0x58 / 4)
#define VT_SETBACKGROUND  (0xBC / 4)

// template vector on the registry singleton, from GuiElement_ReadDefinitionNode
#define REG_TMPL_BEGIN 0x1DC
#define REG_TMPL_END   0x1E0

#define TYPE_LABEL       1
#define TYPE_BUTTON      2
#define TYPE_SLIDER      3
#define TYPE_EDITFIELD   5
#define TYPE_PROGRESSBAR 9

#define OFF_PB_VALUE  0x6F0   // CURRENTVALUE, GuiWndProgressBar_ReadDefinitionNode
#define OFF_PB_MAX    0x6F4   // MAXVALUE
#define OFF_SL_VALUE  0x6F8   // CURRENTVALUE, GuiWndSlider_ReadDefinitionNode
#define OFF_SL_RES    0x6FC   // RESOLUTION

// live TextSetting per type (docs/gui-rendering.md)
#define OFF_TS_LABEL     0x8C8
#define OFF_TS_BUTTON    0xA70
#define OFF_TS_EDITFIELD 0x86C

// First 8 bytes of the dispatcher, verified before patching.
//   6A FF              PUSH -1
//   64 A1 00 00 00 00  MOV EAX, FS:[0]
static const uint8_t kPrologue[8] = { 0x6A, 0xFF, 0x64, 0xA1, 0x00, 0x00, 0x00, 0x00 };
#define STOLEN 8

// STLport std::string is 0x20 bytes; the wide string object is 0x30. 0x40 covers both.
#define STLSTR_SIZE 0x40

// ---- game function types ----------------------------------------------------
// __fastcall with a dummy EDX is the portable way to spell __thiscall.
typedef void* (__fastcall* tStrCtor)(void* self, void* edx, const char* src);
typedef void  (__fastcall* tStrDtor)(void* self, void* edx);
typedef void* (__fastcall* tWStrCtor)(void* self, void* edx, const wchar_t* src);
typedef void  (__fastcall* tWStrDtor)(void* self, void* edx);
// GuiRegistry_FindElementByName is __thiscall with TWO stack args and `RET 8`: the
// name and a second dword the only call site (0x00DB72EA) passes as 0. Calling it
// with one arg left ESP off by 4 and crashed the caller's epilogue (v6 crash #2).
typedef void* (__fastcall* tFindByName)(void* self, void* edx, void* nameStr, int zero);
typedef void* (__cdecl*    tRegistryGet)(void);
// CONFIRMED __stdcall: the only call site (0x00737544) pushes 3 args and does not
// clean the stack afterwards.
typedef int   (__stdcall*  tCreateElement)(int type, void* parentName, void* newName);
typedef void  (__fastcall* tTemplateApply)(void* tmpl, void* edx, void* element);
typedef void  (__fastcall* tSetTextW)(void* self, void* edx, void* wstr, char notify);
typedef void  (__fastcall* tVoidThis)(void* self, void* edx);
typedef uint32_t (__fastcall* tSetBackground)(void* self, void* edx, void* str);
typedef void* (__fastcall* tXmlDocCtorEmpty)(void* doc, void* edx, void* ctx);
typedef void* (__fastcall* tXmlNodeCtor)(void* node, void* edx);
typedef void  (__fastcall* tXmlNodeInit)(void* node, void* edx, const char* name, void* pool);
typedef void  (__fastcall* tXmlNodeAppend)(void* parent, void* edx, void* node);
typedef void  (__stdcall*  tInstantiateXml)(void* parentElem, void* doc, int structIdx);
typedef void  (__fastcall* tRebuildPaths)(void* registry, void* edx);
typedef void  (__fastcall* tApplyTemplateIdx)(void* elem, void* edx, int idx);
typedef void  (__fastcall* tRegisterElem)(void* elem, void* edx);
typedef void  (__fastcall* tFrameWrapModal)(void* frame, void* edx, char on);
typedef void  (__fastcall* tTooltipSetText)(void* mgr, void* edx, const wchar_t* text);
typedef void  (__fastcall* tTooltipLayout)(void* mgr, void* edx, void* request);

static const tStrCtor       gStrCtor     = (tStrCtor)ADDR_STR_CTOR;
static const tStrDtor       gStrDtor     = (tStrDtor)ADDR_STR_DTOR;
static const tWStrCtor      gWStrCtor    = (tWStrCtor)ADDR_WSTR_CTOR;
static const tWStrDtor      gWStrDtor    = (tWStrDtor)ADDR_WSTR_DTOR;
static const tFindByName    gFindByName  = (tFindByName)ADDR_FIND_BY_NAME;
static const tRegistryGet   gRegistryGet = (tRegistryGet)ADDR_REGISTRY_GET;
static const tCreateElement gCreate      = (tCreateElement)ADDR_CREATE;
static const tTemplateApply gTemplateApply = (tTemplateApply)ADDR_TEMPLATE_APPLY;
static const tSetTextW      gLabelSetTextW = (tSetTextW)ADDR_LABEL_SETTEXTW;
static const tSetTextW      gTsSetTextW    = (tSetTextW)ADDR_TS_SETTEXTW;
typedef void  (__fastcall* tTsSetInt)(void* self, void* edx, int v);
typedef void* (__fastcall* tGetFont)(void* self, void* edx, void* nameStr);
static const tTsSetInt gTsSetFont   = (tTsSetInt)ADDR_TS_SETFONT;
static const tTsSetInt gTsSetHAlign = (tTsSetInt)ADDR_TS_SETHALIGN;
static const tTsSetInt gTsSetVAlign = (tTsSetInt)ADDR_TS_SETVALIGN;
// FUN_00418950: stlp_string::assign(begin, end)  __thiscall, RET 8 (verified in disassembly)
typedef void* (__fastcall* tStrAssign)(void* self, void* edx, const char* begin, const char* end);
static const tStrAssign gStrAssign = (tStrAssign)0x00418950;

// TextSetting offset for the element types that have one; 0 otherwise.
static uint32_t TextSettingOffset(uint32_t type) {
    if (type == 1) return 0x8C8;    // LABEL
    if (type == 2) return 0xA70;    // BUTTON
    if (type == 5) return 0x86C;    // EDITFIELD
    return 0;
}

// ---- shared memory ----------------------------------------------------------
#define BRIDGE_NAME    "AnnoREBridge"
#define BRIDGE_SIZE    4096
#define BRIDGE_MAGIC   0x4F4E4E41u   // 'ANNO'
#define BRIDGE_VERSION 8
#define COMPANION_EXE "AnnoRECompanion.exe"

#define CREATE_IDLE      0
#define CREATE_REQUESTED 1
#define CREATE_DONE      2
#define CREATE_NO_PARENT 3
#define CREATE_BAD_TYPE  4
#define CREATE_FAILED    5

// v6 element operations
#define OP_NONE          0
#define OP_ONCREATE      1
#define OP_SETTEXT       2
#define OP_SETBACKGROUND 3
#define OP_SETVISIBLE    4   // opArg[0] == '1' shows, anything else hides
#define VT_SETVISIBLE    (0xAC / 4)
// GuiWnd_SetVisible(this, int visible, char fireEvents, char force): `visible` is a full dword that
// is stored at +0x9C and compared with == 1 everywhere (RenderTree, IsVisible, layout). Passing a
// char left register garbage in the upper bytes (+0x9C read 0x010A6101 on 2026-09-26) and the
// element silently stopped rendering. Pass an int.
typedef void (__fastcall* tSetVisible)(void* self, void* edx, int visible, char b, char c);

#define OPST_IDLE     0
#define OPST_PENDING  1
#define OPST_DONE     2
#define OPST_FAILED   3
#define OPST_BADTYPE  4   // element type has no TextSetting we know
#define OPST_WAITSRC  5   // "@OTHER" background source not seen yet

#pragma pack(push, 4)
struct Bridge {
    uint32_t magic;
    uint32_t version;
    int32_t  value;          // what we display
    int32_t  maxValue;       // widget range; <= 0 leaves the game's own
    uint32_t writes;         // producer increments
    uint32_t hits;           // we increment on every widget update
    uint32_t lastType;       // element type we last matched
    char     targetName[64]; // matched against ACCESSID *or* PATHID

    uint32_t createRequest;  // producer sets CREATE_REQUESTED
    uint32_t createStatus;   // we report CREATE_*
    int32_t  createType;     // element type enum to create (9 = PROGRESSBAR)
    char     parentName[64]; // existing element to parent under, e.g. "INFOBAR"
    char     newName[64];    // PATHID for the new element
    int32_t  rect[4];        // CRect {left,top,right,bottom} -> element+0x2A4
    uint32_t applyRect;      // non-zero: push rect[] every frame

    // --- v3: introspection -------------------------------------------------
    char     inspectName[64];// element to dump (ACCESSID or PATHID)
    uint32_t dumpOffset;     // byte offset into that element
    uint32_t dumpCount;      // dwords to copy, max 64
    uint32_t inspectPtr;     // the element pointer we found
    uint32_t inspectHits;    // increments each time we dump
    int32_t  dumpOut[64];

    // --- v3: skinning ------------------------------------------------------
    int32_t  skinIndex;      // template index to apply to targetName
    uint32_t skinRequest;    // producer sets 1
    uint32_t skinStatus;     // 0 idle 1 requested 2 done 3 bad index 4 failed

    // --- v4: arbitrary field writes on the target, applied every frame -------
    // Deliberately general: ICONGUID, BARCOLOR, flag words, anything, without a
    // rebuild. Offsets are unchecked beyond a sanity bound - see the README.
    struct { uint32_t enabled; uint32_t offset; int32_t value; } pokes[4];   // ends at 648

    // --- v6: element operations -------------------------------------------
    uint32_t opRequest;      // OP_*; producer sets, we clear when done
    uint32_t opStatus;       // OPST_*
    char     opTarget[64];   // element to act on (ACCESSID or PATHID)
    char     opArg[260];     // SetBackground: layout name (paths are ~90 chars!), or "@OTHER" to copy OTHER's +0x199
    uint16_t opText[128];    // SetText: UTF-16, NUL-terminated
    uint32_t opResult;       // last return value / bg-object pointer for the record
    char     listOut[512];   // OP_LISTCHILDREN: "PATHID|ACCESSID|type|visible;" per child
    char     rectTarget[64]; // v6.1: the element rect[] belongs to (empty = targetName)

    // --- v7: click -> show/hide ------------------------------------------------
    char     toggleButton[64]; // element whose OnLeftClick we react to (ACCESSID or PATHID)
    char     toggleTarget[64]; // element to show/hide (full path allowed)
    uint32_t toggleCount;      // clicks handled
    char     lastEvent[32];    // last non-OnRender event name seen on toggleButton
    uint32_t traceEvents;      // v7.1: non-zero -> log every non-render event with its element

    // --- v8.4: tooltip text override ------------------------------------------------
    char     tipTarget[64];    // element (ACCESSID/PATHID/full path) whose tooltip text we replace
    uint16_t tipText[128];     // UTF-16 replacement, NUL-terminated
    uint32_t tipCount;         // times applied
    uint32_t tipMode;          // 1 = ToolTipManager_SetText only, 2 = SetText + LayoutStandard (crashed the game once, 2026-09-26)
};
#define OP_LISTCHILDREN  5
#define OP_SETFONT       6   // opArg = font name from localisation_addon.xml (e.g. "Tooltip")
#define OP_SETALIGN      7   // opArg = "h,v" with h 0/1/2 and v 0/4/8
#define OP_LOADSAVE      8   // v7: opText = full path of a .sww; no target element needed
#define OP_CLICK         9   // v7: dispatch "OnLeftClick" to the target, as the mouse would
#define OP_XML           10  // v8: opArg = path of a .dlg fragment, opTarget = parent element
#define OP_REBUILDPATHS  11  // v8: GuiRegistry_RebuildPathMap; no target element needed

// Shared-memory settlement-row click payload. Kept in unused bridge space so the
// external companion's compact Bridge view remains layout-compatible.
#define O_SELECTION_SEQ 2400
#define O_SELECTION_ROW 2404
#define O_SELECTION_CTRL 2408

// v7: MainControl_LoadSavegameFile(CMainControl*, stlp_wstring*) -- __stdcall, RET 8
// (bytes at 0x004B065E: c2 08 00). Posts a load message to the main-control queue
// under a critical section; the load itself happens later on the main loop.
#define ADDR_LOADSAVE        0x004B0320
#define ADDR_MAINCONTROL_PTR 0x012D92CC   // CMainControl* g_pMainControl
typedef void (__stdcall* tLoadSave)(void* mainControl, void* wpath);
#pragma pack(pop)

static Bridge*  g_bridge = NULL;
static HANDLE   g_map = NULL;
static uint8_t* g_tramp = NULL;

typedef char(__fastcall* tDispatch)(void* self, void* edx, void* evName, void* args, char defRes);
static tDispatch g_orig = NULL;

// ---- logging ----------------------------------------------------------------
static void logf(const char* fmt, ...) {
    char path[MAX_PATH];
    if (!GetTempPathA(MAX_PATH, path)) return;
    strncat_s(path, MAX_PATH, "annobridge.log", _TRUNCATE);
    FILE* f = NULL;
    if (fopen_s(&f, path, "a") != 0 || !f) return;
    SYSTEMTIME t; GetLocalTime(&t);
    fprintf(f, "[%02d:%02d:%02d] ", t.wHour, t.wMinute, t.wSecond);
    va_list ap; va_start(ap, fmt); vfprintf(f, fmt, ap); va_end(ap);
    fputc('\n', f);
    fclose(f);
}

// ---- element size table ------------------------------------------------------
// Straight from GuiRuntime_CreateElement (0x00DB72C0): the malloc size per type.
// EVERY read and write below is clamped to this. Writing past it corrupts the
// heap, which surfaces much later as an access violation inside ntdll.
static uint32_t ElementSize(uint32_t type) {
    switch (type) {
        case 1:  return 0x924;  // LABEL
        case 2:  return 0xAAC;  // BUTTON
        case 3:  return 0x7BC;  // SLIDER
        case 5:  return 0x8A8;  // EDITFIELD
        case 6:  return 0x704;  // BOXLAYOUT
        case 7:  return 0xBF0;  // COMBOBOX
        case 8:  return 0x84C;  // LISTBOX
        case 9:  return 0x6FC;  // PROGRESSBAR
        case 10: return 0x738;  // CARDLAYOUT
        case 12: return 0x920;  // RENDERTARGET
        case 13: return 0x714;  // ALPHAPROGRESSBAR
        case 15: return 0x938;  // VIDEOPANEL
        case 20: return 0x6F8;  // STRUCTTEMPLATEINSTANCE
        default: return 0;      // unknown -> refuse everything
    }
}

// True only if [off, off+len) lies inside an element of this type.
static bool InBounds(uint32_t type, uint32_t off, uint32_t len) {
    uint32_t sz = ElementSize(type);
    return sz != 0 && off + len <= sz && (off + len) >= off;
}

// ---- small helpers ----------------------------------------------------------
struct ScopedStr {
    __declspec(align(8)) uint8_t buf[STLSTR_SIZE];
    ScopedStr(const char* s) { memset(buf, 0, sizeof(buf)); gStrCtor(buf, NULL, s); }
    ~ScopedStr() { gStrDtor(buf, NULL); }
    void* operator&() { return buf; }
};

struct ScopedWStr {
    __declspec(align(8)) uint8_t buf[STLSTR_SIZE];
    ScopedWStr(const wchar_t* s) { memset(buf, 0, sizeof(buf)); gWStrCtor(buf, NULL, s); }
    ~ScopedWStr() { gWStrDtor(buf, NULL); }
    void* operator&() { return buf; }
};

static const char* ElemPathId(void* e)   { return (const char*)((uint8_t*)e + OFF_PATHID); }
static const char* ElemAccessId(void* e) { return (const char*)((uint8_t*)e + OFF_ACCESSID); }
static uint32_t    ElemType(void* e)     { return *(uint32_t*)((uint8_t*)e + OFF_TYPE); }

// Matches on ACCESSID (existing game widgets) or PATHID (ones we created, which
// get a PATHID but no ACCESSID).
static bool NameMatches(void* e, const char* want) {
    if (!want || !want[0]) return false;
    return strncmp(ElemAccessId(e), want, 0x40) == 0
        || strncmp(ElemPathId(e),   want, 0x40) == 0;
}

// ---- the panel ---------------------------------------------------------------
static void DoCreate(void) {
    if (!g_bridge) return;
    if (!g_bridge->parentName[0] || !g_bridge->newName[0]) {
        g_bridge->createStatus = CREATE_FAILED;
        g_bridge->createRequest = CREATE_IDLE;
        logf("create: parentName/newName not set");
        return;
    }

    // Clear the request BEFORE calling the factory: it dispatches GUI events while
    // attaching the child, which re-enters this hook. v5 created the same widget
    // hundreds of times that way and eventually took the game down.
    g_bridge->createRequest = CREATE_IDLE;
    g_bridge->createStatus = 0;

    int type = g_bridge->createType ? g_bridge->createType : TYPE_PROGRESSBAR;
    int rc = -1;
    __try {
        ScopedStr parent(g_bridge->parentName);
        ScopedStr name(g_bridge->newName);
        rc = gCreate(type, &parent, &name);
    } __except (EXCEPTION_EXECUTE_HANDLER) {
        g_bridge->createStatus = CREATE_FAILED;
        g_bridge->createRequest = CREATE_IDLE;
        logf("create: EXCEPTION calling the factory");
        return;
    }

    switch (rc) {
        case 0:    g_bridge->createStatus = CREATE_DONE;      break;
        case 0xC:  g_bridge->createStatus = CREATE_NO_PARENT; break;
        case 3:    g_bridge->createStatus = CREATE_BAD_TYPE;  break;
        default:   g_bridge->createStatus = CREATE_FAILED;    break;
    }
    g_bridge->createRequest = CREATE_IDLE;
    logf("create: type=%d parent='%s' name='%s' -> rc=%d status=%u",
         type, g_bridge->parentName, g_bridge->newName, rc, g_bridge->createStatus);

    // If it worked, target the new widget automatically unless the producer
    // already picked something else.
    if (rc == 0 && !g_bridge->targetName[0]) {
        strncpy_s(g_bridge->targetName, sizeof(g_bridge->targetName),
                  g_bridge->newName, _TRUNCATE);
    }
}

// ---- introspection -----------------------------------------------------------
static void* FindByFullPath(const char* path);

// Cached full-path resolution for the two names that are used every event.
struct PathCache { char name[64]; void* ptr; };
static PathCache g_targetCache, g_inspectCache;

// Returns self if it matches `want`; otherwise, if `want` is a full path
// ("INFOBAR/X"), the element the registry maps it to (cached), else NULL.
static void* Resolve(void* self, const char* want, PathCache& c) {
    if (!want || !want[0]) return NULL;
    if (NameMatches(self, want)) return self;
    if (!strchr(want, '/')) return NULL;
    if (strncmp(c.name, want, 64) != 0 || !c.ptr) {
        c.ptr = FindByFullPath(want);
        strncpy_s(c.name, sizeof(c.name), want, _TRUNCATE);
    }
    return c.ptr;
}

static void DoInspect(void* self) {
    if (!g_bridge->inspectName[0]) return;
    self = Resolve(self, g_bridge->inspectName, g_inspectCache);
    if (!self) return;
    uint32_t n = g_bridge->dumpCount;
    if (n > 64) n = 64;
    uint32_t itype = ElemType(self);
    // Unknown types (FRAME, DESKTOP...) share the CGUIWnd base, so allow reads
    // below the smallest known widget (0x6F8) for them.
    uint32_t limit = ElementSize(itype) ? ElementSize(itype) : 0x6F8;
    while (n && (g_bridge->dumpOffset + n * 4 > limit)) n--;          // clamp, never read past
    g_bridge->inspectPtr = (uint32_t)self;
    g_bridge->dumpCount = n;
    __try {
        const uint8_t* p = (const uint8_t*)self + g_bridge->dumpOffset;
        for (uint32_t i = 0; i < n; i++)
            g_bridge->dumpOut[i] = *(const int32_t*)(p + i * 4);
        g_bridge->inspectHits++;
    } __except (EXCEPTION_EXECUTE_HANDLER) { }
}

// ---- skinning ----------------------------------------------------------------
// Templates live in a vector on the registry singleton; an element records which
// one it uses at +0x14C. Copy that index off a widget that already looks right,
// then apply the same template object here.
static void DoSkin(void* self) {
    g_bridge->skinRequest = 0;
    __try {
        uint8_t* reg = (uint8_t*)gRegistryGet();
        int32_t begin = *(int32_t*)(reg + REG_TMPL_BEGIN);
        int32_t end   = *(int32_t*)(reg + REG_TMPL_END);
        int32_t count = (end - begin) >> 2;
        int32_t idx = g_bridge->skinIndex;
        if (!begin || idx < 0 || idx >= count) {
            g_bridge->skinStatus = 3;
            logf("skin: index %d out of range (0..%d)", idx, count - 1);
            return;
        }
        void* tmpl = *(void**)(begin + idx * 4);
        if (!tmpl) { g_bridge->skinStatus = 3; logf("skin: template %d is null", idx); return; }
        // v8: GuiWnd_ApplyTemplateByIndex is the game's own wrapper around GuiTemplate_Apply.
        // It saves and restores PATHID/ACCESSID (the v6 "renamed our element" pitfall),
        // stores the index at +0x14C, rebuilds the BACKGROUND object from +0x199 and keeps
        // visibility and ZORDER. No --oncreate needed afterwards.
        ((tApplyTemplateIdx)ADDR_APPLY_TEMPLATE_IX)(self, NULL, idx);
        g_bridge->skinStatus = 2;
        logf("skin: applied template %d (of %d) to %p via GuiWnd_ApplyTemplateByIndex", idx, count, self);
    } __except (EXCEPTION_EXECUTE_HANDLER) {
        g_bridge->skinStatus = 4;
        logf("skin: EXCEPTION applying template");
    }
}

// ---- v6: element operations ---------------------------------------------------
// The background name to copy for "@OTHER", captured when OTHER passes through
// the dispatcher (its full path may be long, so we do not look it up by name).
static char g_bgCache[0x104];
static bool g_bgCacheValid = false;

static void CaptureBackgroundSource(void* self) {
    if (g_bridge->opRequest != OP_SETBACKGROUND || g_bridge->opArg[0] != '@') return;
    if (g_bgCacheValid) return;
    if (!NameMatches(self, g_bridge->opArg + 1)) return;
    uint32_t type = ElemType(self);
    if (!InBounds(type, OFF_BACKGROUND, 0x104)) return;
    strncpy_s(g_bgCache, sizeof(g_bgCache), (const char*)((uint8_t*)self + OFF_BACKGROUND), _TRUNCATE);
    g_bgCacheValid = true;
    logf("bg: captured '%s' from %s", g_bgCache, g_bridge->opArg + 1);
}

// Resolve "PARENT/CHILD" through the registry's full-path map, so an element that
// is hidden (and therefore never dispatches events) can still be reached.
static void* FindByFullPath(const char* path) {
    if (!path || !strchr(path, '/')) return NULL;
    void* found = NULL;
    __try {
        ScopedStr s(path);
        found = gFindByName(gRegistryGet(), NULL, &s, 0);
    } __except (EXCEPTION_EXECUTE_HANDLER) { found = NULL; }
    return found;
}

static void DoOpOn(void* self);

// ---- v8: XML route ---------------------------------------------------------------
// The game keeps its GUI XML as CXMLExtNode trees (0x5C bytes: +0x1C name, +0x3C value,
// +8/+0xC child list, +0x14 parent, +0x18 pool). We build such a tree from a .dlg
// fragment with the game's own constructors, wrap it in a CConfigDocTemplate positioned
// on the first element node, and hand it to GuiElement_InstantiateXmlSubtree, the
// function STRUCTTEMPLATEINSTANCE uses to clone .stp subtrees. Every tag the element
// types' ReadDefinitionNode understands is honoured, and AddChild fires OnCreate.
// Nodes are malloc'd by us and never freed: the document is not destructed either,
// because XmlDoc_Destruct would hand an unattached root back to the game's node pool.
struct XmlBuild {
    void*    ctx;        // CConfigDocContext at g_pGuiManager+0x42C
    void*    pool;       // ctx+0xC0, what XmlNode_AppendChild allocates list nodes from
    int      nodes;      // nodes created
    int      depth;
    char     err[96];
};

static void* XmlNewNode(XmlBuild& b, const char* name) {
    void* n = calloc(1, XMLNODE_SIZE);
    if (!n) return NULL;
    ((tXmlNodeCtor)ADDR_XMLNODE_CTOR)(n, NULL);                 // vftable + empty strings
    ((tXmlNodeInit)ADDR_XMLNODE_INIT)(n, NULL, name, b.pool);   // links, pool, name
    b.nodes++;
    return n;
}

static void XmlSetValue(void* node, const char* begin, const char* end) {
    // trim, then assign with the game's stlp_string::assign(begin, end)
    while (begin < end && (unsigned char)*begin <= ' ') begin++;
    while (end > begin && (unsigned char)end[-1] <= ' ') end--;
    if (begin == end) return;
    // decode the three entities .dlg files can contain
    char tmp[1024]; size_t o = 0;
    for (const char* p = begin; p < end && o < sizeof(tmp) - 1; p++) {
        if (*p == '&') {
            if (strncmp(p, "&lt;", 4) == 0)  { tmp[o++] = '<'; p += 3; continue; }
            if (strncmp(p, "&gt;", 4) == 0)  { tmp[o++] = '>'; p += 3; continue; }
            if (strncmp(p, "&amp;", 5) == 0) { tmp[o++] = '&'; p += 4; continue; }
        }
        tmp[o++] = *p;
    }
    gStrAssign((uint8_t*)node + 0x3C, NULL, tmp, tmp + o);
}

// Minimal parser for the .dlg/.tpl subset: tags, text, comments, optional <?xml?>.
// No attributes exist in the format. Returns the number of top-level nodes appended
// under `root`, or -1 with b.err set.
static int XmlParseInto(XmlBuild& b, void* root, const char* xml, size_t len) {
    void* stack[32]; int sp = 0; stack[0] = root;
    const char* p = xml; const char* end = xml + len;
    const char* text = NULL; int top = 0;
    while (p < end) {
        if (*p != '<') { if (!text) text = p; p++; continue; }
        if (text && sp > 0) XmlSetValue(stack[sp], text, p);
        text = NULL;
        if (p + 4 <= end && strncmp(p, "<!--", 4) == 0) {
            const char* c = strstr(p + 4, "-->"); if (!c) { strcpy_s(b.err, "unterminated comment"); return -1; }
            p = c + 3; continue;
        }
        if (p + 2 <= end && p[1] == '?') {
            const char* c = strstr(p + 2, "?>"); if (!c) { strcpy_s(b.err, "unterminated <?"); return -1; }
            p = c + 2; continue;
        }
        const char* close = (const char*)memchr(p, '>', end - p);
        if (!close) { strcpy_s(b.err, "unterminated tag"); return -1; }
        if (p[1] == '/') {
            if (sp == 0) { strcpy_s(b.err, "closing tag at top level"); return -1; }
            sp--;
        } else {
            char name[64]; size_t n = close - (p + 1);
            bool selfClose = n && p[n] == '/';
            if (selfClose) n--;
            while (n && (unsigned char)p[n] <= ' ') n--;
            if (n == 0 || n >= sizeof(name)) { strcpy_s(b.err, "bad tag name"); return -1; }
            memcpy(name, p + 1, n); name[n] = 0;
            if (sp >= 31) { strcpy_s(b.err, "nesting too deep"); return -1; }
            void* node = XmlNewNode(b, name);
            if (!node) { strcpy_s(b.err, "out of memory"); return -1; }
            ((tXmlNodeAppend)ADDR_XMLNODE_APPEND)(stack[sp], NULL, node);
            if (sp == 0) top++;
            if (!selfClose) stack[++sp] = node;
        }
        p = close + 1;
    }
    if (sp != 0) { strcpy_s(b.err, "unclosed tags at end of file"); return -1; }
    return top;
}

static int CountChildren(void* elem) {
    uint32_t** head = (uint32_t**)((uint8_t*)elem + 0x80);
    int n = 0;
    for (uint32_t* node = *head; node && node != (uint32_t*)head && n < 4096; node = (uint32_t*)node[0]) n++;
    return n;
}

// The XML constructors (GuiWnd_ConstructFromXml) do not register the element: the game's
// .cdf loader calls GuiWnd_RegisterInRegistry afterwards, which puts the element into the
// registry's all-elements list (what RebuildPathMap walks), the ACCESSID map (what SETVISIBLE
// and friends resolve through) and the GROUP map. Do the same for every element we built.
static int   g_modalWrapped = 0;
static void* g_modalPending[16];
static int   g_modalPendingCount = 0;
static int RegisterSubtree(void* elem, int depth) {
    if (!elem || depth > 32) return 0;
    int n = 1;
    ((tRegisterElem)ADDR_REGISTER_ELEM)(elem, NULL);
    // FRAME_MODAL (0x200) / FRAME_POPUP (0x800): the .cdf loader attaches
    // GuiWndFrame_GetOrCreateModalBackground(frame) instead of the frame itself. The XML
    // factory attaches the bare frame, and the first show then creates an ORPHAN modal
    // background (2026-09-26: frame reparented, background never in the tree, nothing
    // rendered). Wrap it now, while +0x7B0 is still 0, exactly as the loader would.
    if (ElemType(elem) == TYPE_FRAME && (*(uint32_t*)((uint8_t*)elem + 0x2A0) & 0xA00)
        && *(uint32_t*)((uint8_t*)elem + 0x7B0) == 0 && *(void**)((uint8_t*)elem + 0x88)) {
        // Deferred: wrapping unlinks the frame from the list we are walking and frees the
        // list node (2026-09-26: walking on from it hung the GUI thread).
        if (g_modalPendingCount < 16) g_modalPending[g_modalPendingCount++] = elem;
    }
    uint32_t** head = (uint32_t**)((uint8_t*)elem + 0x80);
    for (uint32_t* node = *head; node && node != (uint32_t*)head; node = (uint32_t*)node[0])
        n += RegisterSubtree((void*)node[2], depth + 1);
    return n;
}

// Returns the number of new direct children of `parent`, or -1.
static int DoXml(void* parent, const char* path) {
    logf("xml: begin '%s' under %p (type %u)", path, parent, ElemType(parent));
    FILE* f = NULL;
    if (fopen_s(&f, path, "rb") != 0 || !f) { logf("xml: cannot open '%s'", path); return -1; }
    fseek(f, 0, SEEK_END); long sz = ftell(f); fseek(f, 0, SEEK_SET);
    if (sz <= 0 || sz > 256 * 1024) { fclose(f); logf("xml: bad size %ld", sz); return -1; }
    char* buf = (char*)malloc(sz + 1);
    if (!buf) { fclose(f); return -1; }
    size_t got = fread(buf, 1, sz, f); fclose(f); buf[got] = 0;

    XmlBuild b; memset(&b, 0, sizeof(b));
    uint8_t* mgr = *(uint8_t**)ADDR_G_GUIMANAGER;
    b.ctx = mgr ? *(void**)(mgr + 0x42C) : NULL;
    if (!b.ctx) { free(buf); logf("xml: no XML context"); return -1; }
    b.pool = (uint8_t*)b.ctx + 0xC0;

    __declspec(align(8)) uint8_t doc[XMLDOC_SIZE]; memset(doc, 0, sizeof(doc));
    ((tXmlDocCtorEmpty)ADDR_XMLDOC_CTOR_EMPTY)(doc, NULL, b.ctx);   // root "<unnamed>", doc+8 = root
    void* root = *(void**)(doc + 4);

    int top = XmlParseInto(b, root, buf, got);
    free(buf);
    if (top < 0) { logf("xml: parse error: %s", b.err); return -1; }
    if (top == 0) { logf("xml: no element nodes"); return -1; }

    // position the document on the first child of the root: doc+8 = current node
    uint32_t* firstList = *(uint32_t**)((uint8_t*)root + 8);
    *(void**)(doc + 8) = firstList ? (void*)firstList[0] : NULL;
    if (!*(void**)(doc + 8)) { logf("xml: root has no children"); return -1; }

    int before = CountChildren(parent);
    logf("xml: parsed %d nodes, instantiating", b.nodes);
    ((tInstantiateXml)ADDR_INSTANTIATE_XML)(parent, doc, -1);
    int after = CountChildren(parent);
    logf("xml: instantiated, children %d -> %d, registering", before, after);
    // register the new children (they are the last after-before entries of the list)
    int registered = 0, i = 0; g_modalWrapped = 0; g_modalPendingCount = 0;
    uint32_t** head = (uint32_t**)((uint8_t*)parent + 0x80);
    for (uint32_t* node = *head; node && node != (uint32_t*)head; node = (uint32_t*)node[0], i++)
        if (i >= before) registered += RegisterSubtree((void*)node[2], 0);
    for (int k = 0; k < g_modalPendingCount; k++) {
        void* fr = g_modalPending[k];
        logf("xml: wrapping modal frame %p '%s' (parent %p)", fr, ElemPathId(fr), *(void**)((uint8_t*)fr + 0x88));
        ((tFrameWrapModal)ADDR_FRAME_WRAP_MODAL)(fr, NULL, 1);
        // The fresh background is visible by default and would swallow every click on its
        // siblings while the frame is still hidden. Re-apply the frame's own visibility
        // through its SetVisible override (CGUIWndFrame vtable +0xAC = FUN_00D8C7D0), which
        // shows/hides the background together with the frame.
        {
            void** fvt = *(void***)fr;
            int vis = *(int*)((uint8_t*)fr + OFF_VISIBLE) ? 1 : 0;
            ((tSetVisible)fvt[VT_SETVISIBLE])(fr, NULL, vis, 0, 0);
        }
        logf("xml: wrapped -> background %p (frame visible %d)", *(void**)((uint8_t*)fr + 0x7B0), *(int*)((uint8_t*)fr + OFF_VISIBLE));
        g_modalWrapped++;
    }
    ((tRebuildPaths)ADDR_REBUILD_PATHS)(gRegistryGet(), NULL);
    logf("xml: '%s' -> %d nodes, %d top-level, parent %p children %d -> %d, %d elements registered, %d modal frames wrapped",
         path, b.nodes, top, parent, before, after, registered, g_modalWrapped);
    return after - before;
}

static void DoOp(void* self) {
    uint32_t op = g_bridge->opRequest;
    if (op == OP_NONE) return;
    if (op == OP_LOADSAVE) {                        // global: no target element
        uint32_t st = OPST_FAILED;
        __try {
            void* mc = *(void**)ADDR_MAINCONTROL_PTR;
            g_bridge->opText[127] = 0;
            if (mc && g_bridge->opText[0]) {
                ScopedWStr path((const wchar_t*)g_bridge->opText);
                ((tLoadSave)ADDR_LOADSAVE)(mc, &path);
                st = OPST_DONE;
                logf("op: LoadSavegame posted (mainControl %p)", mc);
            } else {
                logf("op: LoadSavegame: mainControl %p, empty path %d", mc, g_bridge->opText[0] == 0);
            }
        } __except (EXCEPTION_EXECUTE_HANDLER) {
            logf("op: LoadSavegame EXCEPTION");
        }
        g_bridge->opRequest = OP_NONE;
        MemoryBarrier();
        InterlockedExchange((volatile LONG*)&g_bridge->opStatus, st);
        return;
    }
    if (op == OP_REBUILDPATHS) {
        uint32_t st = OPST_FAILED;
        __try { ((tRebuildPaths)ADDR_REBUILD_PATHS)(gRegistryGet(), NULL); st = OPST_DONE; logf("op: RebuildPathMap"); }
        __except (EXCEPTION_EXECUTE_HANDLER) { logf("op: RebuildPathMap EXCEPTION"); }
        g_bridge->opRequest = OP_NONE;
        MemoryBarrier();
        InterlockedExchange((volatile LONG*)&g_bridge->opStatus, st);
        return;
    }
    if (strchr(g_bridge->opTarget, '/')) {          // full-path target: act on any event
        void* e = FindByFullPath(g_bridge->opTarget);
        if (!e) { g_bridge->opStatus = OPST_WAITSRC; return; }
        DoOpOn(e);
        return;
    }
    if (!NameMatches(self, g_bridge->opTarget)) return;
    DoOpOn(self);
}

static void DoOpOn(void* self) {
    uint32_t op = g_bridge->opRequest;
    uint32_t type = ElemType(self);
    // Read-only ops work on any CGUIWnd (FRAME etc. are not in the size table).
    if (!ElementSize(type) && op != OP_LISTCHILDREN && op != OP_SETVISIBLE && op != OP_CLICK && op != OP_XML) {
        g_bridge->opRequest = OP_NONE;
        MemoryBarrier();
        InterlockedExchange((volatile LONG*)&g_bridge->opStatus, OPST_BADTYPE); return;
    }
    void** vtbl = *(void***)self;
    uint32_t st = OPST_FAILED;

    __try {
        switch (op) {
        case OP_ONCREATE: {
            ((tVoidThis)vtbl[VT_ONCREATE])(self, NULL);
            st = OPST_DONE;
            g_bridge->opResult = InBounds(type, OFF_BG_OBJECT, 4)
                               ? *(uint32_t*)((uint8_t*)self + OFF_BG_OBJECT) : 0;
            logf("op: OnCreate on %p type %u -> bg object %08X", self, type, g_bridge->opResult);
            break;
        }
        case OP_SETTEXT: {
            uint32_t tsOff = 0;
            if (type == TYPE_LABEL)          tsOff = OFF_TS_LABEL;
            else if (type == TYPE_BUTTON)    tsOff = OFF_TS_BUTTON;
            else if (type == TYPE_EDITFIELD) tsOff = OFF_TS_EDITFIELD;
            if (!tsOff || !InBounds(type, tsOff, 0x2C)) { st = OPST_BADTYPE; break; }
            g_bridge->opText[127] = 0;
            ScopedWStr text((const wchar_t*)g_bridge->opText);
            if (type == TYPE_LABEL) gLabelSetTextW(self, NULL, &text, 1);
            else                    gTsSetTextW((uint8_t*)self + tsOff, NULL, &text, 1);
            st = OPST_DONE;
            logf("op: SetText on %p type %u (%u chars)", self, type,
                 (unsigned)wcsnlen((const wchar_t*)g_bridge->opText, 127));
            break;
        }
        case OP_SETBACKGROUND: {
            const char* name = g_bridge->opArg;
            if (name[0] == '@') {
                if (!g_bgCacheValid) { st = OPST_WAITSRC; break; }   // try again next event
                name = g_bgCache;
            }
            ScopedStr s(name);
            uint32_t rc = ((tSetBackground)vtbl[VT_SETBACKGROUND])(self, NULL, &s);
            g_bridge->opResult = InBounds(type, OFF_BG_OBJECT, 4)
                               ? *(uint32_t*)((uint8_t*)self + OFF_BG_OBJECT) : 0;
            st = OPST_DONE;
            logf("op: SetBackground('%s') on %p type %u -> rc=%u bg object %08X",
                 name, self, type, rc & 0xFF, g_bridge->opResult);
            break;
        }
        case OP_LISTCHILDREN: {
            // STLport list head at +0x80: node {next, prev, CGUIWnd* elem}
            char* out = g_bridge->listOut; size_t cap = sizeof(g_bridge->listOut); out[0] = 0;
            uint32_t** head = (uint32_t**)((uint8_t*)self + 0x80);
            int n = 0;
            for (uint32_t* node = *head; node && node != (uint32_t*)head && n < 64; node = (uint32_t*)node[0], n++) {
                uint8_t* c = (uint8_t*)node[2];
                if (!c) continue;
                char line[160];
                _snprintf_s(line, sizeof(line), _TRUNCATE, "%s|%s|%u|%d;",
                            ElemPathId(c), ElemAccessId(c), ElemType(c), *(int32_t*)(c + 0x9C));
                if (strlen(out) + strlen(line) + 1 >= cap) break;
                strcat_s(out, cap, line);
            }
            g_bridge->opResult = n;
            st = OPST_DONE;
            break;
        }
        case OP_SETFONT: {
            // The font is resolved BY NAME at layout time: TextEngine_LayoutText reads the
            // stlp_string at +0x78 of the parameter block that starts at TextData+0x50,
            // i.e. TextData+0xC8 (FUN_00DC1AA0 writes it there from the <FONT> tag).
            // TextEngine_GetFontByName takes its string by value and crashed v6 when
            // called with a pointer, so we do not call it: we assign the name with the
            // game's own string assign (thiscall, RET 8) and clear the laid-out flag.
            // Equivalent without any game call: prefix the text with "[FONT name]",
            // which is CONFIRMED to work.
            uint32_t tsOff = TextSettingOffset(type);
            if (!tsOff) { st = OPST_BADTYPE; break; }
            uint8_t* data = *(uint8_t**)((uint8_t*)self + tsOff + 4);
            if (!data) { st = OPST_FAILED; logf("op: SetFont: TextData not allocated yet, set text first"); break; }
            const char* name = g_bridge->opArg;
            gStrAssign(data + 0x50 + 0x78, NULL, name, name + strlen(name));
            *(uint32_t*)(data + 4) &= ~0x10u;
            st = OPST_DONE;
            logf("op: SetFont('%s') on %p type %u", name, self, type);
            break;
        }
        case OP_SETALIGN: {
            uint32_t tsOff = TextSettingOffset(type);
            if (!tsOff) { st = OPST_BADTYPE; break; }
            int h = atoi(g_bridge->opArg);
            const char* comma = strchr(g_bridge->opArg, ',');
            int v = comma ? atoi(comma + 1) : 0;
            gTsSetHAlign((uint8_t*)self + tsOff, NULL, h);
            gTsSetVAlign((uint8_t*)self + tsOff, NULL, v);
            st = OPST_DONE;
            logf("op: SetAlign(%d,%d) on %p type %u", h, v, self, type);
            break;
        }
        case OP_SETVISIBLE: {
            int vis = (g_bridge->opArg[0] == '1') ? 1 : 0;
            ((tSetVisible)vtbl[VT_SETVISIBLE])(self, NULL, vis, 1, 0);
            st = OPST_DONE;
            logf("op: SetVisible(%d) on %p type %u", vis, self, type);
            break;
        }
        case OP_CLICK: {
            // Same call shape as the game's FUN_004555E0: name string, 16-byte arg block
            // {0, 0, element, result}, default result 1. Goes through the trampoline so
            // our own hook does not see it twice.
            if (!g_orig) { st = OPST_FAILED; break; }
            const char* ev = g_bridge->opArg[0] ? g_bridge->opArg : "OnLeftClick";
            ScopedStr name(ev);
            uint32_t args[4] = { 0, 0, (uint32_t)self, 0 };
            char rc = g_orig(self, NULL, &name, args, 1);
            if (args[3]) {   // result object: release it like the game does (vtable slot 1, arg 1)
                void** rvt = *(void***)args[3];
                ((void (__fastcall*)(void*, void*, int))rvt[1])((void*)args[3], NULL, 1);
            }
            g_bridge->opResult = (uint32_t)(uint8_t)rc;
            st = OPST_DONE;
            logf("op: dispatch '%s' on %p type %u -> %d", ev, self, type, rc);
            break;
        }
        case OP_XML: {
            int n = DoXml(self, g_bridge->opArg);
            g_bridge->opResult = (uint32_t)n;
            st = n >= 0 ? OPST_DONE : OPST_FAILED;
            break;
        }
        default:
            st = OPST_FAILED;
        }
    } __except (EXCEPTION_EXECUTE_HANDLER) {
        st = OPST_FAILED;
        logf("op %u: EXCEPTION on %p", op, self);
    }

    if (st == OPST_WAITSRC) { g_bridge->opStatus = st; return; }   // keep the request
    g_bgCacheValid = false;
    g_bridge->opRequest = OP_NONE;
    MemoryBarrier();
    InterlockedExchange((volatile LONG*)&g_bridge->opStatus, st);
}

// ---- per-event work ----------------------------------------------------------
// Runs on the game's GUI thread. The event name is ignored: OnRender fires every
// frame for a visible element, so matching on the name alone is enough.
// Everything that calls into the game re-enters this hook (the game dispatches
// events from inside its own GUI calls). Guard so nested events only observe.
// Apply a whole presentation snapshot in one GUI dispatch. Resolve widgets on
// every batch: pointers may become invalid after loading a save or rebuilding HUD.
static void PublishUiEvent(int row, int island, int player, uint32_t ctrl = 0) {
    if (!g_ui) return;
    uint32_t n = (uint32_t)g_ui->clickWrite;
    UiClick* event = &g_ui->clicks[n % UI_CLICK_CAPACITY];
    event->row = row;
    event->island = island;
    event->player = player;
    event->ctrl = ctrl;
    event->tick = GetTickCount();
    MemoryBarrier();
    InterlockedIncrement(&g_ui->clickWrite);
}

static void ObserveDialog(void* self) {
    if (!g_ui || !self) return;
    const char* access = ElemAccessId(self);
    void* dialog = NULL;
    if (access && strcmp(access, "ANNORE_PROD5_DIALOG") == 0) dialog = self;
    else if (access && (strcmp(access, "ANNORE_PROD5_BUTTON") == 0 ||
                        strcmp(access, "ANNORE_PROD5_CLOSE") == 0))
        dialog = FindByFullPath("SYSTEMMESSAGES/ANNORE_PROD5_DIALOG");
    if (!dialog) return;
    // Only compare remembered pointer identity; never dereference a cached widget.
    if (g_observedDialog != dialog) {
        g_observedDialog = dialog;
        g_dialogWasVisible = false;
    }
    void* parent = *(void**)((uint8_t*)dialog + 0x88);
    if (parent) {
        const int32_t* rect = (int32_t*)((uint8_t*)parent + OFF_EXTENT);
        int width = rect[2] - rect[0], height = rect[3] - rect[1];
        if (width > 0 && height > 0) {
            InterlockedExchange(&g_ui->viewportWidth, width);
            InterlockedExchange(&g_ui->viewportHeight, height);
        }
    }
    bool visible = *(int32_t*)((uint8_t*)dialog + OFF_VISIBLE) == 1;
    if (visible == g_dialogWasVisible) return;
    g_dialogWasVisible = visible;
    PublishUiEvent(visible ? UI_DIALOG_OPEN : UI_DIALOG_CLOSE,
        *(int32_t*)0x012D54F8, *(int32_t*)0x012D54F4);
}

static void DoUiBatch() {
    if (!g_ui || InterlockedCompareExchange(&g_ui->state, 2, 1) != 1) return;
    uint32_t result = 0;
    __try {
        void* dialog = FindByFullPath("SYSTEMMESSAGES/ANNORE_PROD5_DIALOG");
        void* button = FindByFullPath("INFOBAR/ANNORE_PROD5_BUTTON");
        if (!dialog || !button) {
            ((tRebuildPaths)ADDR_REBUILD_PATHS)(gRegistryGet(), NULL);
            dialog = FindByFullPath("SYSTEMMESSAGES/ANNORE_PROD5_DIALOG");
            button = FindByFullPath("INFOBAR/ANNORE_PROD5_BUTTON");
        }
        if (!dialog) result |= UI_MISSING_DIALOG;
        if (!button) result |= UI_MISSING_BUTTON;
        if (g_ui->count > UI_CAPACITY) result |= UI_FAILED_UPDATE;
        void* elements[UI_CAPACITY] = {};
        uint32_t textOffsets[UI_CAPACITY] = {}, types[UI_CAPACITY] = {};
        // Validate the entire batch before changing widgets. Missing rows during
        // a HUD transition must not leave a half-applied settlement selection.
        for (uint32_t i = 0; i < g_ui->count && i < UI_CAPACITY; ++i) {
            UiUpdate* u = &g_ui->updates[i];
            u->target[63] = 0;
            u->text[127] = 0;
            void* e = FindByFullPath(u->target);
            if (!e) { result |= UI_FAILED_UPDATE; continue; }
            uint32_t type = ElemType(e);
            elements[i] = e;
            types[i] = type;
            if (u->flags & ~(UI_TEXT | UI_VISIBLE | UI_CHECKED | UI_CHECKED_ON | UI_RECT | UI_BAR | UI_ICON))
                result |= UI_FAILED_UPDATE;
            if ((u->flags & UI_CHECKED_ON) && !(u->flags & UI_CHECKED))
                result |= UI_FAILED_UPDATE;
            if ((u->flags & UI_CHECKED) &&
                (type != TYPE_BUTTON || !InBounds(type, 0xA60, 1)))
                result |= UI_FAILED_UPDATE;
            if ((u->flags & UI_RECT) &&
                (u->rect[2] <= u->rect[0] || u->rect[3] <= u->rect[1]))
                result |= UI_FAILED_UPDATE;
            if ((u->flags & UI_BAR) && (type != TYPE_PROGRESSBAR ||
                !InBounds(type, OFF_PB_VALUE, 8) || u->barMax <= 1 ||
                u->barValue < 0 || u->barValue > u->barMax)) result |= UI_FAILED_UPDATE;
            if ((u->flags & UI_ICON) && (type != TYPE_LABEL ||
                !InBounds(type, 0x878, 4))) result |= UI_FAILED_UPDATE;
            if (u->flags & UI_TEXT) {
                uint32_t off = type == TYPE_LABEL ? OFF_TS_LABEL :
                    type == TYPE_BUTTON ? OFF_TS_BUTTON :
                    type == TYPE_EDITFIELD ? OFF_TS_EDITFIELD : 0;
                if (!off || !InBounds(type, off, 0x2C)) {
                    result |= UI_FAILED_UPDATE;
                    continue;
                }
                textOffsets[i] = off;
            }
        }
        for (uint32_t i = 0; !result && i < g_ui->count; ++i) {
            UiUpdate* u = &g_ui->updates[i];
            void* e = elements[i];
            if (u->flags & UI_RECT) {
                // Verified Ghidra ABI: ECX=this, four stack arguments; setter
                // invalidates alignment so absolute hit-test rects update too.
                ((void (__fastcall*)(void*, void*, int, int, int, int))0x00D5B150)
                    (e, NULL, u->rect[0], u->rect[1], u->rect[2], u->rect[3]);
            }
            if (u->flags & UI_BAR) {
                *(int32_t*)((uint8_t*)e + OFF_PB_VALUE) = u->barValue;
                *(int32_t*)((uint8_t*)e + OFF_PB_MAX) = u->barMax;
            }
            if (u->flags & UI_ICON) *(int32_t*)((uint8_t*)e + 0x878) = u->icon;
            if (u->flags & UI_TEXT) {
                ScopedWStr text((const wchar_t*)u->text);
                if (types[i] == TYPE_LABEL) gLabelSetTextW(e, NULL, &text, 1);
                else {
                    void* setting = (uint8_t*)e + textOffsets[i];
                    gTsSetTextW(setting, NULL, &text, 1);
                    if (types[i] == TYPE_BUTTON) {
                        // Renderer axes differ from the historical setter names:
                        // first controls vertical position, second horizontal.
                        gTsSetHAlign(setting, NULL, 1);
                        gTsSetVAlign(setting, NULL, 4);
                    }
                }
            }
            if (u->flags & UI_VISIBLE) {
                void** vt = *(void***)e;
                ((tSetVisible)vt[VT_SETVISIBLE])(e, NULL, u->visible ? 1 : 0, 0, 0);
            }
            if (u->flags & UI_CHECKED) {
                // Confirmed static in Ghidra: +0xA60 selects the native Checked
                // layouts; 0x00D3E2C0 refreshes visuals, text color and tooltip.
                // Keep TOGGLEBUTTON off: the companion owns selection, including
                // Ctrl-click and clicking an already selected settlement.
                *(uint8_t*)((uint8_t*)e + 0xA60) = (u->flags & UI_CHECKED_ON) ? 1 : 0;
                ((tVoidThis)0x00D3E2C0)(e, NULL);
            }
        }
    } __except (EXCEPTION_EXECUTE_HANDLER) { result |= UI_FAILED_UPDATE; }
    if (!result) {
        g_ui->displayedPlayer = g_ui->player;
        memcpy(g_ui->displayedIslands, g_ui->rowIslands, sizeof(g_ui->rowIslands));
        ObserveDialog(FindByFullPath("SYSTEMMESSAGES/ANNORE_PROD5_DIALOG"));
    }
    g_ui->result = result;
    MemoryBarrier();
    InterlockedExchange(&g_ui->state, 0);
    SetEvent(g_uiDone);
}

static volatile LONG g_busy = 0;

// v7: click on toggleButton shows/hides toggleTarget. Runs inside the busy guard
// because SetVisible dispatches OnShow/OnHide back into this hook.
static PathCache g_toggleCache;
static void DoToggle(void* self, const char* ev) {
    if (!g_bridge->toggleButton[0] || !ev || !ev[0]) return;
    if (!NameMatches(self, g_bridge->toggleButton)) return;
    if (strcmp(ev, "OnRender") == 0 || strcmp(ev, "OnPreRender") == 0) return;
    strncpy_s(g_bridge->lastEvent, sizeof(g_bridge->lastEvent), ev, _TRUNCATE);
    if (strcmp(ev, "OnLeftClick") != 0) return;
    if (!g_bridge->toggleTarget[0]) return;

    // Full-path map lookup; a top-level name is its own full path, so no '/' rule here.
    void* target = NULL;
    if (strncmp(g_toggleCache.name, g_bridge->toggleTarget, 64) == 0 && g_toggleCache.ptr) {
        target = g_toggleCache.ptr;
    } else if (gFindByName && gRegistryGet) {
        ScopedStr s(g_bridge->toggleTarget);
        target = gFindByName(gRegistryGet(), NULL, &s, 0);
        g_toggleCache.ptr = target;
        strncpy_s(g_toggleCache.name, sizeof(g_toggleCache.name), g_bridge->toggleTarget, _TRUNCATE);
    }
    if (!target) { logf("toggle: target '%s' not found", g_bridge->toggleTarget); return; }

    uint8_t vis = *((uint8_t*)target + OFF_VISIBLE);
    void** vtbl = *(void***)target;
    ((tSetVisible)vtbl[VT_SETVISIBLE])(target, NULL, vis ? 0 : 1, 1, 0);   // int arg, see typedef
    g_bridge->toggleCount++;
    logf("toggle: click on %s -> %s visible %u -> %u", g_bridge->toggleButton,
         g_bridge->toggleTarget, vis, (unsigned)*((uint8_t*)target + OFF_VISIBLE));
}

// v8.4: replace the tooltip text of one element. The element must carry a <TOOLTIP><TEXT>
// guid so the game shows a tooltip at all; we swap the resolved text once per show.
static PathCache g_tipCache;
static uint32_t  g_tipLastShow = 0;
static void DoTooltip(void* self) {
    if (!g_bridge->tipTarget[0] || !g_bridge->tipText[0]) return;
    uint8_t* mgr = *(uint8_t**)ADDR_TOOLTIP_MGR_PTR;
    if (!mgr) return;
    void* cur = *(void**)(mgr + TTM_CUR_ELEMENT);
    uint32_t tick = *(uint32_t*)(mgr + TTM_SHOW_TICK);
    if (!cur || !tick || tick == g_tipLastShow) return;
    void* want = Resolve(self, g_bridge->tipTarget, g_tipCache);
    if (!want) {                                    // not resolvable via this event; try by name match on cur
        if (!NameMatches(cur, g_bridge->tipTarget)) return;
        want = cur;
    }
    if (want != cur) return;
    g_tipLastShow = tick;
    __try {
        g_bridge->tipText[127] = 0;
        ((tTooltipSetText)ADDR_TOOLTIP_SETTEXT)(mgr, NULL, (const wchar_t*)g_bridge->tipText);
        if (g_bridge->tipMode == 2) ((tTooltipLayout)ADDR_TOOLTIP_LAYOUT)(mgr, NULL, mgr + TTM_REQUEST);
        g_bridge->tipCount++;
        logf("tooltip: replaced text for %p (mode %u)", cur, g_bridge->tipMode);
    } __except (EXCEPTION_EXECUTE_HANDLER) {
        logf("tooltip: EXCEPTION");
        g_bridge->tipTarget[0] = 0;
    }
}

static void OnElementEvent(void* self, const char* ev) {
    if (!self || !g_bridge) return;

    if (ev && strcmp(ev, "OnLeftClick") == 0) {
        const char* access = ElemAccessId(self);
        if (access && g_ui && (strcmp(access, "ANNORE_PREVIOUS") == 0 || strcmp(access, "ANNORE_NEXT") == 0))
            PublishUiEvent(strcmp(access, "ANNORE_NEXT") == 0 ? UI_NEXT : UI_PREVIOUS, 0, g_ui->displayedPlayer);
        if (access && strncmp(access, "ANNORE_ISLAND_ROW", 17) == 0) {
            int row = atoi(access + 17);
            if (row >= 1 && row <= 32) {
                uint8_t* shared = (uint8_t*)g_bridge;
                *(int32_t*)(shared + O_SELECTION_ROW) = row;
                *(uint32_t*)(shared + O_SELECTION_CTRL) =
                    (GetAsyncKeyState(VK_CONTROL) & 0x8000) ? 1u : 0u;
                MemoryBarrier();
                InterlockedIncrement((volatile LONG*)(shared + O_SELECTION_SEQ));
                if (g_ui) {
                    PublishUiEvent(row, row <= 19 ? g_ui->displayedIslands[row - 1] : 0,
                        g_ui->displayedPlayer, *(uint32_t*)(shared + O_SELECTION_CTRL));
                }
            }
        }
    }

    if (g_bridge->traceEvents && ev && ev[0]
        && strcmp(ev, "OnRender") != 0 && strcmp(ev, "OnPreRender") != 0) {
        logf("ev %-22s on %p type %2u path=%s access=%s", ev, self, ElemType(self),
             ElemPathId(self), ElemAccessId(self));
    }

    DoInspect(self);
    if (g_busy) return;                 // nested call from one of our own game calls
    InterlockedExchange(&g_busy, 1);
    __try { DoTooltip(self); } __finally { InterlockedExchange(&g_busy, 0); }
    InterlockedExchange(&g_busy, 1);
    __try {
        ObserveDialog(self);
        if (g_bridge->createRequest == CREATE_REQUESTED) DoCreate();
        CaptureBackgroundSource(self);
        DoOp(self);
        DoUiBatch();
        DoToggle(self, ev);
    } __finally {
        InterlockedExchange(&g_busy, 0);
    }

    self = Resolve(self, g_bridge->targetName, g_targetCache);
    if (!self) return;

    if (g_bridge->skinRequest && !g_busy) {
        InterlockedExchange(&g_busy, 1);
        __try { DoSkin(self); } __finally { InterlockedExchange(&g_busy, 0); }
    }

    uint32_t type = ElemType(self);
    g_bridge->lastType = type;

    // The rect belongs to rectTarget when set (v6.1), otherwise to whatever the
    // current target is -- which moved the label onto the button once.
    bool rectIsOurs = !g_bridge->rectTarget[0]
                   || strncmp(g_bridge->rectTarget, g_bridge->targetName, 64) == 0;
    if (g_bridge->applyRect && rectIsOurs && InBounds(type, OFF_EXTENT, 16)) {
        int32_t* ext = (int32_t*)((uint8_t*)self + OFF_EXTENT);
        for (int i = 0; i < 4; i++) ext[i] = g_bridge->rect[i];
    }

    for (int i = 0; i < 4; i++) {
        if (!g_bridge->pokes[i].enabled) continue;
        uint32_t off = g_bridge->pokes[i].offset;
        if (off & 3) continue;                      // must be aligned
        if (!InBounds(type, off, 4)) {              // must be inside THIS element
            g_bridge->pokes[i].enabled = 0;         // disable it rather than repeat
            logf("poke +0x%X rejected: outside a type-%u element (size 0x%X)",
                 off, type, ElementSize(type));
            continue;
        }
        *(int32_t*)((uint8_t*)self + off) = g_bridge->pokes[i].value;
    }

    int32_t v = g_bridge->value, m = g_bridge->maxValue;
    if (type == TYPE_PROGRESSBAR && InBounds(type, OFF_PB_VALUE, 8)) {
        *(int32_t*)((uint8_t*)self + OFF_PB_VALUE) = v;
        if (m > 0) *(int32_t*)((uint8_t*)self + OFF_PB_MAX) = m;
    } else if (type == TYPE_SLIDER && InBounds(type, OFF_SL_VALUE, 8)) {
        *(int32_t*)((uint8_t*)self + OFF_SL_VALUE) = v;
        if (m > 0) *(int32_t*)((uint8_t*)self + OFF_SL_RES) = m;
    }
    g_bridge->hits++;   // counts every matched element, whatever its type
}

// The event name arrives as an stlp_string*. STLport short-string layout, as the
// game's own code reads it (GuiRuntime_CreateElement, 0x00DB72C0):
//   +0x00  union { char buf[16]; char* heapPtr; }
//   +0x10  char* finish
//   +0x14  char* endOfStorage   == obj+0x10 while the text still fits in buf
// So a name of 15 chars or fewer ("OnLeftClick") lives AT the object itself; only
// longer names are behind the pointer. Reading +0 as a pointer gave garbage (v7.0).
static const char* EventCStr(void* evName) {
    if (!evName) return "";
    const char* obj = (const char*)evName;
    const char* eos = *(const char* const*)(obj + 0x14);
    const char* p = (eos == obj + 0x10) ? obj : *(const char* const*)obj;
    return p ? p : "";
}

static char __fastcall hkDispatch(void* self, void* edx, void* evName, void* args, char defRes) {
    bool observeAfter = false;
    __try {
        const char* access = ElemAccessId(self);
        observeAfter = access && (strcmp(access, "ANNORE_PROD5_BUTTON") == 0 ||
                                  strcmp(access, "ANNORE_PROD5_CLOSE") == 0);
    } __except (EXCEPTION_EXECUTE_HANDLER) { }
    __try { OnElementEvent(self, EventCStr(evName)); } __except (EXCEPTION_EXECUTE_HANDLER) { }
    char result = g_orig(self, edx, evName, args, defRes);
    // Capture transitions caused by the original event handler even if it used
    // SetVisible without firing OnShow/OnHide (trigger and close button paths).
    if (observeAfter && !g_busy) {
        InterlockedExchange(&g_busy, 1);
        // The original handler may have replaced the GUI tree. Resolve the
        // dialog again rather than dereferencing its event sender afterwards.
        __try { ObserveDialog(FindByFullPath("SYSTEMMESSAGES/ANNORE_PROD5_DIALOG")); }
        __except (EXCEPTION_EXECUTE_HANDLER) { }
        InterlockedExchange(&g_busy, 0);
    }
    return result;
}

// ---- hook install -----------------------------------------------------------
static bool InstallHook(void) {
    uint8_t* target = (uint8_t*)ADDR_DISPATCH;

    MEMORY_BASIC_INFORMATION mbi;
    if (!VirtualQuery(target, &mbi, sizeof(mbi)) || mbi.State != MEM_COMMIT) {
        logf("target not mapped"); return false;
    }
    if (memcmp(target, kPrologue, STOLEN) != 0) {
        logf("PROLOGUE MISMATCH at %p -- wrong binary or already hooked; refusing", target);
        return false;
    }

    g_tramp = (uint8_t*)VirtualAlloc(NULL, 32, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE);
    if (!g_tramp) { logf("trampoline alloc failed"); return false; }

    memcpy(g_tramp, target, STOLEN);
    g_tramp[STOLEN] = 0xE9;
    *(int32_t*)(g_tramp + STOLEN + 1) = (int32_t)((target + STOLEN) - (g_tramp + STOLEN + 5));
    g_orig = (tDispatch)g_tramp;

    DWORD old = 0;
    if (!VirtualProtect(target, STOLEN, PAGE_EXECUTE_READWRITE, &old)) {
        logf("VirtualProtect failed"); return false;
    }
    target[0] = 0xE9;
    *(int32_t*)(target + 1) = (int32_t)((uint8_t*)hkDispatch - (target + 5));
    target[5] = 0x90; target[6] = 0x90; target[7] = 0x90;
    VirtualProtect(target, STOLEN, old, &old);
    FlushInstructionCache(GetCurrentProcess(), target, STOLEN);

    logf("hook installed at %p, trampoline %p", target, g_tramp);
    return true;
}

static void LaunchCompanion(HMODULE self) {
    char dllPath[MAX_PATH] = { 0 };
    if (!GetModuleFileNameA(self, dllPath, MAX_PATH)) {
        logf("companion: cannot resolve DLL path (%lu)", GetLastError());
        return;
    }
    char* slash = strrchr(dllPath, '\\\\');
    if (!slash) {
        logf("companion: DLL path has no directory");
        return;
    }
    *slash = 0;

    char exePath[MAX_PATH] = { 0 };
    if (sprintf_s(exePath, sizeof(exePath), "%s\\\\%s", dllPath, COMPANION_EXE) < 0) {
        logf("companion: path too long");
        return;
    }
    if (GetFileAttributesA(exePath) == INVALID_FILE_ATTRIBUTES) {
        logf("companion: '%s' not found; continuing without it", exePath);
        return;
    }

    STARTUPINFOA si; PROCESS_INFORMATION pi;
    ZeroMemory(&si, sizeof(si)); ZeroMemory(&pi, sizeof(pi));
    si.cb = sizeof(si);
    char cmd[MAX_PATH + 64];
    if (sprintf_s(cmd, sizeof(cmd), "\\\"%s\\\" --parent-pid %lu", exePath,
                  (unsigned long)GetCurrentProcessId()) < 0) return;
    if (!CreateProcessA(exePath, cmd, NULL, NULL, FALSE,
                        CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP,
                        NULL, dllPath, &si, &pi)) {
        logf("companion: CreateProcess failed (%lu)", GetLastError());
        return;
    }
    logf("companion: started pid=%lu", pi.dwProcessId);
    CloseHandle(pi.hThread);
    CloseHandle(pi.hProcess);
}

static DWORD WINAPI InitThread(LPVOID param) {
    HMODULE self = (HMODULE)param;
    char exe[MAX_PATH] = { 0 };
    GetModuleFileNameA(NULL, exe, MAX_PATH);
    const char* base = strrchr(exe, '\\');
    base = base ? base + 1 : exe;
    if (_stricmp(base, "Addon.exe") != 0) {
        logf("host is '%s', not Addon.exe -- staying inert", base); return 0;
    }
    if ((uintptr_t)GetModuleHandleA(NULL) != 0x00400000) {
        logf("unexpected image base %p -- staying inert", GetModuleHandleA(NULL)); return 0;
    }
    if (!IsVerifiedGoldProcess(GetCurrentProcess())) {
        logf("unsupported executable fingerprint -- staying inert"); return 0;
    }

    g_map = CreateFileMappingA(INVALID_HANDLE_VALUE, NULL, PAGE_READWRITE, 0,
                               BRIDGE_SIZE, BRIDGE_NAME);
    if (!g_map) { logf("CreateFileMapping failed (%lu)", GetLastError()); return 0; }
    g_bridge = (Bridge*)MapViewOfFile(g_map, FILE_MAP_ALL_ACCESS, 0, 0, BRIDGE_SIZE);
    if (!g_bridge) { logf("MapViewOfFile failed (%lu)", GetLastError()); return 0; }

    g_bridge->magic = BRIDGE_MAGIC;
    g_bridge->version = BRIDGE_VERSION;
    g_bridge->tipMode = 1;
    logf("bridge v%u ready at %p (section '%s')", BRIDGE_VERSION, g_bridge, BRIDGE_NAME);

    g_uiMap = CreateFileMappingA(INVALID_HANDLE_VALUE, NULL, PAGE_READWRITE,
                                0, sizeof(UiShared), UI_MAP_NAME);
    if (g_uiMap) g_ui = (UiShared*)MapViewOfFile(g_uiMap, FILE_MAP_ALL_ACCESS,
                                                0, 0, sizeof(UiShared));
    g_uiDone = CreateEventA(NULL, FALSE, FALSE, UI_DONE_NAME);
    if (g_ui && g_uiDone) {
        ZeroMemory(g_ui, sizeof(*g_ui));
        g_ui->version = UI_VERSION;
        g_ui->size = sizeof(UiShared);
        g_ui->ownerPid = GetCurrentProcessId();
        MemoryBarrier();
        g_ui->magic = UI_MAGIC;
    } else {
        g_ui = NULL;
        logf("UI batch transport unavailable");
    }
    InstallHook();
    LaunchCompanion(self);
    return 0;
}

BOOL APIENTRY DllMain(HMODULE hMod, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) {
        DisableThreadLibraryCalls(hMod);
        HANDLE thread = CreateThread(NULL, 0, InitThread, hMod, 0, NULL);
        if (thread) CloseHandle(thread);
    }
    return TRUE;
}
