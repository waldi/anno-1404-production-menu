# The GUI system

What Anno's in-game UI actually is, and what that means for putting a custom
menu inside the game rather than beside it. Written while answering one
question: *can a companion tool render into the game natively, or does it have
to be a separate window / DirectX overlay?*

Function behaviour and the derivations live in Ghidra (see the address table at
the end). This file records the shape of the system and the decision it forces.

Sources for everything below: `Addon.exe` in Ghidra, plus the extracted
`addon/Data1.rda` tree at `data/addon/data1.rda/addondata/config/gui/`.

The three questions this answers, up front:

| Question | Answer |
|---|---|
| Is the UI hardcoded, or loaded from data? | Loaded. A declarative XML widget toolkit — 967 definition files, 383 distinct tags — compiled into `anno.cdf`. |
| Can a new menu with **working buttons** be added with data files alone? | **Yes.** `<EVENTHANDLER>` binds events to ~200 built-in actions, including `EXECUTE_CONSOLE_COMMAND`. No injection. |
| Can it display *our* numbers — per-island population from `annomem.py`? | **Yes, with an injected DLL, and it is a one-dword write.** A progress bar's value and a slider's value are plain `long`s on the element. Arbitrary *text* is harder and goes through substitution tags. |

> **Superseded in part (2026-09-18).** `docs/gui-rendering.md` goes one layer down,
> to how widgets are drawn, and corrects three things stated below: a label **does**
> take an arbitrary wide string at runtime (`GuiWndLabel_SetTextW`, `0x00D7BC40`), so
> the `[CONTEXT_STRING]` route is unnecessary; the flat-rectangle progress bar is a
> missing background object, fixed by calling the widget's OnCreate; and the
> registry's `+0x254` map is keyed by full path (`INFOBAR/CHILD`), which is how to
> nest under a runtime-created parent.

> **Correction.** An earlier revision of this file concluded that data-only
> menus get "chrome, not behaviour". That was wrong, and the extracted RDA is
> what corrected it. Behaviour is substantially data-driven. The real boundary
> is narrower and sits elsewhere: **our own data**, not behaviour.

## The format is plain XML, and it ships compiled

`addondata/config/gui/menudefinitions/` holds 967 files:

- **`.dlg`** (85) — dialogs, i.e. menus
- **`.tpl`** (881) — reusable templates, referenced by `<TEMPLATE_REF_ID>`

The smallest one in full:

```xml
<FRAME>
  <PATHID>INGAME_MESSAGES_FRAME</PATHID>
  <TEMPLATE_REF_ID>FULLSCREEN_MENU_TEMPLATE</TEMPLATE_REF_ID>
  <ACCESSID>INGAME_MESSAGES_FRAME</ACCESSID>
  <TRANSPARENT>1</TRANSPARENT>
  <ZORDER>-10</ZORDER>
  <ALIGNMENT_HORIZONTAL>1</ALIGNMENT_HORIZONTAL>
  <ALIGNMENT_BOTTOMVERTICAL>2</ALIGNMENT_BOTTOMVERTICAL>
  ...
</FRAME>
```

`PATHID` is the element's **string name** — the question the binary alone left
open. `ACCESSID` is the separate handle C++ uses to reach an element, and the one
`hidden.ini` / `disabled.ini` key on.

The game loads **`anno.cdf`** (18.6 MB), a binary serialisation of the built
widget graph — raw pointers and element names in the clear, not XML. The `.dlg`
and `.tpl` files are its *sources*.

**The compiler is in the retail binary, and it works.** Confirmed by running it:
`Addon.exe /compilegui` writes `<install>/addondata/config/gui/anno.cdf` at
18,644,566 bytes against the shipped 18,654,609 — a full recompile of the whole
GUI. It exits 0 and the game boots alongside. The branch at `0x004D80E6` reads
the menudefinitions path from settings`+0x254` and calls
`GuiDefinition_EnumerateSourceFiles` (`0x00D92810`).

**But it compiles from the RDA, not from loose files.** All 967 `.dlg`/`.tpl`
sources were staged loose under `<install>/addondata/config/gui/menudefinitions/`
and `infobar.dlg` was edited to add elements named `ANNORE_*`. The recompiled
`anno.cdf` contains `INFOBAR` and `HOUSE_FOOD_DEMAND_FULFILLMENT_BAR` but **not**
`ANNORE` — the loose edit was ignored. The undocumented `/rda` switch (string at
`0x010D6BF0`, handled immediately above the `/compilegui` branch) changed nothing.

The first run, before the output directory existed, wrote nothing at all — so
staging mattered only because it created the directory the OUTPUT goes in.

So the authoring loop is real but the input side is not solved: **loose files do
not override the archives**, at least not on this path. That contradicts the
earlier assumption in this file, which was based on loose mousepointers existing
in `data/` — those are probably files absent from the RDA rather than overrides.
Getting a custom `.dlg` compiled most likely means repacking an override archive
(RDAExplorer can write RDA), or finding whatever makes the file layer prefer disk.

## The tag vocabulary

383 distinct tags across the definitions. Structure:

| Group | Tags |
|---|---|
| identity | `PATHID`, `ACCESSID`, `TEMPLATE_REF_ID`, `TEMPLATE` |
| element types | `BUTTON`, `TOGGLEBUTTON`, `LABEL`, `FRAME`, `FRAME_MODAL`, `FRAME_POPUP`, `PROGRESSBAR`, `CIRCULARLYPROGRESSBAR`, `COMBOBOX`, `LISTBOX`, `EDITFIELD`, `SLIDER`, `VIDEOPANEL`, `RENDERTARGET`, `BOXLAYOUT`, `CARDLAYOUT` |
| geometry | `EXTENT` / `LEFT` `TOP` `WIDTH` `HEIGHT`, `ZORDER`, `ALIGNMENT_*`, `FITSIZETOCHILDREN`, `XSPACE`, `YSPACE`, `LINECOLUMNCOUNT`, `LINE_ORDER`, `BOXLAYOUT_BEHAVIOUR`, `RESIZEPARENTWINDOW` |
| appearance | `BACKGROUND`, `TEXTSETTING`, `TEXT`, `TEXTGUID`, `TEXTCOLOR`, `MOUSEOVERTEXTCOLOR`, `DISABLEDTEXTCOLOR`, `FONT`, `ICONGUID`, `ICONSPACE`, `ICONSTRETCH`, `TOOLTIP`, `TOOLTIP_PRESSED`, `TOOLTIP_DISABLED`, `MOUSEPOINTER`, `TRANSPARENT`, `HIDDEN` |
| behaviour | `EVENTHANDLER`, `EVENT`, `IS_DRAGGABLE`, `SOUNDGUIDCLICK`, `START_SEQUENCE`, `EVENT_REACTION_SEQUENCE` |

Everything a 2009 in-house XAML equivalent would have: modal popups, box and
card layouts, alignment, spacing, templates, per-state skins.

Button/slider/listbox skin states are enumerated in `layoutnames.txt`:
`Normal`, `Normal+MouseOver`, `Normal+MouseButtonDown`, `Normal+Disabled`,
`Checked+…`, `Keyboardfocus`, and `Overlay_` variants of each.

## Behaviour *is* data-driven

This is the finding that changes the architecture.

```xml
<BUTTON>
  <EXTENT><LEFT>83</LEFT><TOP>10</TOP><WIDTH>55</WIDTH><HEIGHT>55</HEIGHT></EXTENT>
  <ICONGUID>142519</ICONGUID>
  <ACCESSID>QUICKSAVE_BUTTON</ACCESSID>
  <EVENTHANDLER>
    <EXECUTE_CONSOLE_COMMAND>/quicksave()</EXECUTE_CONSOLE_COMMAND>
    <EVENT>OnLeftClick</EVENT>
  </EVENTHANDLER>
</BUTTON>
```

`<EVENTHANDLER>` appears 2829 times. Inside it, `<EVENT>` names the trigger and a
sibling tag names the action. The action catalogue is large — roughly 200 verbs —
and covers real gameplay, not just UI plumbing:

- UI: `SETVISIBLE`, `SELECT_CARD_IN_CARDLAYOUT`, `SELECT_MENU_TAB`,
  `SET_TOOLTIP_VISIBLE`, `START_SEQUENCE`, `TOGGLE_MENU`, `SHOW_SCREEN_MESSAGE`,
  `PAGELAYOUT_NEXTPAGE`, `SETKEYBOARDFOCUS`, `CLEAR_EDITFIELD`
- game: `SET_PAUSE`, `UPGRADE_RESIDENTHOUSE`, `SET_SHIP_FIGHTMODE`,
  `KILL_SELECTED_OBJECTS`, `NEUTRALBUILDING_PRODUCE`, `ADD_WAREHOUSE_ITEM`,
  `PUT_INTO_MARKETPLACE`, `INITIATE_MINIMAP_SIGNAL`, `SELECT_QUEST`,
  `EXECUTE_ACTIONBAR_ACTION`
- shell: `QUIT_GAME_TO_MAIN_MENU`, `QUIT_GAME_TO_DESKTOP`, `RESTART_LEVEL`,
  `SET_GRAPHICS_OPTION`, `SET_VSYNC`, `SET_SCREEN_MODE`, `SET_VOLUME`
- and the escape hatch: **`EXECUTE_CONSOLE_COMMAND`**

Event names come from `events.map`, which turned out to be **editor help text**
(`EventName#<German description>`), not a binding table — which is why the loader
at `0x00D71770` parses it and throws the result away. It is still the
authoritative list: `OnLeftClick`, `OnRightClick`, `OnMiddleClick`, the
`DoubleClick` / `ButtonDown` / `ButtonUp` families, `OnDrop`, `OnChangeValue`,
`OnRChangeValue`, `OnKeyboardFocusLost`, `OnCreate`, `OnMouseEnter`/`Exit`/`Move`
(plus `IncludingChildren`), `OnMouseWheelUp`/`Down`, `OnGameSelection`.

### The slot idiom C′ would copy

`infobar_island_info.dlg` is the closest existing thing to what we would build,
and it shows the house pattern: **pre-declare numbered slots, let C++ fill them
by `ACCESSID`.**

```
INFOBAR_RESOURCE_PANEL0 .. PANEL9
INFOBAR_RESOURCE_ICON1  .. ICON10
INFOBAR_RESOURCE_AMOUNT1 .. AMOUNT10
```

Each slot carries `<HIDDEN>1</HIDDEN>` and is revealed only when used, and
containers re-layout via `<EVENTHANDLER><FITSIZETOCHILDREN/><EVENT>OnRender</EVENT>`.
This is the same shape as the `"%sWARE_ICONBUTTON%d"` format strings in `.rdata`
(`0x010E3408`): the game composes the name, looks the element up, writes into it.

A custom per-island panel would be the same — `ANNORE_ISLAND_ROW0..15` with
`_NAME` and `_POP` labels — with our code doing what the game's own listener does.

### The console

`EXECUTE_CONSOLE_COMMAND` reaches a real command interpreter (`CConsole`,
`Console.ini`) with a path-like namespace. Commands recovered from the string
table around `0x010FD880`–`0x010FE800`:

```
togglepause          quicksave            quickload         ShowLog
ShowBuildMenu        ShowStrategyMap      ShowDiplomacyMenu ShowResearchMenu
ShowIngameChat       ResetMenus           TogglePostcardView
StartRelocationMode  StartRemoveMode      StartStreetMode   StartPipetteMode
ToggleMilitaryInfoMode  ToggleSubtitles   togglesimplecammode
camToNorth  setcam  Rotate  ChangeVariation
storegroup  selectgroup  addtogroup
openactionarchive  openobjectlist  bind  showbinds  forceasync
ToggleAllCheckValidation
/game/help  /game/discoverall  /game/togglebuildingrules
/game/iamtheking(1)  /game/vitaminb()
```

Note `bind` (key code → console command, with `showbinds`) and `/game/help`.
Parameter types are typed (`int`, `float`, `char*`, `String`), and an unknown
name yields `unknown command` / `Methode nicht vorhanden`. The last two entries
are cheats — `Vitamin B added`, `Cheater!` — so the console is not a read-only
surface. Anything wired to it should be chosen deliberately.

**The string table undersells it.** `addondata/config/game/bindings.ini` is a
plain-text file of `bind("KEY","/command")` lines, and it reveals a namespaced
console with typed arguments that the `.rdata` scan missed entirely:

```ini
bind("B","/game/ShowBuildMenu")
bind("F7","/game/ShowDiplomacyMenu(-1)")
bind("K","/game/wm/togglewarehouses(0)")
bind("SHIFT K","/game/wm/togglewarehouses(1)")
bind("TAB","/game/wm/toggleships(0)")
bind("U","/game/wm/upgradebuilding(0)")
bind("DELETE","/game/wm/killselected")
bind("CONTROL 1","/game/storegroup(1)")
bind("I","/menu/ToggleAllCheckValidation()")
```

Namespaces: root (`/quicksave`, `/quickload`, `/openobjectlist`,
`/togglesimplecammode`), `/game/`, `/game/wm/` (world manager — `togglewarehouses`,
`toggleships`, `toggletroops`, `toggleinfiltratedbuildings`, `killselected`,
`killlastfarmfield`, `upgradebuilding`, `SetCameraToLastEvent`), and `/menu/`.
Run `/game/help` in-game for the authoritative list.

Two consequences for a custom menu. First, `bindings.ini` is itself a loose
data file, so a **hotkey** for a custom menu needs no code either. Second, the
`toggleX(0|1|2)` shape suggests overlay/filter toggles that a menu button could
drive directly.

### Hiding and disabling, from data

`hidden.ini` and `disabled.ini` are flat lists of `ACCESSID`s that the engine
hides or greys out — a data-level kill switch over any element, no code. Live
examples include `MAIN_HIGHSCORE_BUTTON` and `05MAIN_CREDITS_BUTTON`.

## Where data-only stops

## Widgets can be created at runtime, from code

The data route is blocked on getting sources past the archives. It turns out not
to matter, because the engine has a **runtime widget factory** and one call does
the whole job.

`GuiRuntime_CreateElement` (`0x00DB72C0`):

```c
int GuiRuntime_CreateElement(int eType, stlp_string *parentName, stlp_string *newPathId);
// 0 = ok, 0xC = parent not found, 3 = unknown type
```

It resolves the parent by name, builds a descriptor, allocates, constructs the
right class, calls `parent->vtable[0xA4](child, 0, 0)` to attach it, and triggers
a relayout. Sizes and constructors per type:

| Type | Enum | Bytes | Ctor |
|---|---|---|---|
| LABEL | 1 | `0x924` | `0x00D7CC30` |
| BUTTON | 2 | `0xAAC` | `0x00D3F450` |
| SLIDER | 3 | `0x7BC` | `0x00DA2710` |
| EDITFIELD | 5 | `0x8A8` | `0x00D58770` |
| BOXLAYOUT | 6 | `0x704` | `0x00D8D900` |
| COMBOBOX | 7 | `0xBF0` | `0x00DA5DA0` |
| LISTBOX | 8 | `0x84C` | `0x00DABD40` |
| PROGRESSBAR | 9 | `0x6FC` | `0x00DB9940` |
| CARDLAYOUT | 10 | `0x738` | `0x00DBA140` |
| RENDERTARGET | 12 | `0x920` | `0x00D7DD50` |
| ALPHAPROGRESSBAR | 13 | `0x714` | `0x00DB9180` |
| VIDEOPANEL | 15 | `0x938` | `0x00D83240` |
| STRUCTTEMPLATEINSTANCE | 20 | `0x6F8` | `0x00DBA8B0` |

### The registry

`GuiRegistry_Get()` (`0x00418FE0`) returns a lazily-initialised singleton at
**`0x012D8F78`** holding two name maps:

- **`+0x1AC`** — keyed by `ACCESSID`. Every element with a non-empty ACCESSID
  **self-registers here at construction**, from `GuiWnd_Construct` (`0x00D6D8D0`)
  via `GuiRegistry_RegisterByAccessId` (`0x00D6D630`). That function also files
  the element into per-type maps and pops a *"Duplicate access ID: %s"* message
  box on a clash — useful as a sanity check that our code ran.
- **`+0x254`** — what `GuiRegistry_FindElementByName` (`0x00D51140`) searches and
  what the factory uses to resolve a parent. **UNVERIFIED** which key it uses.

`GuiRegistry_FindElementByName(registry, name)` returns the element pointer or 0,
which makes the event-dispatcher hook unnecessary for *finding* elements —
`tools/production-menu` currently does it the hard way and can be simplified.

### What this means

A custom native panel needs no `.dlg`, no `/compilegui` and no RDA repacking:
create a `BOXLAYOUT` or `LABEL` under an existing frame, add a `PROGRESSBAR`
child, and drive its value at `+0x6F0` as annobridge already does.

Two details to settle first. A runtime-created element gets a PATHID but **no
ACCESSID**, so it will not be in the by-name map — either write `+0x109` and
register it manually, or keep the pointer by walking the parent child list (heads
at parent`+0x78`/`+0x7C`). And for a first test the `INFOBAR` frame is convenient
because its PATHID and ACCESSID are the same string, so the `+0x254` key question
does not block it.

There is a declarative binding layer, and it is nearly what we would want.

`guimodelconditionparser.grm` is a plain-text GOLD Parser grammar named
`GUIModelConditions` — *"Parsing bool expressions for gui model conditions"*.
It defines a real expression language:

```
<Condition>  ::= <Expression> | <Expression> '||' <Expression>
               | <Expression> '&&' <Expression> | '!' <Expression>
               | 'ModelEvent/' ModelEvent
<Comparison> ::= <Identifier> ('<'|'>'|'<='|'>='|'=='|'!=') <Constant Number>
               | <Identifier> ('=='|'!=') String  | <Identifier> (…) <Identifier>
<Identifier> ::= 'Model/' Integer | <ElementVar>
<ElementVar> ::= SubElement AttributeName | AttributeName
```

And `models/*.xml` declares the binding itself — a named model bound to a dialog
by `ElementAccessID`, with each attribute routed to a sub-element's category and
attribute:

```xml
<Model>
  <Name>credits</Name>
  <ElementAccessID>MAIN_MENU_CREDITS_DIALOGUE</ElementAccessID>
  <Attributes>
    <Attribute>
      <Type>2</Type><Name>titleGUID</Name><SubElement>TITLE</SubElement>
      <ElementCategory>TextSetting</ElementCategory>
      <ElementAttribute>TextGUID</ElementAttribute>
    </Attribute>
    ...
```

So an element's text/icon/visibility can be driven declaratively from a model.
**But the model is populated by C++.** Only one model ships in the addon RDA
(`credits`), and no menu definition references `Model/` or `ModelEvent` at all —
this looks like a late, barely-used feature.

That is the boundary. We can declare a menu, lay it out, skin it natively, make
its buttons act — and we cannot make a label read a number that `annomem.py`
computed, because nothing in the data path reaches our process.

### Dynamic numbers are the easy case

If the value to show is a **number**, none of the text machinery is involved.
Two widgets hold their value as a plain `long` on the element:

| Widget | Type enum (`+0xC4`) | Value | Range | Renders |
|---|---|---|---|---|
| `PROGRESSBAR` | 9 | `+0x6F0` | `+0x6F4` MAXVALUE | a bar |
| `SLIDER` | 3 | `+0x6F8` | `+0x6FC` RESOLUTION | a bar, **and digits** with the `SLIDER_PERCENTAGE_TEXT` flag (`+0x2A0` bit 1) |

No localisation GUID, no text engine, no strings, no STLport marshalling. An
injected DLL writes one dword and the widget updates.

That makes the full loop for a native readout about as small as it can get:

1. Declare the panel and widget in a `.dlg` with your own `ACCESSID`; wire a
   button to it with `<SETVISIBLE>MY_PANEL; toggle</SETVISIBLE>`.
2. Hook `GuiElement_DispatchNamedEvent` (`0x00D6C0D0`), match
   `(char*)(elem+0x109)` against your `ACCESSID`, cache the pointer.
3. On `OnRender`, write your value to `+0x6F0` (bar) or `+0x6F8` (slider).

Two things to check before relying on the slider. Its digit formatting is
**UNVERIFIED** — presumably `CURRENTVALUE / RESOLUTION` as a percentage, in which
case `RESOLUTION = 100` would display the raw value. And a slider is
*interactive*, so input should be disabled if it is meant as a readout.

The game's own numeric labels (`INFOBAR_RESOURCE_AMOUNT1..10`) take a third
route: their definitions carry an **empty `<TEXTSETTING/>`**, so C++ fills the
live TextSetting at label`+0x8C8` entirely at runtime. That live struct may carry
runtime-only fields the definition form does not — unexplored, and the obvious
place to look if a real numeric *label* is wanted rather than a bar.

### The text path: templates and substitution tags

A label holds **no character buffer**. Its text is a `TEXTGUID` at `+0x734`
(live copy at `+0x8C8`), and all 1377 `<TEXT>` values across the definitions are
numeric — zero literals. So the data layer genuinely cannot express a string.

But a localisation entry is not a fixed string either. It is a **template**, and
`CTextEngine` / `CTextTagProcessor` substitute `[BRACKETED]` tags from context at
render time. From `addondata/config/texteditor/*.xml`:

```xml
<GUID>40111200</GUID>  "You won this game in: [CURRENT_GAME_PLAY_TIME]"
<GUID>40100128</GUID>  "(Error code: [CONTEXT_STRING])"
<GUID>40100204</GUID>  "Winning conditions ([CONTEXT_INTEGER])"
```

89 distinct tags exist. Most are specific — `CURRENT_CITY_NAME`,
`SEL_WAREHOUSE_CITY_NAME`, `SEL_CENTER_BEGGARCOUNT`, `SEL_HANDLE_STORAGE`,
`SEL_HANDLE_WAREHOUSE_CAPACITY`, `TOWN_VALUE`, `OBJECT_HITPOINTS`,
`BUILDING_COSTS`, `CURRENT_GAME_PLAY_TIME` — but **two are generic,
caller-supplied slots**: `[CONTEXT_STRING]` and `[CONTEXT_INTEGER]`. Tags also
take arguments (`[COLOR 0xAARRGGBB]` … `[DEFAULTCOLOR]`, `[CR]` for a line break).

The tag names live in a `char *[]` table around `0x0127D884`, the same shape as
the ware and population-level enums this project already uses.

This changes the picture twice over:

- **A dynamic value with zero code is possible today.** Point a custom label's
  `<TEXTGUID>` at an existing GUID that already carries a tag — `40111200`
  renders a live clock. No DLL, no new loca entry.
- **Arbitrary computed text has a designed route**: `[CONTEXT_STRING]`. What
  supplies the context is not yet known, and that is now the precise open
  question rather than "is there a text path at all".

Two caveats, both UNVERIFIED. Whether a tag resolves for a label rendered outside
the flow it was written for is untested — context-sensitive tags like
`SEL_HANDLE_*` plainly depend on a selection. And the `texteditor/*.xml` files
look like sources compiled into `data/loca/<lang>` (there is a
`txt/localisation.xml`), so **adding new GUIDs** may need a build step the way
`.dlg` files need `/compilegui`; **reusing existing GUIDs** needs nothing.

### The trigger engine knows the numbers but cannot print them

Worth ruling out explicitly, because it looks like a way around the boundary and
is not. `data/config/ablauf/` (base tree) documents the quest/trigger DSL — 165
actions and 86 conditions as `ID=NAME=description with <TYPED=parameters>`. It
reads exactly the quantities this project cares about:

```
71=RESIDENTS: RESIDENT COUNT=<PLAYER> <MINDESTENS|HOECHSTENS|GENAU>
   <COUNT=Einwohneranzahl,0,1000000> Einwohner der Stufe
   <COMBOCHECKED=Level,ResidentLevel> auf <CONTAINER=Inselcontainer,ISLANDS>
62=RESOURCE: GOODS IN WAREHOUSE=<PLAYER> auf <CONTAINER=ISLANDS> ... <COUNT>t <Product>
75=RESIDENTS: RESIDENT MORAL PER LEVEL ON ISLAND=...
```

Per-player, per-tier, **per-island** resident count, as a first-class condition.
It also has variables with arithmetic (`200 SET VARIABLE`, `1000 CALC VARIABLES`,
`1142 SET TO GUID COUNT`, `1162 SET TO OBJECT VALUE`).

But the output side does not close the loop:

- `1030 GUI: DISPLAY TEXT` takes a static `<CALL=TextID>` — a localisation
  reference, not a variable. There is no "print variable" action.
- The two actions that touch GUI elements, `1451 HIGHLIGHT BUTTON` and
  `1452 ENABLE ELEMENT`, address a **hardcoded enum** (`BUILDMENU_BUTTON`,
  `TEAR_DOWN_BUTTON`, `STRATEGY_MAP_BUTTON`, …), not an arbitrary `ACCESSID`.

So triggers can *branch* on per-island population but cannot display it, and
cannot drive a custom element. They also live in level files, so anything built
this way would be per-map rather than global. The boundary holds.

## Consequences for the companion tool

The read-only rule survives. Definition files are **content, not code**: writing
a `.dlg` and running `/compilegui` does not open a handle on the game, does not
inject, and cannot desync a lockstep session. `AGENTS.md` rule one is untouched
by anything in the data path.

Routes, revised:

| | Approach | Injection | Looks native | Can show our numbers |
|---|---|---|---|---|
| A | Separate window | no | no | yes |
| A′ | Borderless always-on-top window over the client rect | no | partly | yes |
| **D** | **Custom `.dlg` + `/compilegui`; buttons wired to console commands** | **no** | **yes** | **no** |
| B | Inject, hook D3D9 `Present`, draw ImGui | yes | no | yes |
| C′ | Custom `.dlg` for layout; injected code finds elements by `ACCESSID` and fills labels | yes | yes | yes |

**D + A is the honest recommendation**: an in-game menu for anything the game
already knows how to do, and the external window for our own aggregates. It
costs no charter change and no crash risk.

C′ remains the only route to *native display of our own data*, and D is not
wasted work if you go there later — the same `.dlg` becomes C′'s layout, and the
injected side only has to find elements by name and set text.

## Calling into the process, if it comes to C′

- No ASLR, image base `0x400000` — Ghidra addresses valid verbatim, no signature
  scanning.
- RTTI for 2363 classes; `annomem.py scan` already finds objects by vtable.
- GOG build: no DRM, no anti-cheat.
- `d3d9.dll` / `d3d10.dll` / `dxgi.dll` loaded by name at runtime, so a render
  hook means device-vtable hooking, not IAT patching.
- The game links **STLport**; these APIs take STLport `std::string` by pointer.
  Build them with the game's own `StlpString_CtorFromCStr` (`0x00B09160`) and
  `StlpString_Dtor` (`0x00B088A0`).
- `GuiElement_DispatchNamedEvent` (`0x00D6C0D0`) is one function that sees every
  event in the game, with the element and the event name in hand — the natural
  single hook. Handlers live in a per-element map at element `+0xC0`, walked as
  `0x54`-byte records.

## Which archive holds what

The GUI ships twice — once for the base game, once for the addon — and the
addon binary (`Addon.exe`) uses the `addondata/` copy. Getting this wrong wastes
a lot of time, so:

| Archive | Files | Holds |
|---|---|---|
| `addon/Data1.rda` | 4483 | **`addondata/config/gui/**`** — the addon GUI, i.e. the one `Addon.exe` loads. Plus `addondata/config/game/{assets,properties,icons,datasets,templates}.xml` and `bindings.ini`. |
| `maindata/Data1.rda` | 2746 | the base game's `data/config/**`, including `data/config/gui/` (`Anno4.exe`'s UI) and `data/config/ablauf/` (the trigger DSL catalogue) |
| `maindata/patch1.rda` | 1111 | ships a newer `data/config/gui/anno.cdf` + `backgrounds.bin` (Jan 2010) |
| `maindata/Data0.rda` | 261 | AI scripts in **CSL**, the engine's own scripting language (`CCSLExecutor`, `CCSLModule`, `CCSLFunction`) |
| `maindata/Data2.rda` | 12651 | `data/graphics/ui/` — icons, symbols, textures, mousepointers |
| `maindata/Data5.rda` | 139 | shaders, including `gui_font_p4c1t2t2.fx`, `gui_icon_p4c1t2t2.fx`, `gui_line.fx` |

**Patches override.** `patch0/1/2.rda` (4110 / 1111 / 450 files) are layered over
the base archives, and `patch1` carries a newer base `anno.cdf`. Before treating
any extracted file as current, check whether a patch supersedes it.

### Base tree vs addon tree

Both are extracted now. For `Addon.exe` the answer is simple: **the addon tree is
a strict superset and the base tree adds no GUI capability.**

| | base `data/config/gui/` | addon `addondata/config/gui/` |
|---|---|---|
| menudefinitions | 817 (72 `.dlg`, 744 `.tpl`) | 967 (85 `.dlg`, 881 `.tpl`) |
| `.dlg` names not in the other | **none** | 13 |
| `models/` + `guimodelconditionparser.*` | absent | present |
| `iconfilemap.xml` | present | **absent** |

The 13 addon-only dialogs are `statistics_menu`, `ingame_chat`, `internet_chat`,
`network_info`, `network_blocking_info`, `objectmenu_bailiwick`,
`objectmenu_csp_warehouse_land`, `objectmenu_espionage_building`,
`popup_info_large`, and four online-account dialogs.

Two details that matter for authoring:

- **The model/condition binding layer is addon-only.** `models/` and the
  `GUIModelConditions` grammar exist only under `addondata/`, which supports
  reading it as a late Venice addition rather than a mature feature.
- **`Addon.exe` reads both trees.** `iconfilemap.xml` lives only in the base tree,
  and the binary references it as `data/config/gui/IconFileMap.xml` (`0x010A64E4`)
  — with the `data/` prefix, not `addondata/`. It maps icon file IDs to PNG paths
  and dimensions, so anything using `<ICONGUID>` resolves through the base tree.

Archive contents can be listed without decrypting: walk the block chain from the
`uint32` at offset `0x414`, each block header being 20 bytes
`(flags, fileCount, dirSizeCompressed, dirSizeUncompressed, nextBlockOffset)`
sitting *after* its data, with 540-byte directory records (520-byte UTF-16 name
+ 5 dwords). Blocks with `flags & 3 == 0` are neither compressed nor encrypted,
so their names read straight out — which is how the `patch1` GUI entries above
were found without running an extractor.

## Settlement selector design (research, 2026-09-30)

A larger production dialog with a left-side settlement selector is feasible, but a
native `LISTBOX` is not yet the best integration point:

- `LISTBOX` is a real native type (enum 8, `0x84C` bytes, constructor
  `0x00DABD40`) and supports `OnChangeValue`, selected/hover text colours, and
  scrolling. The shipped XML uses it with game-specific listener actions such as
  `SET_PLAYER_PROFILE`; `FILLLISTBOX` is a generic action verb, not an observed
  way to provide arbitrary external settlement strings. No declarative
  multi-select tag was found in the extracted GUI definitions.
- The safest first implementation is a fixed, scrollable set of numbered rows
  (`BUTTON` or `LABEL` children) that the companion fills with the 19 island
  names and IDs. The current memory reader already enumerates islands and names.
  Hidden/unused rows can be cleared each update, as with the production rows.
- The injected bridge must report row clicks to the external companion. It can
  match `OnLeftClick` on `ANNORE_ISLAND_ROW##`, sample `GetAsyncKeyState(VK_CONTROL)`
  on the GUI thread, and publish `{row, ctrl, sequence}` through the existing
  shared-memory bridge. The companion then owns a selected-island set: normal
  click replaces it, Ctrl-click toggles/adds it, and the hovered island is the
  default selection only while no manual selection is active.
- The dialog renderer can show one selected island at a time initially, or an
  aggregate across the selected set later. Aggregation needs explicit semantics
  for goods, population, duplicate production buildings, and unavailable data.

Required work is therefore: enlarge/re-layout the frame; add a left selector and
scrolling; extend the bridge event payload; add companion selection state and
island-name updates; then decide whether multi-island output is aggregate or
selected-island tabs. Native `LISTBOX` remains worth a prototype, but its data
population and Ctrl-selection semantics require further Ghidra/live validation.

## Open questions

- **Does `Addon.exe /compilegui` work on the retail build, and what does it write?**
  Everything in route D depends on this one answer.
- Does a loose file under `<install>/addondata/config/gui/` override the addon
  RDA, as `data/graphics/ui/mousepointer` does for `maindata`? If `/compilegui`
  only rewrites `anno.cdf`, this decides how the result gets loaded.
- `/game/help` should dump the full console command catalogue at runtime — the
  string-table list above may be incomplete.
- What populates a model, and is any live game value (population, stock) already
  exposed as one? A cheap long shot that would collapse the whole problem.
- Is `CFileChangeManager` wired to GUI definitions (hot reload)?
- `anno.cdf` binary format — only needed if `/compilegui` turns out not to work.
- Record layout of the `0x54`-byte handler entries at element `+0xC0` (only
  needed for C′).
- Element type enum values 0, 4, 11, 14, 16–19 are absent from the switch at
  `0x00DB8B20` although more types exist — where are they handled?

## Addresses

| Address | Name | Confidence |
|---|---|---|
| `0x004D80E6` | `/compilegui` branch in `ParseParam` | confirmed from disassembly; not yet run |
| `0x00D92810` | `GuiDefinition_EnumerateSourceFiles` (globs `*.dlg`/`*.tpl`) | confirmed |
| `0x00D6C0D0` | `GuiElement_DispatchNamedEvent` | confirmed by decompilation, not observed live |
| `0x00DB8B20` | `GuiDefinition_BeginElementNodeForType` (type enum → token) | confirmed |
| `0x00DB8600` | `GuiDefinition_BeginElementNode` (`PATHID`) | confirmed, cross-checked against extracted XML |
| `0x00D71770` | `GuiEventMap_LoadFromFile` | confirmed; `events.map` is editor help text |
| `0x004555E0` | worked example: fires `OnMouseEnter` | confirmed |
| `0x00B09160` / `0x00B088A0` | `StlpString_CtorFromCStr` / `StlpString_Dtor` | high, inferred from call shape |
| `0x010A6304` | `g_GuiElementTypeTokens` | confirmed by raw read |
| `0x010CD8F8` | default menudefinitions path → settings `+0x254` (set at `0x0048E210`, consumed at `0x004D8108`) | confirmed |
| `0x012D334C` | `g_pGuiManager` — used by both the compiler branch and the event dispatcher | confirmed as shared; class unidentified |
| `0x010FD880`–`0x010FE800` | console command name table | confirmed by extraction |

Relevant classes (RTTI): `CDynamicGUIElementManager`, `CDispatcherMenuManager`,
`CALGUI`, `CConsole`, `CConsoleTemplate`, `CDragAndDropManager`, `CBlinkManager`,
`CBackgroundLayoutManager`, `CFileDatabase`, `CFileDirect`, `CFileChangeManager`.

Extracted sources: `data/addon/data1.rda/addondata/config/gui/` (from
`addon/Data1.rda`, the tree `Addon.exe` loads) and
`data/maindata/data1.rda/data/config/` (from `maindata/Data1.rda`, the base game).
