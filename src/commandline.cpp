#include "commandline.h"
#include <windows.h>

// Returns position of a substring in command line args or -1
size_t CmdGetParam(const wchar_t* param)
{
	size_t res = std::wstring(GetCommandLineW()).find(param);
	bool found = (res != std::wstring::npos);

	//wprintf(L"CmdGetParam: %ls %ls %d\n", param, (found ? L":) found" : L":( not found"), found ? res : -1);
	return found ? res : -1;
}

// Returns parsed parameter (1 char spacing req), pre-/appends text if needed.
std::wstring CmdParseParam(const wchar_t* param, const wchar_t* addPrefix, const wchar_t* addSuffix)
{
	std::wstring commandLine = GetCommandLineW();
	size_t paramPos = CmdGetParam(param);
	if (paramPos == -1)
		return L"";

	size_t offset = paramPos + lstrlenW(param) + 1;
	size_t paramEnd = commandLine.find(L" ", offset);
	if (paramPos == -1)
		return L"";
	std::wstring res = commandLine.substr(offset, paramEnd - offset);

	return (addPrefix + res + addSuffix).c_str();
}