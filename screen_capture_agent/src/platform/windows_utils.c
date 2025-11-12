#include <windows.h>
#include <stdio.h>
#include <stdint.h>

BOOL selectInputDesktop() {
    HDESK desktop = OpenInputDesktop(0, FALSE, GENERIC_ALL);
    if (desktop == NULL) {
        return FALSE;
    }
    BOOL result = SetThreadDesktop(desktop);
    CloseDesktop(desktop);
    return result;
}

BOOL inputDesktopSelected() {
    HDESK current = GetThreadDesktop(GetCurrentThreadId());
    if (current == NULL) {
        return FALSE;
    }
    
    HDESK input = OpenInputDesktop(0, FALSE, GENERIC_READ);
    if (input == NULL) {
        return FALSE;
    }
    
    BOOL same = (current == input);
    CloseDesktop(input);
    return same;
}

int handleMask(uint8_t *out, const uint8_t *mask, int32_t width, int32_t height, int32_t bmWidthBytes, int32_t bmHeight) {
    const uint8_t *andMask = mask;
    int32_t andMaskSize = bmWidthBytes * bmHeight;
    int32_t offset = height * bmWidthBytes;
    const uint8_t *xorMask = mask + offset;
    int32_t xorMaskSize = andMaskSize - offset;
    int doOutline = 0;
    
    for (int y = 0; y < height; y++) {
        for (int x = 0; x < width; x++) {
            int byte = y * bmWidthBytes + x / 8;
            int bit = 7 - x % 8;

            if (byte < andMaskSize && !(andMask[byte] & (1 << bit))) {
                // Valid pixel, so make it opaque
                out[3] = 0xff;

                // Black or white?
                if (xorMask[byte] & (1 << bit))
                    out[0] = out[1] = out[2] = 0xff;
                else
                    out[0] = out[1] = out[2] = 0;
            }
            else if (byte < xorMaskSize && xorMask[byte] & (1 << bit)) {
                // Replace any XORed pixels with black, because RFB doesn't support
                // XORing of cursors.  XORing is used for the I-beam cursor, which is most
                // often used over a white background, but also sometimes over a black
                // background.  We set the XOR'd pixels to black, then draw a white outline
                // around the whole cursor.

                out[0] = out[1] = out[2] = 0;
                out[3] = 0xff;

                doOutline = 1;
            }
            else {
                // Transparent pixel
                out[0] = out[1] = out[2] = out[3] = 0;
            }

            out += 4;
        }
    }
    return doOutline;
}

void drawOutline(uint8_t *outline, const uint8_t *colors, int32_t width, int32_t height, int32_t outlineLen) {
    memset(outline, 0, outlineLen);
    int newWidth = width + 2;
    int newHeight = height + 2;
    
    for (int y = 0; y < height; y++) {
        for (int x = 0; x < width; x++) {
            int srcIdx = (y * width + x) * 4;
            int dstIdx = ((y + 1) * newWidth + (x + 1)) * 4;
            
            memcpy(&outline[dstIdx], &colors[srcIdx], 4);
            
            if (colors[srcIdx + 3] > 0) {
                for (int dy = -1; dy <= 1; dy++) {
                    for (int dx = -1; dx <= 1; dx++) {
                        int ny = y + 1 + dy;
                        int nx = x + 1 + dx;
                        if (ny >= 0 && ny < newHeight && nx >= 0 && nx < newWidth) {
                            int borderIdx = (ny * newWidth + nx) * 4;
                            if (outline[borderIdx + 3] == 0) {
                                outline[borderIdx] = outline[borderIdx + 1] = outline[borderIdx + 2] = 0;
                                outline[borderIdx + 3] = 255;
                            }
                        }
                    }
                }
            }
        }
    }
}

int get_di_bits(uint8_t *out, HDC dc, HBITMAP hbm, int32_t width, int32_t height) {
    BITMAPINFO bi;
    memset(&bi, 0, sizeof(bi));
    bi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    bi.bmiHeader.biWidth = width;
    bi.bmiHeader.biHeight = -height;
    bi.bmiHeader.biPlanes = 1;
    bi.bmiHeader.biBitCount = 32;
    bi.bmiHeader.biCompression = BI_RGB;

    if (GetDIBits(dc, hbm, 0, height, out, &bi, DIB_RGB_COLORS) == 0) {
        return GetLastError();
    }
    return 0;
}

