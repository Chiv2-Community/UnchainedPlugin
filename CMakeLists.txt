cmake_minimum_required(VERSION 3.5)

set(TARGET UnchainedPlugin)

set(VER_MAJOR 0 CACHE STRING "MAJOR.minor.patch.build")
set(VER_MINOR 5 CACHE STRING "major.MINOR.patch.build")
set(VER_PATCH 0 CACHE STRING "major.minor.PATCH.build")
set(VER_BUILD 0 CACHE STRING "major.minor.patch.BUILD")

option(PRINT_CLIENT_MSG "Output chat messages to CLI" OFF)
option(POST_BUILD_COPY "Copy output file to specified dir" OFF)
set(POST_BUILD_COPY_DIR "I:\\Epic Games\\Chivalry2_c\\TBL\\Binaries\\Win64\\Plugins" CACHE STRING "Directory to copy to")

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED True)

# logging
#set(CMAKE_C_FLAGS_DEBUG "${CMAKE_C_FLAGS_DEBUG} -D_DEBUG_CONSOLE")
set(CMAKE_CXX_FLAGS_DEBUG "${CMAKE_CXX_FLAGS_DEBUG} -D_DEBUG_CONSOLE")
#set(CMAKE_C_FLAGS_RELEASE "${CMAKE_C_FLAGS_RELEASE} -D_DEBUG_CONSOLE")
set(CMAKE_CXX_FLAGS_RELEASE "${CMAKE_CXX_FLAGS_RELEASE} -D_DEBUG_CONSOLE")

project(${TARGET} VERSION 1)

include_directories(${PROJECT_SOURCE_DIR}/${TARGET}/include)

add_subdirectory(${TARGET})