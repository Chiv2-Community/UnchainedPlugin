cmake_minimum_required(VERSION 3.16)

project(MinHook C)

# Add include directory
include_directories(include)

# Collect source files
file(GLOB_RECURSE MINHOOK_SOURCES
        "src/*.c"
)

# Create library
add_library(MinHook STATIC ${MINHOOK_SOURCES})

# Set include directories for users of this library
target_include_directories(MinHook PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
        $<INSTALL_INTERFACE:include>
)