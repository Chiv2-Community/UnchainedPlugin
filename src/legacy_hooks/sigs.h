#pragma once

// https://stackoverflow.com/a/8349281

#define FUNCTYPES \
	etype(MaxFuncType) //this always needs to be last

#define etype(x) uint32_t x = 0x0;


#define etype(x) F_##x,
typedef enum { FUNCTYPES }  FuncType;
#undef etype
#define etype(x) #x,
static const char* strFunc[F_MaxFuncType + 1] = { FUNCTYPES };

static const char* signatures[F_MaxFuncType + 1] =
{
	/*MaxFuncType*/
	""
};
