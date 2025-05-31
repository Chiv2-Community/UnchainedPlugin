#include "patch.hpp"

void Ptch_Nop(unsigned char* address, int size)
{
    unsigned long protect[2];
    VirtualProtect((void*)address, size, PAGE_EXECUTE_READWRITE, &protect[0]);
    memset((void*)address, 0x90, size);
    VirtualProtect((void*)address, size, protect[0], &protect[1]);
}

void Ptch_Repl(unsigned char* address, DWORD newVal)
{
    DWORD d;
    VirtualProtect((void*)address, 1, PAGE_EXECUTE_READWRITE, &d);
    *address = 0xEB; // Patch to JMP
    VirtualProtect((void*)address, 1, d, NULL);
}
