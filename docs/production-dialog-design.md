# Production dialog: paired capacity bars

Proposal, 2026-10-08. Based on the user-provided Anno 1800 production screenshot
and the current dialog audit. No runtime UI changes made by this analysis.

## Implementation status (2026-10-08)

Implemented in the V4 companion and live-verified after automatic restart:
paired native blue/green progress bars, native ware icons from dataset atlas
mapping, t/min labels, normalized per-row fill, separate visible empty tracks,
capacity status, shortage-first ordering and paging. Rate aggregation is stored
as values in the background snapshot. Peasant demand is calculated for SaltFish
and Cider; other demand remains Unknown. Potential excludes paused buildings.
The current 1440x1080 capture is
`../Anno 1404 Gold Edition/production-paired-bars-final_full.png`.

Still needs validation: next/previous page clicks, 600px viewport, mixed-tier
demand and warehouse-limited delivery. The current rate is nominal potential,
not actual delivered goods.

## Native feasibility

Ghidra confirms CGUIWndProgressBar (type9), CURRENTVALUE/MAXVALUE integers and
PB_Bar/PB_Overlay/PB_Background skin layers. Renderer0x00DB9EF0 requires max>1;
definition parser0x00DB9A40 handles values, bar color, vertical/reverse flags.
Existing repo findings document live working textured native bars. Extracted
objectmenulistprogressbarblue.bkg and objectmenulistprogressbargreen.bkg provide
PB_Bar artwork (12px high source tiles); these exact skins are not live-tested
in the proposed paired layout. Their empty PB_Background needs a separate row
or track surface for the unfilled portion to remain readable.

Use two PROGRESSBAR elements plus overlay LABELs for values. A native progress
bar does not itself provide the proposed numerical captions. Reuse native
goods ICONGUID labels after verifying each ware-to-icon mapping.

## Row design

Use a quiet opaque native panel with 72–80px product rows. Each row contains:
- A 40px product icon and readable localized product name.
- Blue upper track: Demand, one value in t/min.
- Green lower track: Potential output, one value in t/min.
- A fixed balance column: signed potential minus demand, in t/min, with
  Missing capacity / Capacity available / Balanced. Retain words with color.

Put captions in aligned label areas or overlay them on the full track, keeping
their position independent of the fill width. One compact legend explains the
colors and per-row normalization. Never omit product names in favor of icons.

## Comparison rule

For known nonnegative demand D and potential P, choose M=max(D,P).
When M>0, demand fill=D/M, potential fill=P/M. Thus the larger fills100%, equal
positive values fill both100%, and the smaller immediately communicates ratio.
Use native MAXVALUE10000, CURRENTVALUE=round(10000*value/M).
If D=P=0, leave both empty and show No demand / No capacity.
Unknown demand is Unknown, not zero; do not show a comparative balance verdict.

Example: D2,P3 t/min -> upper67%, lower100%, capacity margin+1 t/min.
Example: D3,P2 -> upper100%, lower67%, missing capacity1 t/min.
Each row has its own scale: widths compare demand with output within a good,
not absolute volumes between different goods. Numeric labels preserve magnitude.

## Data contract

Current Produced caption is a building count. Need is population/200 for fish
or /340 for cider, expressed as producer equivalents. They must not become
rate-labelled bars without conversion.
describe_building already computes rate from amount, cycle and productivity,
zeroing paused producers, but SnapshotWorker retains only counts. Preserve and
sum nominal rates per ware/island in the value-only snapshot. Define Potential
output as nominal output at current productivity with paused producers excluded;
tooltip: capacity assuming inputs and warehouse space, not measured delivery.
Full100% installed capacity including paused buildings is a separate metric,
not silently interchangeable with this definition.

Demand needs verified per-class consumption and relevant industry inputs.
Current aggregate-population fish/cider estimate is not complete demand for
all citizen tiers or goods. Mark supported estimates; unsupported values remain
Unknown until the demand model is expanded. Do not infer balance from buildings.

## Dialog structure and rollout

Keep fullscreen10px margin/30px padding; large selected-scope title, left
settlement list, right product list. Explicit selected marker plus native checked
artwork; one Ctrl-click hint, selected-count summary for multi-selection.
Default order: missing capacity first; offer alphabetical order and a shortages
filter later. Use scrolling instead of the existing12-good silent limit or
narrow settlement columns. Keep unknown-demand goods visible but neutral.

Extend the versioned batched UI transport with typed bar value/max updates;
preflight type9, bounds and capacities before mutation on GUI thread. A richer
row creates more widgets than the current64-entry batch supports; size capacity
for the full visible row set and bump protocol version with DLL/companion
installed together. Keep sampling independent of selection and rendering.

Implement in order: rate/demand contract, one native row proving icon/skin/text
and zero/unknown states, full batched list, scroll/selection behavior, live visual
checks at multiple resolutions. Follow standing automatic restart workflow
after implementation. This proposal itself requires no restart.
