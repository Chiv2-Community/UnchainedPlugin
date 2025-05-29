#pragma once

#include <Windows.h>

void Ptch_Nop(unsigned char *address, int size);

void Ptch_Repl(unsigned char *address, DWORD newVal);
