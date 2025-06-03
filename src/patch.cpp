#include "patch.hpp"

bool Ptch_Nop(void* address, int size)
{
    unsigned long protect[2];
    auto res1 = VirtualProtect((void*)address, size, PAGE_EXECUTE_READWRITE, &protect[0]);
    if (!res1) return false;

    memset((void*)address, 0x90, size);
    return VirtualProtect((void*)address, size, protect[0], &protect[1]);
}

bool Ptch_Repl(void* address, DWORD newVal)
{
    DWORD d;
    auto res1 = VirtualProtect(address, 1, PAGE_EXECUTE_READWRITE, &d);
    if (!res1) return false;
    *static_cast<unsigned char *>(address) = 0xEB; // Patch to JMP
    return VirtualProtect(address, 1, d, NULL);
}