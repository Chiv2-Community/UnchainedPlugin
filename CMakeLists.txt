cmake_minimum_required(VERSION 3.16)

# Project name and language
project(UnchainedPlugin VERSION 1.0.0 LANGUAGES CXX C)


# Set C++ standard
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# Set output directories
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib)
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib)

# Conditionally enable debug features
if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    add_compile_definitions(DEBUG_BUILD)
    add_compile_definitions(_DEBUG_CONSOLE)
endif()

# Add include directories
include_directories(
        ${PROJECT_SOURCE_DIR}/include
        ${PROJECT_SOURCE_DIR}/lib/MinHook/include
        ${PROJECT_SOURCE_DIR}/lib/Sig
        ${PROJECT_SOURCE_DIR}/lib/tiny-json
)

# Collect source files
file(GLOB_RECURSE SOURCES
        "src/*.cpp"
        "src/*.c"
)

# Handle .cpp files in include directory
file(GLOB INCLUDE_CPPS
        "include/*.cpp"
)
list(APPEND SOURCES ${INCLUDE_CPPS})

# Build libraries
add_subdirectory(lib/MinHook)
add_subdirectory(lib/tiny-json)

# Configure version resource file
configure_file(
        ${CMAKE_CURRENT_SOURCE_DIR}/version.rc.in
        ${CMAKE_CURRENT_BINARY_DIR}/version.rc
        @ONLY
)

# Create the main target as a shared library (DLL)
add_library(${PROJECT_NAME} SHARED
        ${SOURCES}
        ${CMAKE_CURRENT_BINARY_DIR}/version.rc
)

# Link libraries
target_link_libraries(${PROJECT_NAME}
        PRIVATE
        MinHook
        tiny-json
        winhttp.lib
)

# Set additional compiler flags
if(MSVC)
    target_compile_options(${PROJECT_NAME} PRIVATE /W4)
else()
    target_compile_options(${PROJECT_NAME} PRIVATE -Wall -Wextra -Wpedantic -fpermissive -msse4.2)
endif()

# Install rules (optional)
install(TARGETS ${PROJECT_NAME}
        RUNTIME DESTINATION bin
        LIBRARY DESTINATION lib
        ARCHIVE DESTINATION lib
)