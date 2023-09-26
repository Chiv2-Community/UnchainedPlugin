﻿#pragma once
#include <Sig/Sig.hpp>
#include "sigs.h"
#include "Logging.h"

const char* logo = R"( 
_________  .__     .__                .__                    ________             
\_   ___ \ |  |__  |__|___  _______   |  |  _______  ___.__. \_____  \        
/    \  \/ |  |  \ |  |\  \/ /\__  \  |  |  \_  __ \<   |  |  /  ____/   
\     \____|   Y  \|  | \   /  / __ \_|  |__ |  | \/ \___  | /       \    
 \______  /|___|  /|__|  \_/  (____  /|____/ |__|    / ____| \_______ \       
        \/      \/                 \/                \/              \/      
 ____ ___                 .__             .__                     .___            
|    |   \  ____    ____  |  |__  _____   |__|  ____    ____    __| _/           
|    |   / /    \ _/ ___\ |  |  \ \__  \  |  | /    \ _/ __ \  / __ |      
|    |  / |   |  \\  \___ |   Y  \ / __ \_|  ||   |  \\  ___/ / /_/ |      
|______/  |___|  / \___  >|___|  /(____  /|__||___|  / \___  >\____ |          
               \/      \/      \/      \/          \/      \/      \/      
)";

uint64_t FindSignature(HMODULE baseAddr, DWORD size, const char* title, const char* signature)
{
	auto logger = el::Loggers::getLogger("FindSignature");


	const void* found = nullptr;
	found = Sig::find(baseAddr, size, signature);
	uint64_t diff = 0;
	if (found != nullptr)
	{
		diff = (uint64_t)found - (uint64_t)baseAddr;
		//std::cout << title << ": 0x" << std::hex << diff << std::endl;
		logger->info("?? -> %v : 0x%v", title, diff);
	}
	else
		logger->error("!! -> %v : nullptr", title);
		//std::cout << title << ": nullptr" << std::endl;

	return diff;

}
// Hook macros

HMODULE baseAddr;
MODULEINFO moduleInfo;

#define DECL_HOOK(retType, funcType, args)    \
	typedef retType (*funcType##_t) args;		\
	funcType##_t o_##funcType;					\
	retType hk_##funcType args

#define HOOK_ATTACH(moduleBase, funcType) \
	MH_CreateHook(moduleBase + curBuild.offsets[F_##funcType], hk_##funcType, reinterpret_cast<void**>(&o_##funcType)); \
	MH_EnableHook(moduleBase + curBuild.offsets[F_##funcType]); 

/*
#define HOOK_FIND_SIG(funcType) \
	if (curBuild.offsets[F_##funcType] == 0)\
		curBuild.offsets[F_##funcType] = FindSignature(baseAddr, moduleInfo.SizeOfImage, #funcType, signatures[F_##funcType]); \
	else printf("-> %s : (conf)", #funcType);
	//long long sig_##funcType = FindSignature(baseAddr, moduleInfo.SizeOfImage, #funcType, signatures[F_##funcType]);
*/