file(GLOB_RECURSE RUST_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs")

project(sleuthlib C)

add_custom_command(
    OUTPUT ${CMAKE_CURRENT_SOURCE_DIR}/target/release/sleuthlib.lib
    COMMAND cargo +nightly build --release
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

target_link_libraries(${PROJECT_NAME}_interface INTERFACE 
    ${PROJECT_NAME}

    # Native system libs required by Rust
    advapi32.lib
    cfgmgr32.lib
    gdi32.lib
    kernel32.lib
    msimg32.lib
    msvcrtd.lib
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
    crypt32.lib
    cryptnet.lib
    dbghelp.lib
    legacy_stdio_definitions.lib
    ncrypt.lib
)
