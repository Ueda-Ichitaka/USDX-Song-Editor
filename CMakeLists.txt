# About me: top-level CMake project of the USDX Song Editor. It builds the Rust
# workspace through cargo (cargo has no install support for GUI desktop
# applications, CMake does, plan section 3) and installs the binary, the desktop
# file, the AppStream metainfo and the icon with the KDE install directories.
# The build follows the KDE guide for Kirigami with Rust, extended for the
# release profile mapping and the packaging files that the plan requires.

cmake_minimum_required(VERSION 3.28)
project(usdx-song-editor VERSION 0.1.0 LANGUAGES)

find_package(ECM 6.0 REQUIRED NO_MODULE)
set(CMAKE_MODULE_PATH ${ECM_MODULE_PATH})

include(KDEInstallDirs)
include(ECMUninstallTarget)
include(ECMFindQmlModule)
ecm_find_qmlmodule(org.kde.kirigami REQUIRED)
find_package(KF6 REQUIRED COMPONENTS QQC2DesktopStyle)

# The cargo profile follows the CMake build type: Release builds through CMake
# produce the optimized binary (the packaging in P8-04 builds this way), anything
# else produces the plain debug build.
set(USDX_CARGO_PROFILE debug)
set(USDX_CARGO_FLAGS "")
if(CMAKE_BUILD_TYPE STREQUAL "Release")
  set(USDX_CARGO_PROFILE release)
  set(USDX_CARGO_FLAGS --release)
endif()

# Cargo is not on every shell's PATH (on this machine it lives in ~/.cargo/bin),
# so the build target uses the resolved absolute path.
find_program(USDX_CARGO_EXECUTABLE cargo PATHS "$ENV{HOME}/.cargo/bin")
if(NOT USDX_CARGO_EXECUTABLE)
  message(FATAL_ERROR "cargo was not found on PATH or in ~/.cargo/bin; install Rust (https://rustup.rs)")
endif()

add_custom_target(usdx-song-editor ALL
  COMMAND ${USDX_CARGO_EXECUTABLE} build ${USDX_CARGO_FLAGS} --target-dir ${CMAKE_CURRENT_BINARY_DIR}
  WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
  COMMENT "Building the Rust workspace with cargo (${USDX_CARGO_PROFILE})"
  VERBATIM
)

install(PROGRAMS ${CMAKE_CURRENT_BINARY_DIR}/${USDX_CARGO_PROFILE}/usdx-song-editor
  DESTINATION ${KDE_INSTALL_BINDIR}
)
install(FILES packaging/io.github.ueda_ichitaka.UsdxSongEditor.desktop
  DESTINATION ${KDE_INSTALL_APPDIR}
)
install(FILES packaging/io.github.ueda_ichitaka.UsdxSongEditor.metainfo.xml
  DESTINATION ${KDE_INSTALL_METAINFODIR}
)
install(FILES packaging/io.github.ueda_ichitaka.UsdxSongEditor.svg
  DESTINATION ${KDE_INSTALL_ICONDIR}/hicolor/scalable/apps
)
