# Production, goods and needs

What the game itself knows about production and consumption, and how to read it.
Struct layouts live in Ghidra; this file records the *derivation* and the
things Ghidra cannot say — what was checked against the running game, what is
still missing, and what a consumer should not assume.

The three questions this answers, up front:

| Question | Answer |
|---|---|
| Does the game know per-settlement demand, or is consumption static? | It simulates. Every house carries a 13-entry demand quote; every settlement keeps a 68-good demand history. |
| Can we list the production buildings of a settlement? | Yes, with type name, position, product and rate. |
| Productivity, paused state, input goods? | All three are directly readable, and pause is a separate flag from productivity. |

## The lever: reflection functions

Anno 1404 serialises save games through a reflection layer that registers each
field **by name, as a literal string**:

```c
FUN_004135b0(ar, "m_ProductionTime", (int *)(this + 8),  0);
FUN_00b0fbe0(ar, "m_Productivity",   (int *)(this + 0x10), 100);
FUN_00b0fc60(ar, "m_ProductionStopped", (char *)(this + 0x14), '\0');
```

One of these functions per class, each an authoritative field map with offsets,
types and defaults. This is far stronger than inferring layout from `memset`
sizes, and it is how essentially everything below was found. Search strings for
`^m_` to locate a class, then take the single xref.

They also reveal array *lengths* for free: a reflect helper that loops over a
name table emits one named record per entry, so `m_DemandsPerLevel` is provably
11 entries and `m_demandHistory` provably 68.

The reflect function sits at **vtable + 0x34** on the property classes.

## The three enums

A single region of the image holds the name tables. Absolute addresses, since
there is no ASLR:

- `0x0127B820` — `char *[68]`, **the ware enum**. `8 SaltFish`, `9 Bread`,
  `15 Beer`, `35 Flour`… Entries 6 (`BuildMaterialCount`) and 28
  (`FinalGoodsCount`) are range sentinels, not goods.
- `0x0127B930` — `char *[11]`, **the population levels**, and it names the four
  rows this project had listed as unknown:
  `0 Outlaw, 1 Beggar_on_street, 2 Beggar_in_hospice, 3 Ruin_of_ambassador,
  4 Nomad, 5 Ambassador, 6 Ruin_of_peasant, 7 Peasant, 8 Citizen, 9 Patrician,
  10 Nobleman`. The seven tiers previously identified by correlation land
  exactly on the matching names — an independent confirmation of the
  population array.
- `0x0127B060` — `char *[13]`, the **need categories**: Food, Drink, Clothes,
  Property, OrientDrink, OrientProperty, Company, Faith, Amusement, Security,
  OrientFaith, OrientAmusement (plus `NoCategory`).

## The on-disk source of all this

Everything below was recovered from memory and from the reflect functions. Since
then `addon/Data1.rda` has been extracted, and the object model also exists as
XML at `data/addon/data1.rda/addondata/config/game/`:

| File | Size | What |
|---|---|---|
| `assets.xml` | 1.7 MB | 980 assets over 71 templates (`FactoryBuilding`, `FarmBuilding`, `Resident`, `TradingShip`, `SimpleUnit`, …), each with its `<Values>` block — `BuildCost`, `Blocking`, and so on |
| `properties.xml` | 1.5 MB | the property definitions behind the `CPropertyXxx` classes |
| `templates.xml` | 82 KB | the templates assets instantiate |
| `datasets.xml` | 311 KB | — |
| `icons.xml` | 405 KB | icon GUIDs, as referenced by `<ICONGUID>` in the GUI definitions |

**This is the addon's set only.** `FishermanHut` is absent, because base-game
objects live in the base `data/config/game/` inside `maindata/Data1.rda`, which
is not extracted yet. The memory work found 1126 object types; this file holds
980, so the two sets are complementary, not redundant.

**UNVERIFIED as a source of truth.** Nothing here has been cross-checked against
the live values, and the standing caution still applies — XML is the design-time
nameplate, and `--measure` remains the authority on what actually happens. Treat
it as a naming and structure reference until someone reconciles the two.

## Object model

Buildings are **entity-component**. A game object holds a fixed set of property
slots, each a `CPropertyXxx` instance, plus one pointer to the shared static
data for its type:

```
game object ──┬── +0x4C  CPropertyResidenceBuilding*   (houses)
              ├── +0x54  CPropertyFarm / CPropertyFactory / …  (producers)
              ├── +0x5C  maintenance cost
              ├── +0xC0  player slot
              ├── +0xC4  CPropertyDataContainer*   ← static, shared per type
              └── +0xD4  CIsland*
```

`CPropertyDataContainer` is 0x12C bytes of **fixed slots**, one per property
type — slot offset alone identifies the class. 1126 of them are resident in
memory, one per object type in the RDA data, whether or not the player has ever
built one.

```
container ──┬── +0x08  CPropertyDataFactory         input goods
            ├── +0x20  CPropertyDataResidenceBuilding
            ├── +0x34  CPropertyDataWareProduction  output good + rate
            └── +0x74  CPropertyDataStandard        type name + GUID
```

`CPropertyDataStandard + 0x08` is an **inline** NUL-terminated `char[]` holding
the internal type name — `FishermanHut`, `PeasantHouse`, `CiderFarm` — with the
GUID at `+0x28`. (Inline again. See the standing note in `methodology.md`.)

## Recipes

`CPropertyDataWareProduction`: `+0x08` cycle time in ms, `+0x0C` product ware
id, `+0x10` output capacity, `+0x14` amount per cycle (1000 == 1 t).
`CPropertyDataFactory`: `+0x08/+0x0C/+0x10` and `+0x14/+0x18/+0x1C` — ware id,
capacity, amount per cycle, for up to two inputs.

The field assignment is not a layout guess. Decoded across the live containers
it reproduces Anno 1404's known chains:

```
Flour            -> Bread      Herbs + Wheat  -> Beer
IronOre + Coal   -> Iron       Clay + Quartz  -> Mosaic
SugarCane        -> Sugar      Wood           -> Paper
```

A building with no inputs has a **null** `CPropertyDataFactory` slot, so "does
this building consume goods" is answerable without decoding anything.

## Runtime production state

`CPropertyProduction` is the base; `CPropertyFactory` and `CPropertyFarm`
extend it.

```
+0x04  owner building        +0x14  m_ProductionStopped  (byte)
+0x08  m_ProductionTime      +0x18  m_ReservedGoods
+0x0C  m_Goods  (held)       +0x1D  m_IsProducing        (byte)
+0x10  m_Productivity (%)    +0x24  m_Capacity
+0x28  m_RawGoods1, +0x2C m_RawGoods2   (CPropertyFactory only)
```

**`m_Productivity` and `m_ProductionStopped` are independent.** A paused
building keeps whatever productivity it had, so reading productivity alone will
show a paused building as fully productive. Both must be read. The UI's own
getter (`CPropertyProductionGetDisplayedProductivity`, `0x0079E300`) returns
`+0x10` but forces 0 while the building is still under construction.

Effective output:

```
t/min = (nAmountPerCycle / 1000) * (60000 / nCycleMs) * (m_Productivity / 100)
        and 0 if m_ProductionStopped
```

## Settlements: the Kontor

```
CIsland + 0x3F78 + playerSlot*4  ->  CWarehouse*
```

No scanning. The array is pre-populated for every slot, including third-party
traders at slots 8..18; `m_storage <= 999` means that player has no Kontor
there. On the test save, 8 of 570 live `CWarehouse` instances passed that test
and they were exactly the settled island/player pairs.

```
+0x33C  m_storage           warehouse capacity
+0x35C  m_productsMap       std::map<wareId,int>  ← THE STOCK, 1000 == 1 t
+0x3B0  m_demandHistory     68 lists, 40 rolling samples each
+0x768  m_DemandsPerLevel   11 lists
+0x7C0  m_PlayerID
```

`m_DemandsPerLevel` is a trap: despite the name it is **build material**
demand, not food. Walked live it gives Peasant → {Wood, Tools}, Citizen →
{+Stone}, Patrician → {+Glass} — the house upgrade cost table.

`m_demandHistory[ware]` *is* the per-settlement demand record. Only goods the
settlement actually demands have non-zero lists. The samples read 100 on three
settlements with 32, 64 and 188 peasants, so the value is a **coverage
percentage**, not an absolute amount — worth stating, because reading it as
tons would look plausible and be wrong.

## Needs are simulated

Every inhabited house carries `CDemandQuote[13]` at
`CPropertyResidenceBuilding + 0x18`, one per need category, each 0x1C bytes:

```
categoryID, critical, downgradeQuote, minQuote, upgradeQuote, quote, fulfillment
```

So the game tracks, per house, current satisfaction per category plus the
thresholds at which that house upgrades or downgrades. `13 * 0x1C == 0x16C`
lands exactly on `m_MoraleBuildingDemandCap` at `+0x184`, which is why the
array length is asserted rather than assumed.

This settles the question behind the community calculator: its
`Peasants / 200` style ratios are an *external reconstruction* of something the
game computes properly. A companion tool does not have to reproduce the
formula — it can read the result.

## Consumption, measured

The static rate constants were never located (see below), but they turned out
not to be needed: consumption is directly observable, and cleanly.

**Goods are deducted from `m_productsMap` on a strict 4.00 s tick**, as a single
negative step per good. Producer deliveries arrive as positive jumps at
unrelated moments, so sampling at 0.25 s separates the two perfectly — no need
to pause anything or to isolate a settlement:

```
Second Island   32 peasants   SaltFish  -21 kg/tick   gaps 4.00 s
First Island    64 peasants   SaltFish  -42 kg/tick   gaps 4.00 s
Odesville      188 peasants   SaltFish -125 kg/tick   gaps 4.00 s
Odesville      188 peasants   Cider     -55 kg/tick   gaps 4.00 s
```

Exactly one step size per good per settlement, and the step scales linearly
with population. The per-head rate is truncated to whole kg at the settlement
level, which is why 32 → 21 rather than 21.33.

Solving back:

| good | kg per head per tick | t/min per head | = one building per |
|---|---|---|---|
| SaltFish (peasant) | 2/3 | 0.01000 | 200 peasants |
| Cider (peasant) | 0.29412 | 0.004412 | 340 peasants |

`FishermanHut` produces 1000 per 30 000 ms = 2 t/min, so 200 peasants consume
exactly one hut. `CiderFarm` produces 1000 per 40 000 ms = 1.5 t/min, so 340
peasants consume exactly one farm.

**Those are the community calculator's divisors, to the digit** — its
`Peasants / 200` for fish and `Peasants / 340` for Most. So the calculator's
table is not an approximation; it is the real balancing, expressed as
"inhabitants per building". Two of its rows are now empirically confirmed, which
raises confidence in the rest considerably — though only those two were tested,
because this save has nothing but peasants.

The general form:

```
consumed(good) t/min = SUM over tiers  population[tier] / divisor[tier][good]
                                       * output_rate(canonical building)
```

Caveat worth keeping: the 4.00 s tick was measured in wall-clock seconds at the
game speed then in use. Whether it is real time or scales with the speed setting
was not tested.

### What actually drives the tick

"Every 4 s" was only an observation. The mechanism behind it:

**The choke point.** `CWarehouseChangeGoods` (`0x007C8170`) is the *only* way a
settlement's stock changes — production deliveries, build costs, trade and
consumption all funnel through it. Signature
`(this, wareId, delta, bNotify, questId)`; 37 call sites.

```
wareId <= 0x43  -> the stock map at +0x35C
wareId >  0x43  -> the quest-item vector at +0x3A4/+0x3A8   (different path)
```

Two behaviours a consumer must know: a result `< 1` **erases the map entry**
rather than storing zero (so "absent" means zero), and the result is clamped by
`CWarehouseGetGoodCapacity` (`0x007C8780`) = `m_storage + extensions × 1000`.
That last one explains the pinned stocks: three settlements all reported
`m_storage == 20999` while stock pinned at 20.999 t, 40.000 t and 40.999 t, and
`40999 == 20999 + 20×1000` fits exactly.

**The beat.** `CWarehouse + 0x228` is `m_TendencyTick`, `int[68]`, a per-ware
timestamp on a **100 ms game clock**. It advances in steps of *exactly 40* —
never a partial step; sampled +80 across 9 s, i.e. two ticks. 40 × 100 ms =
4.00 s. The clock itself was located live as a counter advancing ~10/s inside a
`CEventManager` instance (field +0x80); its static address is not resolved.

**Per settlement, not global.** Odesville ran a consistent 2 s offset from the
two human settlements, so each warehouse has its own phase.

**Central, not per house or per market.** Odesville has 26 residences, 188
peasants, and 4 market buildings, and the step was exactly
`floor(188 × 2/3) = 125`. Per-house truncation would give ~104; per-market would
show several smaller steps at different phases. One event, one step size.

**Only the local player is instrumented.** `ChangeGoods` guards the flow
accumulator (`+0x08`, `int64[68]`, `delta << 12` — the source of the Kontor's
trend arrows) and `m_TendencyTick` on *this warehouse belongs to the local
player*. AI warehouses read all zeros there while consuming normally. Worth
knowing before building a UI on those fields for opponents.

### The tick, solved

Live debugging settled it. First an execute breakpoint on `ChangeGoods` for 45 s
caught only **positive** deltas — so consumption never goes through it. Then a
hardware **data write-breakpoint** on a live `m_productsMap` node caught the
store directly, 11 of 12 hits at `0x007C8B02`, with `ESI` = ware id, `EDI` = old
value, `ECX` = new value:

```
Odesville  SaltFish 32943 -> 32818   (-125, 188 peasants)
Odesville  Cider    33140 -> 33085   ( -55, 188 peasants)
First Is.  SaltFish 40579 -> 40537   ( -42,  64 peasants)
Second Is. SaltFish 40012 -> 39991   ( -21,  32 peasants)
```

Exactly the steps measured from the outside. The tick is **two-phase**:

```
CWarehouseAccumulateDemand (0x007C8940)   map38C[ware] += amount   ← during the interval
CWarehouseApplyAccumulatedDemand (0x007C89F0)                      ← every 4 s
    for each entry: CWarehouseConsumeGood(ware, amount)
CWarehouseConsumeGood (0x007C8A30)
    stock >= amount -> map[ware] = stock - amount     coverage = 100
    stock <  amount -> ERASE the entry                coverage = stock*100/amount
    push coverage into m_demandHistory[ware]; subtract (amount << 12) from +0x08
```

That last line **confirms** the history samples are a coverage percentage, and
the ring length is not hardcoded — it's `GetEconomyBalancingBlock()[+0x0C]`,
which reads 40 live, matching the 40 samples per ware.

The two-phase design is also why truncation lands at settlement level: demand is
summed into one number before it is applied.

**The interval is a data constant.** `GetEconomyBalancingBlock()` returns
`DAT_012b3340 + 0x29E4`, and `[+0x00]` is the tick in milliseconds, default
4000. Patching it to 2000 in the live process halved the deduction period to
exactly 2.00 s on all three settlements at once; restoring 4000 returned it to
4.01 s. Proven, not inferred.

### It is script-driven, and that is why the rates were never found

Both tick entry points have exactly **one** caller: `DispatchScriptApiByNameHash`
(`0x0067B2A0`). Every captured stack carried a frame at `0x0067F3AB` inside it.

I had seen that single-caller pattern earlier and explicitly dismissed it —
"scriptable ≠ script-driven" — which was the right *caution* but the wrong
*conclusion*. The runtime stack settles it: the economy tick genuinely runs from
the game's CSL script layer.

This is the reason the per-good consumption rates are absent from every
`CPropertyData*` slot, from `CPropertyDataBalancing`, and from the residence
data. **They are not in the C++ object model at all.** The engine exposes only
add-demand and apply-demand primitives; the script decides how much and how
often. Script API hash `0xa469fa` computes the amount as
`(x * y * (balancing/1000)) / 6000` under guards `level < 0xB` and `ware < 0x44`
— per population level, per ware. An earlier pass already found 288 CSL scripts
in `maindata/Data0.rda`; that is where to look next.

`DispatchScriptApiByNameHash` is a ~5200-line switch on name hashes and costs
~250 KB to decompile — use its callers, don't read it.

### Nominal is not actual

Productivity does **not** tell you throughput. A `FishermanHut` at 100 %
productivity, unpaused, whose warehouse is at its cap delivers *nothing* — its
`m_Goods` pins at 5 t and the warehouse only drains:

```
  good            nominal  delivered  consumed      net   (t/min)
  SaltFish          2.000      0.000     0.335   -0.335
```

`tools/research/annoprod.py --measure` reports all three columns for exactly this
reason: `nominal` from the static rate and current productivity, `delivered`
and `consumed` from the observed positive and negative steps. A production
screen that shows only nominal output would call this settlement healthy while
its fish stock falls.

### Where the constants are not

Recorded so the negative stays falsifiable — checked and ruled out:

- every `CPropertyData*` slot of the container (all 1126 containers scanned,
  each offset resolved by RTTI);
- `CPropertyDataBalancing` — only 2 live instances, both empty;
- the name table at `0x0127D410`, where `ResidentsWareConsumption` turned out
  to be a **statistics/achievement counter**, not balancing data.

The remaining route is to unpack the RDA data files and read the balancing XML.
Worth doing only if the per-tier table is wanted offline; the live measurement
is otherwise strictly better, since it already includes every modifier.

## Reading it live

```bash
python tools/research/annoprod.py --stock          # grouped by building type
python tools/research/annoprod.py --each -p 0      # one line per building
python tools/research/annoprod.py --measure 60     # + real production/consumption balance
```

Sample output, cross-checked against the running game:

```
=== First Island  (island 5, player 0)  warehouse capacity 20999
  building               makes           n  avg % paused   t/min  inputs
  FishermanHut           SaltFish        1  100.0      0    2.00  -
  LumberHut              Wood            1   96.0      0    1.44  -
  warehouse: Wood=40t, Tools=11t, SaltFish=40.327t
  demand coverage: SaltFish=100%

=== Odesville  (island 14, player 1)  warehouse capacity 20999
  CiderFarm              Cider           1  100.0      0    1.50  -
  FishermanHut           SaltFish        2  100.0      0    4.00  -
  LumberHut              Wood            8   98.8      8    0.00  -
```

The AI's eight lumber huts reading `paused 8` against a wood store of 41 t is
the incidental confirmation that `m_ProductionStopped` means what it says.
