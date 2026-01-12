cmake_minimum_required(VERSION 3.16)

project(sleuthlib C)

file(GLOB_RECURSE RUST_SOURCES
    "${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs"
)

set(RUST_TARGET_DIR
    $<IF:$<OR:$<CONFIG:Debug>,$<CONFIG:RelWithDebInfo>>,
        ${CMAKE_CURRENT_SOURCE_DIR}/target/debug,
        ${CMAKE_CURRENT_SOURCE_DIR}/target/release
    >
)

set_property(
    DIRECTORY
    APPEND
    PROPERTY ADDITIONAL_CLEAN_FILES
    ${CMAKE_CURRENT_SOURCE_DIR}/target
)

add_custom_target(RustBuild
    COMMAND rustup run nightly-x86_64-pc-windows-msvc cargo build
        $<$<OR:$<CONFIG:Release>,$<CONFIG:MinSizeRel>>:--release>
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    DEPENDS
        ${RUST_SOURCES}
        ${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml
    COMMENT "Building Rust library sleuthlib ($<CONFIG>)"
)

add_library(sleuthlib_interface INTERFACE)
add_dependencies(sleuthlib_interface RustBuild)

target_link_directories(sleuthlib_interface INTERFACE
    ${RUST_TARGET_DIR}
)

set(MSVC_RUNTIME_LIB
    $<$<CONFIG:Debug>:msvcrtd.lib>
    $<$<NOT:$<CONFIG:Debug>>:msvcrt.lib>
)

target_link_libraries(sleuthlib_interface INTERFACE
    sleuthlib

    advapi32.lib
    cfgmgr32.lib
    gdi32.lib
    kernel32.lib
    msimg32.lib
    ${MSVC_RUNTIME_LIB}
    ntdll.lib
    ole32.lib
    opengl32.lib
    shell32.lib
    user32.lib
    userenv.lib
    winspool.lib
    ws2_32.lib
    psapi.lib
    bcrypt.lib
    Dbghelp.lib
)
