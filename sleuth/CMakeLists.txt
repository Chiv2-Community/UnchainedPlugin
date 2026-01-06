cmake_minimum_required(VERSION 3.16)

project(sleuthlib C)

file(GLOB_RECURSE RUST_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs")

set(RUST_LIB ${CMAKE_CURRENT_SOURCE_DIR}/target/$<CONFIG>/sleuthlib.lib)

set_property(
    DIRECTORY
    APPEND
    PROPERTY ADDITIONAL_CLEAN_FILES
    ${CMAKE_CURRENT_SOURCE_DIR}/target
)

add_custom_command(
    OUTPUT ${RUST_LIB}
    COMMAND rustup run nightly-x86_64-pc-windows-msvc cargo build $<$<CONFIG:Release>:--release>
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    DEPENDS
        ${RUST_SOURCES}
        ${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml
    COMMENT "Building Rust library sleuthlib for $<CONFIG>..."
)

# Custom target that depends on the output
add_custom_target(RustBuild
    DEPENDS ${RUST_LIB}
)


add_library(${PROJECT_NAME}_interface INTERFACE)
add_dependencies(${PROJECT_NAME}_interface RustBuild)

# Link directories for the current configuration
target_link_directories(${PROJECT_NAME}_interface INTERFACE
    ${CMAKE_CURRENT_SOURCE_DIR}/target/$<CONFIG>
)

# MSVC runtime library (Release vs Debug)
set(MSVC_RUNTIME_LIB "$<$<CONFIG:Release>:msvcrt.lib>$<$<CONFIG:Debug>:msvcrtd.lib>")

# Link libraries
target_link_libraries(${PROJECT_NAME}_interface INTERFACE
    ${PROJECT_NAME}  # your main C/C++ project lib

    # Native system libs required by Rust
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
)
