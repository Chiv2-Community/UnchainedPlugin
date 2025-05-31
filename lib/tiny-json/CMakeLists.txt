cmake_minimum_required(VERSION 3.16)
project(tiny-json C)

# Create library
add_library(tiny-json STATIC
        tiny-json.c
)

# Set include directories for users of this library
target_include_directories(tiny-json PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}>
        $<INSTALL_INTERFACE:include>
)