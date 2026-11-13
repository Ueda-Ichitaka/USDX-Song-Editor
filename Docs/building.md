# USDX Song Editor - Building from source

How to build, test and install the application on a Linux machine with KDE Plasma. The
development machine reference is Arch Linux with Qt 6.12, Kirigami 6.31 and GStreamer
1.28; newer versions should work the same way.

## Dependencies

System packages (Arch package names):

| Package | Used for |
|---|---|
| rustup (Rust 1.100 or newer) | the toolchain |
| qt6-base, qt6-declarative | Qt Quick runtime |
| kirigami, qqc2-desktop-style | the UI framework and its style |
| gstreamer, gst-plugins-base, gst-plugins-good | playback and audio tests |
| extra-cmake-modules, cmake (3.28+) | the install |
| kcoreaddons, ki18n, kconfig | ECM dependency chain |
| git | fetching the source |

Rust crates are fetched from crates.io by cargo (cxx-qt 0.10, cxx-qt-lib 0.10,
encoding_rs, chardetng, serde, toml, dirs, gstreamer-rs and more; the pinned list is in
`Plan/IMPLEMENTATION_PLAN.md` section 2.4).

## The quick way: `scripts/dev.sh`

The guarded development entry point. It checks the available memory before every run,
caps cargo's parallelism to what the machine can carry, and runs cargo inside a
memory-limited systemd scope so an out-of-memory situation kills the build and not the
desktop. Its full manual is `Docs/dev-script.1` (`man Docs/dev-script.1`).

```sh
scripts/dev.sh check            # fmt, clippy -D warnings, all tests
scripts/dev.sh build --release  # release build of the workspace
scripts/dev.sh run              # cargo run
scripts/dev.sh test usdx-format # one crate, all its tests
scripts/dev.sh corpus           # opt-in round-trip test over a real song corpus
```

`check` refuses to start below 4 GiB of available memory and reduces build jobs with the
available memory; an out-of-memory kill is reported as such, not as a test failure.

## Plain cargo

The workspace lives in `src/` with four members:

```
src/usdx-format/       lossless file model: decode, rows, headers, song view, timing, serialize, validate
src/usdx-edit/         patches, undo history, note and header operations, retiming, health, export, upgrade
src/usdx-audio/        GStreamer player, waveform peaks and vocal activity, cache
src/usdx-song-editor/  the application: cxx-qt bridges, controllers, QML, main
```

Build everything:

```sh
cargo build --workspace            # debug
cargo build --workspace --release  # release
```

The application binary is `target/<profile>/usdx-song-editor`. QML and the C++ helpers
compile inside the app crate's build script (cxx-qt code generation plus qmlcachegen), so
a plain `cargo build` is all the app needs.

## Running without a display

The self-test verifies offscreen that the QML module loads, that the window root object
is created, and - with a song path - that the song loads through the bridge end to end:

```sh
QT_QPA_PLATFORM=offscreen target/debug/usdx-song-editor --self-test
QT_QPA_PLATFORM=offscreen target/debug/usdx-song-editor --self-test song.txt
```

The second form prints `lines: <N>` and `title: <window title>` on stdout and exits
non-zero on any failure. The self-test redirects its configuration folder away from your
home, so it never touches your settings. `--version` prints the version line.

## Tests

| What | How |
|---|---|
| Unit, property and snapshot tests of every crate | `scripts/dev.sh test` or `cargo test --workspace` |
| One crate | `scripts/dev.sh test usdx-format [name-filter]` |
| Benchmark-style tests (`#[ignore]`) | `cargo test --workspace --release -- --include-ignored` |
| Real-song corpus round trip | `USDX_CORPUS=/path/to/songs scripts/dev.sh corpus` (colon-separated folders) |
| G-08 drawer probe (permanent regression test) | included in the normal test run |
| CMake install test | `scripts/tests/cmake_install_test.sh` |

The corpus test treats a `.txt` as a song only when it has a `#BPM` header, skips the
Factory helper files (`lyrics.txt`, `language.txt`, `song_end.txt`, `anchors.txt`) and
`.bak` files, and asserts the byte-identical round trip for every song. Songs are never
copied into the repository.

The audio tests generate their own media with GStreamer pipelines into temporary
directories, so they need the GStreamer codec plugins (base and good) but no fixtures.

## Install

The CMake build follows the KDE framework conventions (extra-cmake-modules,
KDEInstallDirs) and installs the binary, the desktop file, the AppStream metainfo and the
icon:

```sh
cmake -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build
cmake --install build          # add -DCMAKE_INSTALL_PREFIX=... to redirect
```

The cargo profile follows `CMAKE_BUILD_TYPE` (Release builds with `--release`), and the
binary is installed from `<binary-dir>/<profile>/`. After installing, the application
appears in Kickoff as "USDX Song Editor" with its icon and AppStream description.

`scripts/tests/cmake_install_test.sh` builds into a temporary prefix, installs there and
validates the desktop file and the metainfo; it also proves that CMake finds cargo even
when it is only in `~/.cargo/bin`.

## Troubleshooting

- **"Refusing to start: only N GiB of memory available"** - the guard needs 4 GiB.
  Close memory-heavy applications or override the meminfo source for tests with
  `DEV_MEMINFO` (see the manpage).
- **Build jobs drop to a small number** - the guard caps jobs to half the available GiB;
  on a busy shared machine this is intentional.
- **`Could not find qmlcachegen` or missing Kirigami types at runtime** - the Qt/Kirigami
  packages of the Dependencies table are incomplete (qmlcachegen ships with
  qt6-declarative).
- **Audio tests fail to build their pipelines** - install gst-plugins-base and
  gst-plugins-good; playback additionally uses the codec plugins for your song files
  (MP3 via gst-plugins-good's mpg123 decoder, Opus via base).
- **Cargo not found in a plain shell** - the binary lives in `~/.cargo/bin`; `dev.sh`
  adds it to PATH itself, CMake finds it with a `~/.cargo/bin` fallback.
