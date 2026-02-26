cmake_minimum_required(VERSION 3.10)

# Set default version if not specified
if(NOT DEFINED VERSION)
    set(VERSION "0.0.0")
else()
    # Remove the 'v' prefix if it exists
    string(REGEX REPLACE "^v" "" VERSION ${VERSION})
endif()

project(UnchainedPlugin
        VERSION ${VERSION}
        LANGUAGES CXX C)

# Parse the version into components
string(REGEX MATCH "^([0-9]+)\\.([0-9]+)\\.([0-9]+)" VERSION_MATCH ${VERSION})
if(VERSION_MATCH)
    set(VERSION_MAJOR ${CMAKE_MATCH_1})
    set(VERSION_MINOR ${CMAKE_MATCH_2})
    set(VERSION_PATCH ${CMAKE_MATCH_3})
else()
    # Fallback if version doesn't match expected format
    set(VERSION_MAJOR 0)
    set(VERSION_MINOR 0)
    set(VERSION_PATCH 0)
endif()


# C++ standard requirements
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

set(CMAKE_BUILD_PARALLEL_LEVEL ${CMAKE_HOST_SYSTEM_PROCESSOR_COUNT})

set(CMAKE_MSVC_RUNTIME_LIBRARY "$<IF:$<CONFIG:Release>,MultiThreadedDLL,MultiThreadedDebugDLL>")

# Options
option(POST_BUILD_COPY "Copy output file to specified dir" OFF)
set(POST_BUILD_COPY_DIR "I:\\Epic Games\\Chivalry2_c\\TBL\\Binaries\\Win64\\Plugins" CACHE STRING "Directory to copy to")

# Generates a header file which includes all the headers of the source dir.
function(generate_include_all_header SOURCE_DIR FILE_NAME)
    file(GLOB HEADER_FILES "${SOURCE_DIR}/*.h" "${SOURCE_DIR}/*.hpp")
    set(INCLUDE_ALL_CONTENT "// GENERATED FILE, DO NOT MODIFY\n")
    string(APPEND INCLUDE_ALL_CONTENT "// GENERATED FILE, DO NOT MODIFY\n")
    string(APPEND INCLUDE_ALL_CONTENT "// GENERATED FILE, DO NOT MODIFY\n")
    string(APPEND INCLUDE_ALL_CONTENT "// GENERATED FILE, DO NOT MODIFY\n")
    string(APPEND INCLUDE_ALL_CONTENT "\n")
    string(APPEND INCLUDE_ALL_CONTENT "// Reload the cmake configuration to regenerate this file.\n")
    string(APPEND INCLUDE_ALL_CONTENT "#pragma once\n")
    foreach(header ${HEADER_FILES})
        get_filename_component(header_name ${header} NAME)
        if(NOT header_name MATCHES ${FILE_NAME})
            string(APPEND INCLUDE_ALL_CONTENT "#include \"${header_name}\"\n")
        endif()
    endforeach()
    file(WRITE "${SOURCE_DIR}/${FILE_NAME}" "${INCLUDE_ALL_CONTENT}")
endfunction()



# Configure version resource
configure_file(
    ${CMAKE_CURRENT_SOURCE_DIR}/version.rc.in
    ${CMAKE_CURRENT_BINARY_DIR}/version.rc
    @ONLY
)

# Add dependencies
add_subdirectory(lib/MinHook)

# Target properties
set_target_properties(${PROJECT_NAME} PROPERTIES
    RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/Output"
)

add_subdirectory(sleuth)

# Define the library as a wrapper that links sleuthlib
# Since sleuthlib is a cdylib, it will contain everything
add_library(${PROJECT_NAME} SHARED 
    ${CMAKE_CURRENT_BINARY_DIR}/version.rc
)

# Target properties for the wrapper
set_target_properties(${PROJECT_NAME} PROPERTIES
    LINKER_LANGUAGE C
)

target_link_libraries(${PROJECT_NAME} PRIVATE
    sleuthlib_interface
)

# Post-build copy
if(POST_BUILD_COPY)
    add_custom_command(
        TARGET ${PROJECT_NAME} POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy
            "$<TARGET_FILE:${PROJECT_NAME}>"
            "${POST_BUILD_COPY_DIR}/${PROJECT_NAME}.dll"
        COMMAND ${CMAKE_COMMAND} -E echo
            "${PROJECT_NAME}.dll copied to ${POST_BUILD_COPY_DIR}"
    )
endif()