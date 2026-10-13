#pragma once
// Population divisors from the repository's NeedsCalculator.cs. These are
// estimates: only peasant fish/cider have been verified against live deductions.
// Output rates were read from loaded CPropertyDataWareProduction on 2026-10-09
// (docs/live-production-rates.json). Indices are the game's 11 population rows.
struct DemandRate { const char* good; double output; double divisors[11]; int tier = -1; int unlockAbove = 0; };
static const DemandRate demandRates[] = {
    {"SaltFish",2,{0,0,285,0,0,0,0,200,500,909,1250}},
    {"Spices",2,{0,0,0,0,0,0,0,0,500,909,1250}},
    {"Bread",4,{0,0,0,0,0,0,0,0,0,727,1025}},
    {"SaltMeat",2.5,{0,0,0,0,0,0,0,0,0,0,1136}},
    {"Cider",1.5,{0,0,500,0,0,0,0,340,340,652,1153}},
    {"Beer",1.5,{0,0,0,0,0,0,0,0,0,625,1071},9,510},
    {"Wine",2,{0,0,0,0,0,0,0,0,0,0,1000},10,1500},
    {"LinenHabit",2,{0,0,0,0,0,0,0,0,476,1052,2500}},
    {"LeatherJerkin",4,{0,0,0,0,0,0,0,0,0,1428,2500},9,690},
    {"FurCoat",2.5,{0,0,0,0,0,0,0,0,0,0,1562},10,950},
    {"BrocadeRobe",3,{0,0,0,0,0,0,0,0,0,0,2112},10,4000},
    {"Book",3,{0,0,0,0,0,0,0,0,0,1875,3333},9,940},
    {"CandleStick",2,{0,0,0,0,0,0,0,0,0,2500,3333},10,3000},
    {"Glasses",2,{0,0,0,0,0,0,0,0,0,0,1709},10,2200},
    {"Dates",3,{0,0,0,0,450,600,0,0,0,0,0}},
    {"Milk",1.5,{0,0,0,0,436,666,0,0,0,0,0},4,145},
    {"Carpets",1.5,{0,0,0,0,909,1500,0,0,0,0,0},4,295},
    {"Coffee",1,{0,0,0,0,0,1000,0,0,0,0,0}},
    {"PearlNecklace",1,{0,0,0,0,0,751,0,0,0,0,0},5,1040},
    {"Perfume",1,{0,0,0,0,0,1250,0,0,0,0,0},5,2600},
    {"Marzipan",4,{0,0,0,0,0,2453,0,0,0,0,0},5,4360},
};
static double resident_demand(const DemandRate& rate, const int population[11], const int global[11]) {
    double demand = 0;
    for (int tier = 0; tier < 11; ++tier) {
        if (!rate.divisors[tier]) continue;
        // Candlesticks use the noblemen gate for both consuming tiers.
        if ((tier == rate.tier || std::string(rate.good) == "CandleStick") &&
            rate.tier >= 0 && global[rate.tier] <= rate.unlockAbove) continue;
        demand += population[tier] / rate.divisors[tier] * rate.output;
    }
    return demand;
}
