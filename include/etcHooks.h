#pragma once

//FString* __cdecl
//UUserFeedbackAndBugReportsLibrary::GetGameInfo(FString* __return_storage_ptr__, UWorld* param_1)
DECL_HOOK(FString*, GetGameInfo, (FString* ret_ptr, void* uWorld))
{
	auto val = o_GetGameInfo(ret_ptr, uWorld);
#ifdef _DEBUG_CONSOLE
	std::wcout << "GetGameInfo: " << *val->str << std::endl;
#endif
	return val;
}