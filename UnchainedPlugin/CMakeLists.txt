cmake_minimum_required(VERSION 3.5)

project(${TARGET})

file(GLOB SOURCES "*.cpp" "include/*.cpp")
file(GLOB HEADERS "*.h" "include/*.h")

source_group("Source Files\\Utils" FILES
    logging.cpp
    include/commandline.cpp
    include/nettools.cpp
)

source_group("Header Files\\Utils" FILES
    include/resource.h
    include/commandline.h
    include/logging.h
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
    include/FunctionHook.hpp
    include/FunctionHookEnabler.hpp
)

configure_file(
  ${CMAKE_CURRENT_SOURCE_DIR}/version.rc.in
  ${CMAKE_CURRENT_BINARY_DIR}/version.rc
  @ONLY)

add_library(${TARGET} SHARED ${SOURCES} ${HEADERS} ${CMAKE_CURRENT_BINARY_DIR}/version.rc)

# add_library(${TARGET} SHARED ${SOURCES} ${HEADERS} Resource.rc)
target_include_directories(${TARGET} PUBLIC include "Lib/Sig")

set_target_properties(${TARGET} PROPERTIES
    RUNTIME_OUTPUT_DIRECTORY "../../Output"
)
# install(TARGETS ${TARGET} RUNTIME DESTINATION "../Output/Cpack")
# include(CPack)
# set(CPACK_INSTALL_CMAKE_PROJECTS
# "${CMAKE_CURRENT_BINARY_DIR}/Output/Debug;${TARGET};ALL;/"
# "${CMAKE_CURRENT_BINARY_DIR}/Output/Release;${TARGET};ALL;/"
#     )

if(PRINT_CLIENT_MSG)
    target_compile_definitions(${TARGET} PUBLIC PRINT_CLIENT_MSG)
endif()

if(POST_BUILD_COPY)
    add_custom_command(
        TARGET ${TARGET}  POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy
                $<TARGET_FILE:${TARGET}>
                ${POST_BUILD_COPY_DIR}/${TARGET}.dll && echo
                ${TARGET}.dll copied to ${POST_BUILD_COPY_DIR}
    )
endif()

add_subdirectory(Lib/MinHook)
add_subdirectory(Lib/tiny-json)

target_link_libraries(${TARGET} MinHook tiny-json winhttp)
