#pragma once

struct UiRect { int left, top, right, bottom; };
struct DialogLayout {
    UiRect frame, title, help, close, settlementsHeader, productionHeader;
    UiRect settlements[19], production[14];
};
inline int UiMin(int a, int b) { return a < b ? a : b; }
inline int UiMax(int a, int b) { return a > b ? a : b; }

// Coordinates are relative to the parent GUI client area, not OS window pixels.
// Keep this pure so different resolutions and settlement counts can be tested.
inline DialogLayout MakeDialogLayout(int width, int height, int settlementCount) {
    DialogLayout l = {};
    const int margin = 10, padding = 30, gap = 30, bodyTop = 154;
    const int frameWidth = UiMax(1, UiMin(1280, width - 2 * margin));
    const int frameHeight = UiMax(1, UiMin(1024, height - 2 * margin));
    const int right = UiMax(padding + 1, frameWidth - padding);
    const int sidebar = UiMin(250, UiMax(80, (frameWidth - 2 * padding) / 3));
    const int productionLeft = padding + sidebar + gap;
    const int frameLeft = (width - frameWidth) / 2, frameTop = (height - frameHeight) / 2;
    l.frame = {frameLeft, frameTop, frameLeft + frameWidth, frameTop + frameHeight};
    l.title = {padding, padding, UiMax(padding + 1, right - 120), padding + 42};
    l.close = {UiMax(padding, right - 120), padding, right, padding + 48};
    l.help = {padding, 82, right, 106};
    l.settlementsHeader = {padding, 118, padding + sidebar, 140};
    l.productionHeader = {productionLeft, 118, right, 140};
    const int available = UiMax(1, frameHeight - padding - bodyTop);
    const int count = UiMax(1, UiMin(19, settlementCount));
    // The native skin has three fixed 16px rows. Wrap into columns rather
    // than clipping its bottom when many settlements must fit a short window.
    const int settlementHeight = 48;
    const int rows = UiMax(1, available / (settlementHeight + 2));
    const int columns = (count + rows - 1) / rows;
    const int columnWidth = (sidebar - (columns - 1) * 2) / columns;
    const int productionHeight = UiMax(1, UiMin(24, (available - 13 * 2) / 14));
    for (int i = 0; i < 19; ++i) {
        int y = bodyTop + (i % rows) * (settlementHeight + 2);
        int x = padding + (i / rows) * (columnWidth + 2);
        l.settlements[i] = {x, y, x + columnWidth, y + settlementHeight};
    }
    for (int i = 0; i < 14; ++i) {
        int y = bodyTop + i * (productionHeight + 2);
        l.production[i] = {productionLeft, y, right, y + productionHeight};
    }
    return l;
}
