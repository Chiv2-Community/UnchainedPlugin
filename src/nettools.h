#pragma once
#include <string>
//TODO: document this
std::wstring GetApiUrl(const wchar_t* path);
//TODO: document this
std::wstring HTTPGet(const std::wstring* url);