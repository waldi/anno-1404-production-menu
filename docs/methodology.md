# Methodology

How structure recovery actually works on this target, and the mistakes that cost
time. This is the reusable part — the specific offsets live in Ghidra and
`findings.json`.

## The core loop

Static analysis tells you a field *exists*. Only live memory tells you what it
*means*. Neither half is sufficient:

> Ghidra says `*(int *)(param_1 + 0x2cd8)` exists.
> Only live memory says that field is Peasants on the island you're looking at.

The loop that works:

1. **Find the class by name.** RTTI is present, so search strings for
   `.?AVCFoo@@`, then walk: type-descriptor name → `TypeDescriptor` (name − 8) →
   xref gives the Complete Object Locator → xref on the COL gives `vtable[-1]` →
   vtable starts 4 bytes later.
2. **Read the constructor.** It gives object size, and its `memset` / array-init
   calls reveal the shape of the field family for free. Note `param_1` is typically
   `uint *`, so Ghidra's `param_1 + 0xaa8` is byte offset `0x2AA0`.
3. **Enumerate instances live** by scanning for the vtable pointer (see the trap
   below). No container walk, no pointer chain.
4. **Correlate.** Take a quantity you already know how to read — a global, or a
   number visible in the UI — and find the field whose live value matches. Prefer a
   test that can *fail*: summing a candidate across all islands and requiring exact
   equality with the global rejects nine wrong candidates without ambiguity.
5. **Feed it back into Ghidra.** Define the struct, apply it, re-decompile. This is
   not bookkeeping — see "Why typing pays" below.

## Shortcut: the binary hands you its own field names

Before correlating anything, check whether the class has a **reflection
function**. Anno 1404 serialises save games by registering every field with its
name as a literal string:

```c
FUN_00b0fbe0(ar, "m_Productivity",      (int *)(this + 0x10), 100);
FUN_00b0fc60(ar, "m_ProductionStopped", (char *)(this + 0x14), '\0');
```

One per class, each an authoritative map of offset → name → type → default. No
correlation, no guessing. Finding them:

```
search_strings("^m_[A-Za-z]*$")   ->  pick a plausible field name
get_xrefs_to(that string)         ->  usually exactly one, the reflect function
```

On the property classes the reflect function sits at **vtable + 0x34**.

Two things this gives you beyond the field list:

- **Array lengths, proved.** A reflect helper that loops a name table emits one
  named record per entry, so the loop bound *is* the array length —
  `m_DemandsPerLevel` is 11 because the loop stops at `0x127b95c`, not because
  11 looked right. Cross-check by confirming the array ends exactly where the
  next named field begins.
- **The enums.** Those name tables are the game's own `char *[]` enums; the
  ware list, population levels and need categories were all found this way.

This does not replace live verification — a name tells you what a field is
called, not what the number means in context — but it removes most of the
guessing that step 4 would otherwise have to do.

## Traps that actually bit us

**Vtable scans return false positives.** Scanning for `0x010ffd00` returned 21 hits
for 19 real islands. One was a code address (the constructor's immediate operand —
filter to `>= 0x01400000`). The other was a stray heap dword that happened to match,
and its garbage contents dominated every cross-island sum, producing "counters" like
`1919376483` (which is ASCII).

*Always validate a scan hit against a constructor invariant.* `CIsland`'s is the
self-referential list sentinel: `[base+8] == base+8 && [base+0xC] == base+8`.
A secondary sanity field helps too (`+0x21D0 == 200`).

**A repeating stride is not necessarily a record size.** The community calculator's
population offsets are 0x20 apart, which looks like a 32-byte per-tier struct with
28 unexplored bytes. It isn't. It's `int[tier][8]` — one row of **8 player slots**.
The giveaway was the game indexing it as `base + 0x7cd8 + player*4` under a
`player < 8` guard. Check how the *code* indexes a table before inventing a record
layout for it.

**Two globals can hold the same pointer.** The calculator uses `0x12d5a90`; game
code uses `0x12d92d8`. They are the same `CGOPManager*`. A single live read settled
what no amount of static reading would have.

**Derived offsets need an empirical check before you trust aggregates.** Verifying
a couple of known-constant fields (`+0x21D0 == 200`) and the ctor sentinel takes one
command and catches a wrong base immediately.

**A vtable address read out of a static dump can be a string.** Reading the
Ghidra listing around `0x011004E0` gave what looked like a clean run of code
pointers, and one of them was recorded as `CPropertyFactory::vftable`. It was
the tail of the ASCII string `"Forgot to implement PropertyFactoryAPI %d\n"`,
read at the wrong alignment. The error surfaced immediately because the live
RTTI walk on that address returned `None` instead of a class name.

*Resolve a vtable by walking it, not by eyeballing a hex dump.* The walk —
`vtable[-1]` → COL → `+12` TypeDescriptor → `+8` name — either returns the
class name or fails loudly, so it is self-checking in a way that reading
addresses off a listing is not.

**Strings can be inline, not behind a pointer.** Searching a `CIsland` for *pointers
to* strings found nothing, and that was written up as "island names do not exist" —
a confirmed negative, with four supporting bullet points. It was wrong. The names are
stored **inline as `wchar_t[16]` UTF-16LE** inside the object, so no pointer to them
ever existed. The error was caught only because the user read the names off the
screen.

The lesson is procedural, not factual:

- When hunting for a string, search the **object's own bytes** for ASCII *and*
  UTF-16 before searching for pointers. Better still, search **all process memory
  for the literal** and then work backwards to whatever contains it — that is what
  finally located it, in one step, and it would have worked immediately.
- A negative result from a search is only as strong as the search's assumptions.
  "I looked and didn't find it" is not "it isn't there". Record *what was searched*,
  so the assumption is visible and falsifiable.
- Prefer a test with a ground truth you can check against reality. Every positive
  finding here was confirmed against a number the game itself displays; the one
  unconfirmed claim was the one that was wrong.

## Sometimes measuring beats finding

The per-good consumption rates were hunted for as static constants and never
found — container slots, balancing classes, name tables, all dead ends. They
were then obtained in about a minute of sampling, exactly, by watching the
warehouse drain.

What made it work is that the two flows have different *shapes*: consumption is
a single negative step on a strict 4.00 s tick, deliveries are positive jumps at
unrelated moments. Sampling well under the tick separates them without having to
pause the game or isolate a settlement. Look for that kind of structure before
assuming a measurement needs a controlled setup.

Two things this buys that a constant would not:

- **The real number, including every modifier.** A measured rate already
  accounts for whatever the constant would have to be adjusted by.
- **A falsification test.** Solving the measured rate back gave exactly the
  divisors the community calculator uses (200 peasants per fisherman's hut, 340
  per cider farm). Two independent numbers landing to the digit is much stronger
  evidence than finding a plausible-looking constant in memory would have been.

The corresponding trap: **nominal is not actual**. A producer at 100 %
productivity, unpaused, with a full warehouse delivers nothing. Any quantity
read straight out of an object is a nameplate figure until a flow measurement
says otherwise.

## When the call graph fails, watch the data

Finding *who writes this field* by working through xrefs can simply not
converge. Resident consumption was hunted through `CWarehouseChangeGoods`'s 37
call sites, several of which were read in full — and the answer was none of
them, because consumption does not use that function at all. Two sessions of
static work; the negative result only became visible when an execute breakpoint
on that function ran for 45 s and caught **only positive deltas**.

A hardware **data write-breakpoint** on one live structure field found the real
writer in a single run. `tools/research/annowatch.py` does this:

```bash
python tools/research/annowatch.py --stock SaltFish        # resolve the live nodes
python tools/research/annowatch.py 0x24048fdc --secs 30    # or a raw address
```

The register state at the trap identified the function, the ware id and the
before/after values at once, and the captured stack then answered a question
nobody had asked: the tick is driven from the **script layer**, which is why the
rate constants were never in the C++ object model and never could have been
found by reading it.

Reach for this whenever the question is "what changes this?" rather than "what
does this function do?".

### Two debugging traps, both expensive

- **WOW64 exception codes.** A 32-bit process delivers debug exceptions to a
  64-bit debugger as `STATUS_WX86_BREAKPOINT` (`0x4000001F`) and
  `STATUS_WX86_SINGLE_STEP` (`0x4000001E`), *not* `EXCEPTION_BREAKPOINT` /
  `EXCEPTION_SINGLE_STEP`. Match only the 64-bit codes and your own trap falls
  through as unhandled — Windows then terminates the game. This killed two
  sessions and was misdiagnosed first as anti-debug, then as a code-integrity
  check. The `--probe` mode that attaches and detaches *without writing
  anything* is what disproved both: build that escape hatch before theorising.
  Also use `Wow64Get/SetThreadContext`, and `DebugSetProcessKillOnExit(FALSE)`
  so the game survives a crashing script.
- **Prefer hardware breakpoints to INT3.** Debug registers write no game memory,
  so nothing can checksum them, and there is no byte to restore if the script
  dies.

## Prove a constant by changing it

`GetEconomyBalancingBlock()[+0x00] == 4000` sat right next to a field the code
demonstrably used as the history ring length, and the measured tick was 4.00 s.
That is a strong circumstantial case and it would have been easy to write up as
fact.

Instead: patch it to 2000, measure, restore. The period halved to exactly 2.00 s
on three settlements simultaneously and returned to 4.01 s on restore. One
minute of work turned "almost certainly" into "proven", and the same trick works
for any suspected constant once writes are permitted.

## Why typing pays

Defining `CIsland` and applying it wasn't documentation — it made the decompiler
produce new information. Re-decompiling `CIsland_SetTierPlayerState` with the type
applied turned index arithmetic into `this->aTierPlayerState[tier][slot]` and in the
same pass:

- confirmed `[11][8]` from the function's own `slot < 8 && tier < 0xb` bounds check,
  upgrading it from inference to assertion;
- confirmed `+0x3018` is satisfaction, because the value is explicitly clamped to 100;
- revealed a field we'd missed entirely at `+0x27E0` (linked-list heads per
  `[tier][player]`) that exactly filled a gap in the layout.

Type things as soon as you know them. The payoff compounds.

## Output discipline

This binary has ~109k functions. `QuestEvaluateConditions` alone decompiles to ~600
lines. Rules that keep sessions workable:

- Prefer targeted queries (`search_instructions` on a displacement, `get_xrefs_to`)
  over decompiling and reading.
- After reading a large function, write what it *proved* into a plate comment, then
  don't re-read it.
- Aggregate server-side. `findpop.py` reads 19 islands × 12 candidate arrays × 88
  values — that's ~20k numbers — and prints only non-zero row sums plus exact
  matches.

## Handing off

Anything worth keeping goes in Ghidra (types, names, plate comments) or
`findings.json` (machine-readable paths + confidence). A finding that exists only in
conversation is lost. A finding that exists in *both* prose and Ghidra will drift —
that already happened once, when a plate comment kept describing candidate arrays
after the population array had been confirmed.
