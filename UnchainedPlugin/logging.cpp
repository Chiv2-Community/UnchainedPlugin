#include <iostream>
#include "logging.h"

int logFString(FString str) {
	return logWideString(str.str);
}

void log(const char* str) {
#ifndef _DEBUG_CONSOLE
	return;
#endif
	std::cout << str << std::endl;

}


int logWideString(wchar_t* str) {
#ifndef _DEBUG_CONSOLE
	return 0;
#endif
	int i = 0;
	const wchar_t* tempStr = str; // Use a temporary pointer
	while (*tempStr != 0) {
		std::wcout << *tempStr;
		tempStr++; // Increment the temporary pointer instead of the input pointer
		i++;
	}
	std::wcout << std::endl;
	return i;
}


int logWideString(const wchar_t* str) {
#ifndef _DEBUG_CONSOLE
	return 0;
#endif
	int i = 0;
	const wchar_t* tempStr = str; // Use a temporary pointer
	while (*tempStr != 0) {
		std::wcout << *tempStr;
		tempStr++; // Increment the temporary pointer instead of the input pointer
		i++;
	}
	std::wcout << std::endl;
	return i;
}