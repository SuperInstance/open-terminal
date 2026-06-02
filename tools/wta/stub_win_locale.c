// Stub Windows locale functions needed by windows-sys Win32_Globalization
// These are needed because sys-locale uses them, but on Linux we just return defaults.
#include <stddef.h>

int GetLocaleInfoEx(const void* lpLocaleName, int LCType, void* lpLCData, int cchData) {
    return 0;
}
int EnumSystemLocalesEx(void* lpLocaleEnumProc, int dwFlags, void* lParam, void* lpReserved) {
    return 0;
}
int GetDateFormatEx(const void* lpLocaleName, unsigned int dwFlags, const void* lpDate,
                    const void* lpFormat, void* lpDateStr, int cchDate, const void* lpCalendar) {
    return 0;
}
