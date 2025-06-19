file(GLOB_RECURSE RUST_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs")

project(sleuthlib C)

add_custom_command(
    OUTPUT ${CMAKE_CURRENT_SOURCE_DIR}/target/release/sleuthlib.lib
    COMMAND cargo build --release
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    DEPENDS
        # ${CMAKE_CURRENT_SOURCE_DIR}/sleuthlib.rs
        ${RUST_SOURCES}
        ${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml
    COMMENT "Building Rust library..."
)

add_custom_target(RustBuild DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/target/release/sleuthlib.lib)


add_library(${PROJECT_NAME}_interface INTERFACE)

add_dependencies(${PROJECT_NAME}_interface RustBuild)

target_link_directories(${PROJECT_NAME}_interface INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/target/release)

# Choose the appropriate MSVC runtime library based on build configuration
set(MSVC_RUNTIME_LIB "msvcrtd.lib")
if(CMAKE_BUILD_TYPE STREQUAL "Release")
    set(MSVC_RUNTIME_LIB "msvcrt.lib")
endif()

target_link_libraries(${PROJECT_NAME}_interface INTERFACE 
    ${PROJECT_NAME}

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
)