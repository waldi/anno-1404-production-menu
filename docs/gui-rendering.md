# How the GUI is drawn

`docs/gui-system.md` describes the GUI as a widget toolkit driven by XML definitions
and answers "can we add a widget". This file goes one layer down and answers "how does
a widget become pixels", because that layer turned out to be where the easy answers
are. Written 2026-09-18 from `Addon.exe` in Ghidra; nothing here has been exercised
from an injected DLL yet unless it says so.

Three things it changes:

| Belief in `gui-system.md` | What the binary does |
|---|---|
| A label's text is a localisation GUID; arbitrary text needs the `[CONTEXT_STRING]` tag path | **A label takes an arbitrary wide string at runtime.** `GuiWndLabel_SetTextW` (`0x00D7BC40`). The game uses it for its own `%d` counters. |
| A runtime-created PROGRESSBAR renders as a flat rectangle; cause unknown | The bar art needs a background object at `+0x580`, built by `GuiWnd_SetBackground` / the type's `OnCreate`. The template alone does not create it. |
| Nesting under a runtime-created parent fails with "parent not found"; the `+0x254` map key is unknown | The key is the element's **full path** `PARENT/CHILD`, and the map is rebuilt after every `GuiRuntime_CreateElement`. |

## The rendering stack

```
GameApp_MainLoop 0x00458480
  └─ CRenderGUIJob thread (RenderGUIJob_ThreadProc 0x004EA380)   -- or inline on the main thread
       └─ MainControl_RenderGUI 0x004A9050          "RenderGUI" profiler scope
            └─ GuiManager_Render 0x00D77820          this = g_pGuiManager (0x012D334C)
                 └─ root->RenderTree(0,0,W,H)        root = *(g_pGuiManager+0x420); vtable +0x80
                      GuiWnd_RenderTree 0x00D658D0   per element, recursive
                        ├─ vtable +0x88  RenderBackground   (GuiWnd_RenderBackground 0x00D61B60)
                        ├─ vtable +0x84  Render(CRect)      (GuiWndLabel_Render 0x00D7D2B0, GuiWndProgressBar_Render 0x00DB9EF0 …)
                        └─ children, each wrapped in PushClip/PopClip when they have a background
                             ▼
                 g_pGUIRenderer (0x012D5410) -> CGUIRenderer3DEngine singleton 0x013049DC
                   DrawQuad / DrawText / LoadTexture / CreateFont …   (30 virtual slots)
                             ▼
                 CRenderEngine+0xAC0: per-frame GUI command buffer (GuiCmdBuffer_PushCommand 0x00CDCC10)
                             ▼
                 render thread executes the commands against D3D9/D3D10
```

The widget classes never see a device. They call an abstract `GUIRenderer` (RTTI class
of that name; concrete `CGUIRenderer3DEngine`), whose methods serialise draw commands
into a buffer that the render thread consumes later. Two consequences for a DLL:

- **Thread.** Draw calls are only valid on the thread running `MainControl_RenderGUI`,
  and only while a frame is being recorded. `g_pGuiManager+0x440` holds that thread's
  id. A hook inside `GuiWnd_RenderTree` or `GuiManager_Render` is on the right thread
  at the right time by construction. The existing `annobridge` hook on the event
  dispatcher fires from the same pass (`OnRender` is dispatched per frame), which is
  why writing widget fields from it works; whether a *draw call* issued from it lands
  in the current frame's buffer is UNVERIFIED but likely.
- **No D3D hooking needed.** Everything the GUI can draw is reachable through one
  pointer, in the game's own coordinate system, with the game's own clipping and fonts.

## The GUIRenderer interface

`g_pGUIRenderer` is set once by `GuiRenderer_SetGlobal` (`0x00D822D0`) during
`GameApp::InitInstance`; the singleton is constructed by
`GuiRenderer3DEngine_Construct` (`0x005EE1C0`). vftable at `0x010E6A5C`. All methods
are `__thiscall` with `this = g_pGUIRenderer`. Rects are `{left, top, right, bottom}`
throughout (CRect order, not x/y/w/h). Colours are `0xAARRGGBB`.

| idx | offset | name | signature | confidence |
|---|---|---|---|---|
| 1 | `+0x04` | SetScreenSize? | `(w, h)` — called by `GuiManager_Render` on a resize flag | medium |
| 2 | `+0x08` | LoadTextureA | `(stlp_string *path) -> texId` | medium |
| 3 | `+0x0C` | LoadTextureW | `(stlp_wstring *path) -> texId` — used by a LABEL's picture path (`+0x8F4`) | high |
| 4 | `+0x10` | ReleaseTexture | `(texId)` | high |
| 5 | `+0x14` | GetTextureSize | `(SIZE *out, texId) -> out` | high |
| 6 | `+0x18` | CreateFont | `(LOGFONTW *lf, FontExtra *x) -> fontHandle`; cached by face name | high |
| 7 | `+0x1C` | ReleaseFont | `(fontHandle)` | high |
| 11 | `+0x2C` | CreateRenderTarget | `(SIZE *, byte, int) -> id` | medium |
| 12 | `+0x30` | ReleaseRenderTarget | `(id)` | medium |
| 15 | `+0x3C` | **DrawQuadF** | `(float ltrb[4], texId, color, float uv[8], int mode, uint alphaMode)` | high |
| 16 | `+0x40` | **DrawQuadI** | `(int ltrb[4], texId, color, float uv[8], int mode, uint alphaMode)` | high |
| 17 | `+0x44` | DrawQuad3D | `(float xyz[12], texId, color, float uv[8], int mode, uint alphaMode)` — what 15/16 expand to | high |
| 18 | `+0x48` | **DrawText** | `(float ltrb[4], stlp_wstring *text, int fontHandle, uint color, float clip[4], float scale[2], float offset[2])` | high (order), medium (scale/offset meaning) |
| 19 | `+0x4C` | DrawQuadEx | like 17 with an extra render-state word | low |
| 24 | `+0x60` | DrawLine? | two points + colour (cmd 5, 0x40 bytes); cf. class `CGUILine` | low |
| 27 | `+0x6C` | PushClip | `(float ltrb[4])` | medium |
| 28 | `+0x70` | PopClip | `()` | medium |

Notes read off `GuiCmd_DrawQuad3D` (`0x00CDE8D0`):

- `texId == 0` draws an untextured fill. That is how BACKGROUNDCOLOR and the
  progress bar's BARCOLOR fallback are drawn.
- `alphaMode == 0x100` means "use the alpha in `color`"; a value `< 0x100` replaces the
  vertex alpha with it.
- `mode 0` is the normal blend; 1 and 3 select other blend states; 2 sets a render
  state from the colour argument. Use 0.
- The default `uv` table the game passes is `{0,0, 0,1, 1,0, 1,1}` (e.g. at
  `0x0128BB50`), matching vertex order `LT, LB, RT, RB`.

## Text

### What a label holds

A label carries two TextSettings: the definition form at `+0x734` (TEXTGUID, alignment,
FONT, TEXTCOLOR, flags, template name) and the **live** one at `+0x8C8`, filled from
the first by `FUN_00DC1AA0` in `GuiWndLabel_OnCreate`. BUTTON's live TextSetting is at
`+0xA70`, EDITFIELD's at `+0x86C`.

```
TextSetting (0x2C)                 TextData (0xF0), lazily allocated at TextSetting+4
+0x00 CGUIWnd *owner               +0x00 mode   0 none | 1 GUID | 2 wchar_t* (external)
+0x04 TextData *                                3 wchar_t* (owned copy) | 4 stlp_wstring
                                   +0x04 flags  bit0/1 h-align (1 centre, 2 right)
                                                bit2/3 v-align, 0x10 laid out, 0x40 relayout pending
                                   +0x08 font id            +0x0C text-engine text id
                                   +0x10 wchar_t*           +0x14 GUID
                                   +0x18 stlp_wstring       +0x48/+0x4C owner client size
                                   +0x50 params block (offsets below are from +0x50):
                                        +0x38 flags, +0x48 colour, +0x78 FONT NAME (stlp_string) = TextData+0xC8
```

Setters, all on the TextSetting, all `__thiscall`:

| function | address | effect |
|---|---|---|
| `TextSetting_SetTextGuid` | `0x00D8B0D0` | `(int guid, char notify)` — mode 1 |
| `TextSetting_SetTextPtr` | `0x00D8B120` | `(wchar_t *s, char copy, char notify)` — mode 2, or 3 if `copy` |
| `TextSetting_SetTextW` | `0x00D8BAB0` | `(stlp_wstring *s, char notify)` — mode 4 |
| `TextSetting_SetHAlign` / `SetVAlign` | `0x00D8B220` / `0x00D8B270` | h 0/1/2, v 0/4/8 |
| Runtime BUTTON receives real clicks | **Works.** With an event trace in the hook, a runtime BUTTON under `SYSTEMMESSAGES` got `OnMouseEnter`, `OnLeftButtonDown`, `OnLeftClick`, `OnLeftButtonUp` from the real mouse. No registration needed; the hit test (`GuiWnd_HandleInputEvent` `0x00D61D80`) only needs `+0x9C` visible and the absolute rect at `+0x8C`. |
| Click shows/hides a runtime dialog | **Works.** The hook reacts to `OnLeftClick` on our button and calls SetVisible (`+0xAC`) on a runtime BOXLAYOUT with the gold message frame; four clicks toggled it 1-0-1-0. |
| `GuiTemplate_Apply` side effect | It overwrites PATHID `+0xC8` and ACCESSID `+0x109` with the template's names (skinned `ANNORE_BTN` became `INGAME_MENUBUTTONS_TEMPLATE`). Full-path lookups still work; name matching does not. |
| Dispatching events by name | **Works.** `GuiElement_DispatchNamedEvent(elem, &stlp"OnLeftClick", {0,0,elem,0}, 1)` on `LOADING_SCREEN/START_MISSION` started the loaded game. |
| `MainControl_LoadSavegameFile` | **Works from the main menu.** `(g_pMainControl, stlp_wstring* path)`, `__stdcall` RET 8 (bytes `c2 08 00` at `0x004B065E`). Posting it while the intro movie plays leaves the game with a broken mouse; wait ~120 s after launch. |
| font | `TextData+0xC8` (stlp_string font name, params block +0x50 / +0x78) | resolved **by name** in `TextEngine_LayoutText` (`0x00D48F80`) via `TextEngine_GetFontByName`, which takes its string **by value** (`RET 0x20`). `TextSetting_SetFont` (`0x00D8B1F0`, `+8`) is not the font. |

Widget-level wrappers: **`GuiWndLabel_SetTextW` (`0x00D7BC40`)** and
`GuiWndLabel_SetTextGuid` (`0x00D7BC10`), both also marking the parent layout dirty.

**The game uses the string path itself.** `0x004E2040` (renamed
`Game_SetPageButtonNumbers_SetTextExample`) walks a frame's children, and for every
BUTTON formats `L"%d"` into an STLport wstring with `FUN_00B070F0` and calls
`TextSetting_SetTextW(button+0xA70, &str, 0)`. The whole "labels only take GUIDs"
conclusion in `gui-system.md` came from reading the *definition* format; the runtime
API is wider.

STLport wide strings, from the same call sites: `FUN_00B074A0(wstring*)` empty ctor,
`FUN_00B074C0(wstring*, const wchar_t*)` ctor from C string, `FUN_00B06AE0(wstring*)`
dtor, `FUN_00B070F0(wstring*, fmt, ...)` printf-style format. 0x20 bytes each, same
layout family as the `StlpString_*` narrow versions already used by `annobridge`.

### Inline tags

`TextEngine_ProcessInlineTag` (`0x00D470D0`) interprets bracket tags while laying a
string out, so font, alignment, colour and icons are controllable from the string alone:
`[FONT name]`, `[CR]`, `[CR n]`, `[CENTER]`, `[RIGHTALIGN]`, `[LEFTALIGN]`, `[INDENT n]`,
`[COLOR AARRGGBB]`, `[DEFAULTCOLOR]`, `[BACKGROUND_COLOR AARRGGBB]`, `[ICONGUID n [w h]]`,
`[LEFTICON n n]`, `[RIGHTICON n n]`, `[IMAGE path]`, `[HYPERLINK …]`, `[TITLE n]`.
Font names are the ones the `.dlg` files use in `<FONT>`: "Mainmenu Subheader",
"Mainmenu Subheader Start", "Tooltip", and more in `localisation_addon.xml`.
**Confirmed live (2026-09-18):** `[FONT Tooltip][CENTER]Tags: [COLOR ffff8000]colour[DEFAULTCOLOR] ok`
set through `GuiWndLabel_SetTextW` rendered in the Tooltip font with the word in orange.
In-game, a runtime LABEL under INFOBAR with the brass panel layout copied from
`INFOBAR_INFO_BACKGROUND1` showed "Population: 1234 on First Island" over the map.

### From string to glyphs

```
GuiWndLabel_Render 0x00D7D2B0
  └─ TextSetting_Render 0x00D8B420
       └─ TextEngine_RenderText 0x00D49360     this = g_pTextEngine (0x012D3038), by text id
            └─ ProcessedText_Render 0x00DC6280 lines at +0x18, inline icons at +0
                 └─ TextLine_Render 0x00DC2F40
                      └─ TextPart_Render 0x00DC3EA0   a run of one font/colour; text at +4, CFontContainer* at +0x44
                           └─ FontContainer_DrawText 0x00D87E70
                                └─ g_pGUIRenderer->DrawText(pos, text, font+0x38, colour, clip, scale, offset)
```

`g_pTextEngine` is the `CTextEngine` (ctor `TextEngine_Construct` `0x00D48D30`; also
`*(g_pGuiManager+0x434)`). It owns the GUID tables (`TextEngine_GetTextByGuid`
`0x00D458A0` returns a `wchar_t*`), the processed-text objects by id (`+0x28`), and the
font map by name (`+0x10`). Layout of a changed TextSetting is deferred: setters clear
flag `0x10`, `TextSetting_Render` registers the setting with
`GuiManager_MarkTextDirty` (`0x00D77FA0`, set at `g_pGuiManager+0x190`), and the text
engine re-lays it before the next draw. Where exactly that consumer runs is not pinned
down, but the game's own dynamic labels rely on it, so a DLL can too.

### Fonts

`CFontContainer` objects are created by `TextEngine_LoadLocalisationXml`
(`0x00D49AD0`) from `<Font>` nodes under `<Localisation>` in
`data/loca/<lang>/localisation_addon.xml` — child elements `Name`, `FontName`, `FontSize`,
`RelativeFontSize`, `Weight`, style words (`Italic`, `Underline`, `StrikeOut`,
`Outline`, `Shadow`, `Antialiasing`, `TextureFont`), `ShadowOffset`, `DefaultColor`,
`ShadowColor`, `OutlineBorderColor`, and per-resolution `Width_N` / `Height_N` /
`Name_N` / `Weight_N`. English base/addon resources were staged on 2026-10-09
under `data/{maindata,addon}/eng0.rda/`; hashes are in the GUI Studio localization
fixture. The addon file declares 21 named fonts, including `Tooltip`, `Headlines`,
`Default`, `Default Bold`, `Mainmenu Subheader` and `ConsoleFont`. The raw resource
inspector shows both source candidates without choosing archive precedence.

Further static checks are recorded first in Ghidra plates: loader `0x00D49AD0`
registers locale-relative GameFont files with `AddFontResourceExA` flags `0x10`.
Font parameter helper `0x00D87870` uses `int(size * screenScaleY + 0.5)` for
relative sizes. `FontContainer_EnsureCreated` supplies this as LOGFONT height
with width zero. A browser CSS font size is therefore not sufficient evidence
of matching metrics. English text tables have UTF-16 BOM encoding; raw GUID
inspection preserves inline tags. Font aliasing, markup, metrics and raster
parity remain open.

Getting a font from code:

- `TextEngine_GetFontByName(g_pTextEngine, stlp_string *name) -> CFontContainer*`
  (`0x00D46310`; creates "MS Sanserif" if the name is unknown), or
- read `+0x44` off any live `TextPart` of a label that already looks right.

The renderer handle DrawText wants is `CFontContainer+0x38`, created on demand by
`FontContainer_EnsureCreated(font, 0)` (`0x00D879E0`). `FontContainer_MeasureText`
(`0x00D88240`, `(SIZE *out, const wchar_t *s)`) measures a string.

## Icons and backgrounds

- **Icons by GUID**: `IconManager_DrawIcon` (`0x00DB3320`) on the singleton from
  `IconManager_Get` (`0x00DB4210`, `0x012D56F8`). Labels call it as
  `(iconGuid=+0x878, flags=+0x87C, overlayGuid, float rect*, clip*, scale*, ?, 1,
  disabled, colour, colourFlag, 0.0f)`. Half the arguments are unnamed; UNVERIFIED.
- **Button icon sizing (resolved):** `GuiWndButton_ReadDefinitionNode` (`0x00D40140`)
  parses `ICONGUID` at `+0xA38`, `ICONSPACE` at `+0xA44..+0xA50`,
  `ICONSTRETCH` at `+0xA54`, and `ICONSCALE` at `+0xA58`. In the button renderer
  (`0x00D3FBA0`), `ICONSTRETCH=1` uses the button's full client rectangle; the
  `ICONSPACE` width/height do not become a smaller destination rectangle. The
  reliable way to place a small icon in a button is a child `LABEL` with its own
  `EXTENT` and `ICONGUID`, `ICONSTRETCH=1`; the production button's marketplace
  icon is live-verified this way at 28x28. `ICONSCALE` exists, but is a transform
  adjustment rather than a direct pixel-size setter.
- **Backgrounds** are `CBackgroundLayout` objects (`+0x580`) created by
  `GuiWnd_SetBackground` (`0x00D62410`, vtable `+0xBC`, takes the BACKGROUND name)
  from the manager at `g_pGuiManager+0x43C`, and drawn by `GuiWnd_RenderBackground`
  (`0x00D61B60`) through `FUN_00DBFE30`. The progress bar's `PB_Bar` /
  `PB_Overlay` / `PB_Background` are sub-layouts of that object.
- **Effective alpha/colour/scale** of an element, with parent inheritance, come from
  `GuiWnd_GetEffAlpha` (`0x00D5AD20`, float at `+0x20`), `GuiWnd_GetEffColor`
  (`0x00D5AD70`, `+0x24`), `GuiWnd_GetEffColor2` (`0x00D5B640`, `+0x28`) and
  `GuiWnd_GetEffScale` (`0x00D5AD40`, two floats at `+0x18`).

## Input: how a click finds an element

`GuiWnd_HandleInputEvent` (`0x00D61D80`, CGUIWnd vtable `+0xEC`) is recursive and
first-child-wins:

1. invisible (`+0x9C` == 0) returns 0;
2. the point is tested against the **absolute** rect at `+0x8C..+0x98` (the layout pass fills
   it from the parent's `+0x8C` origin plus the element's relative `+0x2A4`);
3. each child in the `+0x80` list gets the event if it has flag `0x800000` or the parent
   contains the point; the first non-zero return ends the walk, so an earlier sibling that
   covers the point swallows the click whatever is drawn on top;
4. `GuiWnd_IsInputTransparent` (`0x004F0060`): flag `0x20000000` or effective alpha < 0.001
   means "not me";
5. hover transfer to `g_pHoverElement` (`0x012D32CC`), then a switch on the event type calls
   the per-event virtual, which ends in `GuiElement_DispatchNamedEvent` with a name from the
   per-element table `GuiWnd_InitEventNameTable` fills at `+0x6A8`.

The event name reaches the dispatcher as an `stlp_string*`. Short-string layout: `+0x00`
is a 16-byte inline buffer *or* a heap pointer, `+0x10` finish, `+0x14` end-of-storage;
`+0x14 == obj+0x10` means the text is inline at the object itself. Every mouse event name
except the `IncludingChildren` variants is inline.

Event names seen live on a button, in order, for one click: `OnMouseEnter`,
`OnMouseEnterNormal`, `OnMouseEnterIncludingChildren`, `OnLeftButtonDown`,
`OnLeftButtonDownNormal`, `OnLeftClick`, `OnLeftClickNormal`, `OnLeftButtonUp`,
`OnLeftButtonUpNormal`, `OnMouseExit*`. The `*Normal` / `*Toggled` variants are the
button-state-specific copies.
## CGUIWnd virtuals that matter

Indices verified by diffing the Label, ProgressBar, Frame and Desktop vtables
(`0x010A5164`, `0x010A7784`, `0x010A4ACC`, `0x010A8014`; 72 slots).

| idx | offset | meaning | base impl |
|---|---|---|---|
| 22 | `+0x58` | OnCreate — resolves BACKGROUND (`+0x199`), fires `OnCreate` event | `GuiWnd_OnCreate` `0x00D6DC00`; PB `0x00DB9E10`; Label `0x00D7BD90` |
| 32 | `+0x80` | RenderTree(CRect) | `GuiWnd_RenderTree` `0x00D658D0` |
| 33 | `+0x84` | Render(CRect) — content | Label `0x00D7D2B0`, PB `0x00DB9EF0`, Frame: empty |
| 34 | `+0x88` | RenderBackground | `0x00D61B60` |
| 36 | `+0x90` | ReadDefinitionNode (base tags) | `GuiElement_ReadDefinitionNode` `0x00D6E1A0` |
| 37 | `+0x94` | ReadDefinitionNode (type tags) — what `GuiTemplate_Apply` calls | PB `0x00DB9A40` |
| 41 | `+0xA4` | AddChild(child, after, flag) | `GuiWnd_AddChild` `0x00D6E000` |
| 43 | `+0xAC` | SetVisible(bool, 1, 0) | |
| 47 | `+0xBC` | SetBackground(stlp_string *) | `GuiWnd_SetBackground` `0x00D62410` |
| 49 | `+0xC4` | SetBackgroundObject(ptr) | `0x00D5AFC0` |

## Names: the registry's two maps

`GuiRegistry_Get()` → `0x012D8F78`. `+0x1AC` is keyed by ACCESSID (every element
self-registers in `GuiWnd_Construct`). **`+0x254` is keyed by full path**, built by
`GuiWnd_GetFullPathId` (`0x00D64610`): the parent's full path, `/`, the element's
PATHID, with root-level elements keyed by bare PATHID. `GuiRegistry_RebuildPathMap`
(`0x00D53BF0`) clears and refills it from the all-elements list at `+0x26C`, and
`GuiRuntime_CreateElement` calls it after attaching the child. So a runtime-created
`ANNORE_PANEL` under `INFOBAR` is findable as `"INFOBAR/ANNORE_PANEL"`, and that is the
string to pass as the parent name when nesting. Read from code, not yet tried live.

## What this means for the companion tool

Two routes now, both native-looking, both inside the existing `annobridge` hook model.

**Route 1 — widgets, with real text.** Create a LABEL (type 1) under `INFOBAR` with
`GuiRuntime_CreateElement`, apply the template index copied from a game label that
looks right (its `+0x14C`), write the EXTENT at `+0x2A4`, call its OnCreate (vtable
`+0x58`) so the background gets built, then each frame or on change call
`GuiWndLabel_SetTextW(label, &wstr, 1)` on the GUI thread. Layout, font, colour,
clipping and tag processing are the game's. This is the smallest change to what
already runs: one more game call and a wide-string constructor.

**Route 2 — draw directly.** Hook `GuiManager_Render` (or `GuiWnd_RenderTree` and act
when `this` is the root). After the original returns, call `g_pGUIRenderer->DrawQuadI`
for panels and `->DrawText` with a font handle borrowed from a live label. No widgets,
no templates, no registry, no extents; full freedom over layout; but you own hit
testing and you draw over everything including popups. Good for a HUD overlay, wrong
for anything that should behave like a menu.

Either way, the two things that blocked progress have concrete fixes to try:

1. Flat-rectangle progress bar → call OnCreate (`+0x58`) after `GuiTemplate_Apply`;
   if `+0x199` is empty afterwards, copy the BACKGROUND string from a live bar's
   `+0x199` and call `SetBackground` (`+0xBC`) with it.
2. Nesting → parent name `"INFOBAR/<your PATHID>"`.

## Live results (2026-09-18, main menu and in-game, `tools/production-menu` v6)

Tested from the injected DLL against the running game. Screenshots were taken of the
windowed game; everything below was seen on screen.

| Claim above | Result |
|---|---|
| Arbitrary text via `GuiWndLabel_SetTextW` | **Works.** A runtime LABEL under `MAIN_MENU` shows "Hello from annobridge" in the game font. A BUTTON's caption via `TextSetting_SetTextW` on `+0xA70` works the same way. |
| Real art via `GuiWnd_SetBackground` | **Works.** A copied `mainmenu_start_button_white.bkg` gives a runtime LABEL, BUTTON and PROGRESSBAR the brass main-menu button art. Flags after success: `+0x588` = 1, `+0x58A` = 0. |
| Full-path names in the registry | **Works.** `MAIN_MENU/ANNORE_BAR2` resolves through `GuiRegistry_FindElementByName`; a zero-sized, never-rendered element can be sized, skinned and given text this way. |
| Progress-bar art | **Works.** With the layout name passed intact, `INFOBAR/ANNORE_BAR` given `.../housemenu/housemenuprogressgreen.bkg` draws the olive-green `PB_Bar` gradient clipped to the value, and `.../general_progressbars/tiny_progressbar.bkg` draws the framed silver bar with its `PB_Overlay`. `+0x580` set, `+0x588` = 1, `+0x58A` = 0. Every earlier failure was annobridge truncating the path to 64 bytes (no `.bkg` extension, so `GuiWnd_SetBackground` took its no-load branch). The house-menu layout has no frame of its own: `PB_Background` is a single 1x1 icon 1023 (empty); the frame in the real menu is the parent's art. |
| `GuiTemplate_Apply` | Skin only, confirmed again: it does not set BACKGROUND, EXTENT or the font. |
| Children are clipped to the parent | **Confirmed in-game.** `INFOBAR` is `(0,3)-(1365,195)`; a label at y 190..230 under it showed as a 5px strip and a bar at y 150..174 lost its lower half. Placing them inside y<195 fixed both. |
| Child extents are parent-relative | **Confirmed.** A label at `(20,20,380,60)` under a panel at `(400,300,800,480)` drew at screen 420,320. An "absolute" child rect lands off-screen. |
| Full-screen parent for free placement | `SYSTEMMESSAGES` (FRAME, extent `(0,0)-(1280,1024)`, live in-game) works; `TEMP_FRAME`, `MOUSE_BLOCKER_FRAME` and `FADE_SCREEN` never dispatched events during the test. |
| BOXLAYOUT panel with dialog art | **Works.** A runtime BOXLAYOUT under `SYSTEMMESSAGES` with the copied `GLOBAL_INFO_BACKGROUND` layout draws the dark HUD pill; with the explicit `.../general_frames/bigmessagebackgroundgold.bkg` it draws the full gold message frame including its side tab. |
| Nested children under a runtime panel | **Works.** `GuiRuntime_CreateElement(1, "SYSTEMMESSAGES/ANNORE_PANEL", "ANNORE_P_LBL")` resolves the runtime panel as parent through the full-path map; the label renders inside the frame with tags and colours. |
| Font by name at `TextData+0xC8` | **Works.** Writing "Mainmenu Subheader" then "Tooltip" into the string and clearing flag 0x10 switched a live label between the dark serif and the white shadowed font. |
| `TextSetting_SetHAlign` / `SetVAlign` | **Work.** H: 0 left, 1 **right**, 2 **centre** (verified on screen; the reverse of the obvious guess). V: 0 top; 4 and 8 both move the text down, which is middle and which bottom is unverified. |

Two hard lessons, both crashes, both fixed in v6:

- **Everything you call from the dispatcher hook can re-enter the hook.** The factory
  fires events while attaching; OnCreate fires `OnCreate`. Without a busy flag the
  create request ran hundreds of times and OnCreate recursed until the stack overflowed
  inside `GuiWndProgressBar_OnCreate`.
- **`GuiRegistry_FindElementByName` is `RET 8`**, i.e. two stack arguments (name and a
  zero). Calling it with one corrupted the caller's frame and the game died in the
  DLL's own epilogue with ESP = 0. The Ghidra prototype and plate comment are corrected.

## The XML layer at runtime (2026-09-20)

The `.dlg`/`.tpl` sources are not only compiler input. `Addon.exe` keeps a live XML DOM and
parses template files from the RDA **lazily at runtime**; every `TEMPLATE_REF_ID` the game
applies goes through it. This resolves Tier 1 topics 1 and 3 of `ui-investigation-plan.md`.

### Documents and nodes

| thing | where | notes |
|---|---|---|
| `CConfigDocContext` (0x104) | `*(g_pGuiManager+0x42C)` | ctor `XmlDocContext_Construct` `0x00428670`. Embedded root node at `+4` whose children are the loaded documents, keyed by normalised absolute path (`C:/.../ADDONDATA/CONFIG/GUI/MENUDEFINITIONS/BUTTON_TEMPLATE.TPL`). Node pool at `+0xC4..`, list-node pool at `+0xC0`. |
| `CConfigDocTemplate<CRDString>` (0x1C) | stack object | `+4` root node, **`+8` current node** (all tag lookups are relative to it), `+0x14` ctx. Ctors: `XmlDoc_ConstructByName` `0x00429E50` (open/parse/cache by path), `XmlDoc_ConstructEmpty` `0x00484EC0` (fresh `<unnamed>` root), `XmlDoc_ConstructAtNode` `0x00D920C0`; dtor `XmlDoc_Destruct` `0x0042CEC0`. |
| `CXMLExtNode<CRDString>` (0x5C) | pool | `+0` vftable, `+4` isComment, `+8/+0xC` child list (list node = `{node*, next}`), `+0x14` parent, `+0x18` pool, **`+0x1C` name, `+0x3C` value** (both `stlp_string`). `XmlNode_Construct` `0x0042CD80`, `XmlNode_InitWithName(node, name, ctx+0xC0)` `0x0042C160`, `XmlNode_AppendChild(parent, node)` `0x0042C0D0`. |
| readers | | `XmlDoc_FindNodeByPath(doc, "A/B")` `0x0042B070`, `XmlDoc_GetChildText(doc, out, tag)` `0x00485020`, `XmlDoc_ReadIntTag` `0x0042A090`, `XmlDoc_ReadLongTag(doc, long*, tag)` `0x00DC1DE0`. |
| parser | `XmlDoc_LoadFile` `0x0042ACF0` | Opens the path through `CEngineFile` (`0x00B28B60`, RDA-aware) and streams it through a 16 KB tokenizer into nodes (`XmlDoc_ParseTextFile` `0x0042BC70`; UTF-16 variant `0x0042B650`). Tags and text only, no attributes; `<!-- -->` becomes a comment node. **It takes a path, not a string.** Whether a loose file with a name absent from the RDA is readable is untested (topic 16). |

`tools/research/annogui.py xmldocs` lists the documents in the live context (54 `.tpl` after half an
hour of play, all parsed on first use).

### Element factories from XML

```
GuiElement_CreateFromXmlNode(doc, node)          0x00DA14F0   one element, type = node name
  └─ <Type>_ConstructFromXml(this, doc)           LABEL 0x00D7CD10, BUTTON 0x00D3D8D0, BOXLAYOUT 0x00D8D9C0,
        └─ GuiWnd_ConstructFromXml 0x00D69070      FRAME 0x00D8BC60(doc,0), SLIDER 0x00DA27F0, PROGRESSBAR 0x00DB9DA0(doc,9), ...
        └─ vtable+0x90 ReadDefinitionNode(doc, bLive=0)
GuiElement_InstantiateXmlSubtree(parent, doc, idx) 0x00DBA940  all siblings from doc+8, recursive,
                                                               AddChild on each (which fires OnCreate)
```

The second one is what `STRUCTTEMPLATEINSTANCE` uses to clone an `.stp` subtree
(`GuiWndStructTemplateInstance_Instantiate` `0x00DBACE0`; only `EMPTYSTRUCTTEMPLATE` ships).
It is type-agnostic: point `doc+8` at the first element node of any tree and it builds the
whole dialog under `parent`, property tags skipped, children nested.

**Verified live 2026-09-26 (annobridge v8, `--xml`):** `tools/production-menu/examples/scrollbox.dlg`,
a BOXLAYOUT with `BACKGROUND`, `SCROLLBAR_VERT` 2, `SCROLLBAR_VERT_TEMPLATE_REF_ID`, ten LABEL rows
with `TEXTSETTING` (TEXTGUID + FONT) and a BUTTON with `TEMPLATE_REF_ID` and an `EVENTHANDLER`,
became 134 nodes and one live subtree under `SYSTEMMESSAGES`: gold frame, ten localised rows in
the Tooltip font, the game's own scrollbar, the button skinned with template 122, all in one call
(`docs/img/xml-scrollbox-2026-09-26.png`). The DLL builds the nodes with `XmlNode_Construct` +
`XmlNode_InitWithName(node, name, ctx+0xC0)` + `XmlNode_AppendChild`, sets values with the
STLport assign `FUN_00418950(node+0x3C, begin, end)`, wraps them with `XmlDoc_ConstructEmpty` on
the GUI context, sets `doc+8` to the first element node and calls
`GuiElement_InstantiateXmlSubtree(parent, &doc, -1)` (`__stdcall`, three args).

One thing the factory does not do: **register**. `GuiWnd_ConstructFromXml` never calls
`GuiWnd_RegisterInRegistry` (`0x00D6DB10`), which the `.cdf` loader runs after each element to
put it into the registry's all-elements list (the source of the full-path map), the ACCESSID map
and the GROUP map. Without it the new elements render and receive events, but
`SYSTEMMESSAGES/ANNORE_X_SCROLL` does not resolve and a declared `SETVISIBLE` handler finds no
target. annobridge v8.1 walks the new subtree and calls it for every element before
`GuiRegistry_RebuildPathMap`. Nodes are never freed (a few KB per call) and the document is not
destructed, because `XmlDoc_Destruct` would hand an unattached root back to the game's node pool.

### What a template really is

`CGUITemplate` = `{ char path[0x104]; char accessId[0x41]; char templateRefId[0x41]; }`, 881 in
the vector at `GuiRegistry+0x1DC`, name→index map at `+0x1E8` (`tools/research/annogui.py templates`).
`GuiTemplate_Apply(tmpl, elem)` opens the `.tpl` as a document, positions `doc+8` on the
element node inside `<TEMPLATE>` and calls `elem->vtable[0x94](doc)`, a thunk for
`ReadDefinitionNode(doc, bLive=0)`. So a template copies **every tag the type reads** (names,
extent, background name, text settings, event handlers, scrollbar flags) but creates no
children and no background object. That is the whole "it renamed our element" story.

Use **`GuiWnd_ApplyTemplateByIndex(elem, idx)` `0x00D6DE00`** instead of the raw call: it saves
and restores ACCESSID/PATHID, stores `+0x14C`, rebuilds the background from `+0x199`, keeps
visibility and ZORDER (`+0x2B4`). It is what the live `TEMPLATE_REF_ID` path and the scrollbar
creation use.

`GuiWnd_AddChild` (`+0xA4`) calls the child's **OnCreate** after linking. Any element attached
through it is live; `GuiRuntime_CreateElement`'s widgets only looked dead because `+0x199` was
still empty when OnCreate ran.

## Layout and scrolling (2026-09-20)

### The layout pass

`GuiWnd_RelayoutTree` `0x00D6FCC0` runs every frame from `GuiManager_Render` on the root,
before `RenderTree`:

1. `GuiWnd_ApplyLayoutMode` (`LAYOUTMODEH/V` `+0x570/+0x574`: 1 fill, 2 near, 3 centre, 4 far, against the parent client rect);
2. `vtable+0x78`, recurse into visible children, `vtable+0x7C`;
3. `+0x7C` base = dispatch the `OnRender` named event (this is where `<EVENTHANDLER>` `FITSIZETOCHILDREN` runs); the BOXLAYOUT override first runs `GuiWndBoxLayout_ArrangeChildren` → `GuiWndBoxLayout_PlaceChild` `0x00D8DC60` per visible child: keeps the child's WIDTH/HEIGHT, overwrites LEFT/TOP with a running cursor (`LINE_ORDER +0x6F0` 0 vertical / 1 horizontal, `LINECOLUMNCOUNT +0x6F4`, `XSPACE/YSPACE +0x6F8/+0x6FC`, `LEFT_TO_RIGHT/TOP_TO_BOTTOM +0x700/+0x701`, `BOXLAYOUT_BEHAVIOUR +0x702`);
4. content rect = union of children's relative rects → `GuiWnd_UpdateScrollbars` if `+0x2B8 != 0`.

`FITSIZETOCHILDREN` is `vtable+0xD0` (`GuiWnd_FitSizeToChildren` `0x00D5F980`, BOXLAYOUT override
`0x00D8DED0`). `ALIGNMENT_*` is separate: `GuiWnd_RecomputeAlignment` runs lazily when the
element's stamp `+4` differs from the global `DAT_012D3350`; `GuiWnd_SetExtent(l,t,r,b)`
`0x00D5B150` writes `+0x2A4` and bumps the stamp.

Absolute rects `+0x8C` are written by `RenderTree` from `GuiWnd_GetAbsRect` = parent origin +
relative rect − parent scroll offset, and **only for elements whose rect intersects the clip**.
A row scrolled or clipped out of view keeps a stale `+0x8C`; that is the "row beyond the box
got no rect" observation.

### Scrolling

| field | meaning |
|---|---|
| `+0xA4` / `+0xA8` | current scroll offset x / y, subtracted from every child's position |
| `+0xAC` / `+0xB0` | **target** x / y; `GuiWnd_TickScrollAnimation` eases current → target over 300 ms with a sine |
| `+0x2B8` | scrollbar flags: `0x01` horz, `0x02` horz auto-hide, `0x04` vert, `0x08` vert auto-hide, `0x10/0x20` reset-on-show. `<SCROLLBAR_VERT>n`: 1→`0x04`, 2→`0x0C`, 3→`0x24`, 4→`0x2C` |
| `+0x6C0` / `+0x6C4` | the horizontal / vertical SLIDER child (ACCESSID `___HORZ_SCROLLBAR<n>` / `___VERT_SCROLLBAR<n>`, empty PATHID), created by `GuiWnd_UpdateScrollbars` `0x00D6F3C0` |
| `+0x150` / `+0x154` | `SCROLLBAR_HORZ/VERT_TEMPLATE_REF_ID` index applied to that slider; **−1 on a runtime element → zero-width invisible bar** |
| slider `+0x6F8` / `+0x6FC` | value / range (= content − client), `+0x714` step, `+0x718` page (= client size) |
| `+0x2A0` bit 31 | `AUTODOWN`: stay pinned to the bottom when content grows |

Live results, runtime BOXLAYOUT `SYSTEMMESSAGES/ANNORE_T1_SCROLL` (300x220, gold frame) with 12
runtime LABEL rows of 26 px, `+0x2B8 = 0x0C`, `+0x6F0 = 0`:

- Rows stacked at relative tops 0, 26, … 286 with no rect work beyond their size. Rows 9–12 clipped.
- `___VERT_SCROLLBAR2` appeared at `+0x6C4` with range 113 (= 312 − 200 + 1) and page 200, but extent `(300,0,300,200)`: zero width, never rendered, because `+0x154` was −1.
- Writing `+0xB0 = 60` scrolled the rows (row 5 abs top 404 → 344, screenshot) and the game pushed 60 into the slider. `+0xB0 = 80` likewise. **This is the programmatic scroll API.**
- **Mouse input on the runtime scrollbar is fully native** (2026-09-26, real mouse via
  `tools/production-menu/dev/mouse.py`): the wheel over the rows stepped the slider by its STEPSIZE
  (20) and scrolled the rows; the arrow buttons stepped by 20; a click on the track below the
  knob paged; dragging the knob set the value continuously (`OnChangeValue` on the slider each
  step). The wheel is implemented in `GuiWnd_OnMouseWheelUp/Down` (`0x00D6C760` /
  `0x00D6C910`, vtable `+0x4C`/`+0x50`, input types 0xE/0xD): they walk up the parent chain,
  find the first ancestor whose `___VERT_SCROLLBAR<n>` resolves in the registry, step that
  slider and call `GuiWnd_SetScrollY`; only when no ancestor scrolls do they dispatch the
  named event. That is why `--click NAME --event OnMouseWheelDown` (the 2026-09-20 test)
  never scrolled: the scroll happens before the named event, in the input path.
- Poking the slider's `+0x6F8` directly does not scroll: `GuiWndSlider_PollDragChanged` needs
  the slider's own drag timestamp. Write the container's `+0xB0` instead.
- Two things that cost the afternoon: the game only reads the mouse while its window is the
  **foreground window** (a background process must tap ALT before `SetForegroundWindow`), and
  `GuiWnd_SetVisible` takes an **int** `visible` that is stored verbatim at `+0x9C` and
  compared with `== 1`; annobridge passed a `char`, the upper bytes were register garbage
  (`0x010A6101`), and the box silently stopped rendering. Fixed in v8.2.
- Applying template 573 `SCROLLBAR_VERT_TEMPLATE` to the existing slider (`--skin-index`) gave it the template's extent and it snapped to the client's right edge; a following `--oncreate` built its background, and the **game's own arrowed scrollbar with a knob at the scroll position rendered inside the frame** (`docs/img/scrollbox-2026-09-20.png`, right panel). For a fresh box, set `+0x154` before the bar is created; note the relayout only calls `UpdateScrollbars` when `+0x2B8 != 0`, so clearing the word does not remove an existing bar (clear only bit 4 instead).

## Event handlers and action verbs (2026-09-20)

Handler map at `element+0xC0` (created lazily): STLport map event-name → `GuiEventHandlerList`
(0x2C: `+0` name, `+0x20/+0x24/+0x28` vector of records). Record `GuiActionRecord` (0x54):

```
+0x00 int         id      = GuiManager_GetActionId(verb)   0x00D75560 (map at mgr+0x8C), -1 if unknown
+0x04 stlp_string verb    tag name        e.g. "SETVISIBLE"
+0x24 stlp_string value   tag text        e.g. "ACTION_BUTTON_BOOK_FRAME; toggle"
+0x44 vector<stlp_string> args  value split at ';' and trimmed (GuiActionRecord_SplitArgs)
+0x50 u8          queued  from mgr+0xA4; run deferred on the main thread
```

Execution: `GuiEventHandler_ExecuteAction` → the element's own listener list `+0x578`
(`vtable+0x10(record, args)`, −1 = not mine) → `GuiManager_DispatchAction` `0x00D78C90` →
listener vector `mgr+0xBC[id >> 20]->vtable[0x10]`. The listeners are the menu controllers
(`CMainListener` = group 1 with the generic UI verbs, `CInfobarListener`, `CDiplomacyMenuListener`, …).
`tools/research/annogui.py verbs` dumped **613 verbs** live; the full table with ids is `docs/gui-actions.md`.
Adding a data-driven handler at runtime = `GuiEventHandlerList_AddRecord(list, &verb, &value)`
`0x00496890` on the list for that event name (or let `ReadDefinitionNode` build it from an
`<EVENTHANDLER>` node). Parser: `GuiEventHandler_ParseHandlerNode` `0x00DC2A90`.

## Tier 2 results (2026-09-26): tooltips, modal frames, z-order, icons, alpha, sequences

All tested with `tools/production-menu/examples/tier2.dlg` instantiated through `--xml`
(`docs/img/tier2-2026-09-26.png`, `docs/img/tier2-tooltip-2026-09-26.png`).

- **Tooltips.** `<TOOLTIP><TEXT>guid</TEXT></TOOLTIP>` (plus optional `ALIGNMENT`, `REFWINDOW`)
  fills the 0x50-byte block at `+0x2BC`; when the hovered element's guid changes, `RenderTree`
  calls `ToolTipManager_ShowForElement` (`0x00D8A420`) on the singleton `DAT_01304660`. Per frame
  `ToolTipManager_Update` (`0x00D8AC30`) applies a pending request once: `TextEngine_LookupGuid`
  → `ToolTipManager_SetText` (`0x00D8A870`, takes a plain `wchar_t*`, knows `[TTT layout]`
  sections) → `ToolTipManager_LayoutStandard` (`0x00D89810`, writes the strings into
  `tooltip_frame.dlg`'s labels and sizes them), then shows `TOOLTIP_FRAME` after
  `TooltipShowDelay` (`DAT_0128BE10`). Live: an XML label with guid 10140059 showed the honour
  tooltip. Arbitrary text: annobridge v8.4 `--tooltip NAME TEXT` re-runs SetText (mode 1) or
  SetText + LayoutStandard (mode 2) from the hook once per show; mode 2 crashed the game once,
  mode 1 is untested. See the plan's HANDOFF.
- **Modal frames.** `<FRAME_MODAL>1</FRAME_MODAL>` is flag 0x200 in `+0x2A0` (`FRAME_POPUP`
  0x800). `GuiWndFrame_GetOrCreateModalBackground` (`0x00D8C340`) creates a
  `CGUIWndModalBackground` wrapper (type 14, ZORDER INT_MAX-1, fills the parent) at `+0x7B0` and
  reparents the frame into it; the `.cdf` loader attaches the wrapper instead of the frame, and
  `GuiWndFrame_SetVisible` (`0x00D8C7D0`, the frame's vtable `+0xAC`) shows/hides the wrapper with
  the frame. The XML factory attaches the bare frame, so annobridge wraps it afterwards
  (`GuiWndFrame_WrapInModalBackground` `0x00D8C470`, deferred until after the registration walk).
  Live: SHOW_MODAL showed frame plus wrapper, a click behind was swallowed, MODAL_CLOSE hid both.
  Its full path contains an empty component: `SYSTEMMESSAGES/ANNORE_T2//MODAL`.
- **Z-order.** `GuiWnd_SetZOrder` (`0x00D6DDC0`) sorts the parent's child list ascending
  (`GuiWnd_SortChildrenByZOrder` `0x00D6D4B0`). Render is list order; the hit test walks the list
  backwards. Two overlapping labels with ZORDER 2 (declared first) and 1: the z=2 one drew on top
  and received the click.
- **Icons.** `<ICONGUID>` + `<ICONSTRETCH>1</ICONSTRETCH>` on a LABEL draws the icon (1160 coins,
  1156 honour, 34020 cross); inline `[ICONGUID n]` draws inside text. `[ICONGUID n w h]` did not
  render in this test.
- **Text.** `<ALIGNMENT_VERTICAL>` 0/1/2 in `TEXTSETTING` = top/middle/bottom (verified);
  `<ALIGNMENT_HORIZONTAL>` 0/1/2 is stored as 0/4/8. Registered fonts (`tools/research/annogui.py fonts`):
  ConsoleFont, Credits*, Default, Default Bold, Default Shadow, H1, H2, Headlines, Hyperlink,
  IngameMessages, LoadingBarText, Mainmenu Header/Subheader/Subheader Start/text/text black, Timer,
  Tooltip. `AUTOUPDATE` is field [3] of the TextSetting definition; its consumer is not traced.
- **Alpha and sequences.** `<ALPHA>0.4</ALPHA>` → `+0x3BC`, effective alpha `+0x20`; rendered
  translucent. `EVENT_REACTION_SEQUENCE` (a sequence XML with markers named after events) scaled
  the label to 1.1 on hover. `START_SEQUENCE file; marker; target1, target2` →
  `SequencePlayer_Start` (`0x00D9C400`): the file is parsed once (cache at player `+0x18`), an
  instance is recorded on each target (`+0x6D0`), and **track i animates target i**; the `FADEIN`
  and `FADEOUT` verbs are empty stubs. The two-target fade test is still to be run.
- **Pitfalls.** Wrapping a modal frame while iterating its parent's child list frees the node
  under the iterator and hangs the GUI thread; the fresh wrapper is visible by default and blocks
  its siblings until the frame goes through `GuiWndFrame_SetVisible`.

## Open

The standalone editor now verifies a narrower native font path: renderer font
command 0x3D reaches the custom GDI atlas builder A470E0; draw command 0x48 reaches
glyph quads A46710. The read-only renderer fixture selects D3D10 mode 3. Two font
snapshots each reproduce 5,632 page-zero glyph metadata records across 22 resources.
The direct D3DX font wrappers are a separate path and do not establish GUI parity.

Font XML shadow settings and outline border colors are parsed in document order.
An Outline flag resets its border color to FF000000; later OutlineBorderColor can
override it. Eight named font definitions match captured command parameters,
including active shadow offsets/colors and outline color. An active shadow without
explicit ShadowColor remains unsupported because its initialization is unverified.
Oversized decimal colors are not silently wrapped. The inspector provides native
single-run samples with explicit ARGB input; source DefaultColor overflow, runtime
caption/tag resolution, fractional sampling and translucent pixels remain open.
One independently supplied island-name witness matches 370 fully opaque pixels.
See findings `gui_native_glyph_atlas` and the editor witness manifest for its scope.

- Does `XmlDoc_LoadFile` read a loose file whose path is not in the RDA? (topic 16)
- Font names used by the shipped `.dlg` files (uses, counted in `data/addon/data1.rda/.../menudefinitions`): "Mainmenu Subheader Start" 117, "Tooltip" 110, "Mainmenu Subheader" 93, "Mainmenu text black" 32, "Headlines" 27, "Mainmenu Header" 19, "LoadingBarText" 11, "Mainmenu text" 10, "Default Bold" 5, "IngameMessages" 4, "H1" / "H2" / "Default" / "ConsoleFont" 2 each. Which of these the in-game font container registers is not enumerated yet.
- VAlign: which of 4 / 8 is middle and which is bottom (needs a tall label).
- Detect the end of the intro movie from the GUI tree instead of a 120 s timer (the main menu is live while it plays; `MAIN_MENU_BACKGROUND/BACKGROUNDVIDEO` is a VIDEOPANEL worth watching).
- Exact meaning of DrawText's `scale[2]` / `offset[2]`; slots 19–26.
- Does a draw call from the `OnRender` dispatcher hook land in the current frame?
- The consumer of the dirty-text set at `g_pGuiManager+0x190`.
- ~~Whether `GuiTemplate_Apply` reaches `ReadDefinitionNode` with `bLive` set~~ — resolved: it passes 0; OnCreate (fired by AddChild) builds the background.
