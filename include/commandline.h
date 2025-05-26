#pragma once
#include <iostream>

// Returns position of a substring in command line args or -1
size_t CmdGetParam(const wchar_t* param);

// Returns parsed parameter (1 char spacing req), pre-/appends text if needed.
std::wstring CmdParseParam(const wchar_t* param, const wchar_t* addPrefix = L"", const wchar_t* addSuffix = L"");