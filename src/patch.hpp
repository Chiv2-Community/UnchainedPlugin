#pragma once

#include <Windows.h>

bool Ptch_Nop(void *address, int size);

bool Ptch_Repl(void *address, DWORD newVal);
