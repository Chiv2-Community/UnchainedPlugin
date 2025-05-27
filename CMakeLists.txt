cmake_minimum_required(VERSION 3.10)

set(TARGET UnchainedPlugin)

set(VER_MAJOR 0 CACHE STRING "MAJOR.minor.patch.build")
set(VER_MINOR 4 CACHE STRING "major.MINOR.patch.build")
set(VER_PATCH 6 CACHE STRING "major.minor.PATCH.build")
set(VER_BUILD 0 CACHE STRING "major.minor.patch.BUILD")

option(PRINT_CLIENT_MSG "Output chat messages to CLI" OFF)
option(POST_BUILD_COPY "Copy output file to specified dir" OFF)
set(POST_BUILD_COPY_DIR "I:\\Epic Games\\Chivalry2_c\\TBL\\Binaries\\Win64\\Plugins" CACHE STRING "Directory to copy to")

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

project(${TARGET} VERSION "${VER_MAJOR}.${VER_MINOR}.${VER_PATCH}" LANGUAGES CXX C)

# Adjust include directories for the new structure
include_directories(include)

# Update file glob patterns to match the new directory structure
file(GLOB SOURCES "src/*.cpp" "include/*.cpp")
file(GLOB HEADERS "include/*.h" "include/*.hpp")

# Update source groups to match the new file paths
source_group("Source Files\\Utils" FILES
    src/logging.cpp
    src/commandline.cpp
    src/nettools.cpp
)

source_group("Header Files\\Utils" FILES
    include/resource.h
    include/commandline.h
    include/logging.hpp
    include/nettools.h
)

source_group("Header Files\\Hooks" FILES
    include/adminControl.h
    include/assetLoading.h
    include/backendHooks.h
    include/etcHooks.h
    include/ownershipOverrides.h
    include/sigs.h
    include/unchainedIntegration.h
)

# Fix the path for version.rc.in
configure_file(
    ${CMAKE_CURRENT_SOURCE_DIR}/version.rc.in
    ${CMAKE_CURRENT_BINARY_DIR}/version.rc
    @ONLY)

add_subdirectory(lib/MinHook)
add_subdirectory(lib/tiny-json)
add_subdirectory(lib/quill)

add_library(${TARGET} SHARED ${SOURCES} ${HEADERS} ${CMAKE_CURRENT_BINARY_DIR}/version.rc)

target_compile_definitions(${TARGET} PRIVATE
    NOMINMAX                     # Prevent Windows.h from defining min/max macros
    $<$<CONFIG:Debug>:_DEBUG>    # Define _DEBUG in Debug configuration
    $<$<CONFIG:Debug>:_DEBUG_CONSOLE>
    $<$<CONFIG:Release>:_DEBUG_CONSOLE>
)

# Use QUILL_STATIC_LIB instead of QUILL_DLL_IMPORT
target_compile_definitions(${TARGET} PRIVATE QUILL_STATIC_LIB)

# Update include directories
target_include_directories(${TARGET} PUBLIC 
    include 
    lib/Sig
)

# Set output directory if needed
set_target_properties(${TARGET} PROPERTIES
    RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/Output"
)

if(PRINT_CLIENT_MSG)
    target_compile_definitions(${TARGET} PUBLIC PRINT_CLIENT_MSG)
endif()

if(POST_BUILD_COPY)
    add_custom_command(
        TARGET ${TARGET} POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy
        $<TARGET_FILE:${TARGET}>
        ${POST_BUILD_COPY_DIR}/${TARGET}.dll && echo
        ${TARGET}.dll copied to ${POST_BUILD_COPY_DIR}
    )
endif()

# Link against libraries - the order matters
target_link_libraries(${TARGET}
    PRIVATE MinHook tiny-json winhttp
    PUBLIC quill::quill
)