<!--
About me: the progress checklist for Plan/IMPLEMENTATION_PLAN.md. Agents mark a task here
when it meets the definition of done (plan section 0.3) and note facts a later task must
know, including the UI acceptance result (screens checked in both themes, deviations with
reasons, Plan/UI_SPEC.md section 8.2). One row per task, in plan order.
-->

# Progress

Status values: `todo`, `in progress`, `done`, `blocked (gate G-xx)`.

| Task | Title | Status | Notes for later tasks |
|---|---|---|---|
| P0-01 | Workspace | done | Virtual workspace defaults to resolver 1 even with edition 2024 members; `resolver = "3"` set explicitly (verified via cargo warning). The `usdx-edit -> usdx-format` dependency edge is added in P2-01 when first used (unused dep warns). |
| P0-02 | Guarded dev script | done | `dev.sh check/test/build` work through the systemd scope (verified live). `run` fails cleanly until the app bin exists (P0-03); `corpus` fails cleanly until the corpus test exists (P1-09). `cargo run` accepts no `--workspace` flag - run uses `cargo run`. Guard test also covers the 4 GiB boundary and the jobs floor of 1. |
| P0-03 | App skeleton | done | Crate has a lib target (`src/lib.rs` holds `app_main`, `main.rs` is thin) in addition to the plan's file list: rustc 1.100 bundles the cxx-qt generated C++ archives into each rlib, and cargo applies the package's own build-script link flags to every target, so an integration test that references no library code misses the dependency rlibs and fails with undefined `cxx_qt_init_crate_*`. The test references `version_line()` from the lib, which links the full graph. Verified on 0.10 (KDE guide is 0.8.1): `CxxQtBuilder::new_qml_module(QmlModule::new(uri).qml_files([...]))`, plain `#[cxx::bridge]` files go through `.files()`, own C++ via `.cpp_file()` + `.cc_builder()` include, `QApplication::new()` in cxx-qt-lib-extras, `QGuiApplication::set_desktop_file_name` static on the type (import from `cxx_qt_lib`), `QQuickStyle::set_style` (part of `qt_full`), `QQmlApplicationEngine::new()` + `load(Pin<&mut>, &QUrl)`, `QUrl: From<&str>`; `rootObjects()` is NOT bound - checked via own bridge `src/bridge/selftest.rs` binding `cpp/selftest.cpp`. QML module URI `io.github.ueda_ichitaka.UsdxSongEditor` accepted by QmlUri (alphanumeric + `_` per segment) and processed by qmlcachegen; no gate G-04. `QT_QUICK_CONTROLS_STYLE` env overrides the hard-coded `org.kde.desktop`. Manual `--self-test` offscreen exits 0. |
| P0-04 | CMake install and desktop integration | todo | |
| P1-01 | Byte layer | todo | |
| P1-02 | Row classification | todo | |
| P1-03 | Headers | todo | |
| P1-04 | Timing | todo | |
| P1-05 | Song view | todo | |
| P1-06 | Relative mode, reading | todo | gate G-01 |
| P1-07 | Serializer | todo | |
| P1-08 | Validation | todo | |
| P1-09 | Corpus test | todo | |
| P2-01 | Patch | todo | |
| P2-02 | History | todo | |
| P2-03 | Note operations | todo | |
| P2-04 | Header operations | todo | |
| P2-05 | Change log for export | todo | |
| P3-01 | EditorController: load and navigate | todo | |
| P3-02 | Save service | todo | |
| P3-03 | Settings | todo | |
| P3-04 | SongDocument and LinesModel bridges | todo | |
| P3-05 | QML shell | todo | gate G-08 check first |
| P3-06 | Screenshot harness | todo | |
| P4-01 | Test media helper | todo | |
| P4-02 | Player | todo | |
| P4-03 | Analysis and cache | todo | |
| P4-04 | Player bridge and playhead | todo | |
| P5-01 | SheetViewport | todo | |
| P5-02 | SheetBackground | todo | |
| P5-03 | NotesModel and PhraseEndsModel | todo | |
| P5-04 | Note delegates: select, drag, resize, rubber band | todo | |
| P5-05 | Phrase end handle, insert, split, merge, delete, type keys, inspector | todo | |
| P5-06 | Keyboard navigation and autoplay | todo | |
| P6-01 | Syllable parser and plan | todo | |
| P6-02 | Inline syllable edit and LineTextDialog | todo | |
| P7-01 | Collision pass | todo | |
| P7-02 | Anchor + stretch | todo | |
| P7-03 | Block transform | todo | |
| P7-04 | Tap mapping | todo | |
| P7-05 | Health analysis | todo | |
| P7-06 | Anchors export | todo | |
| P7-07 | RepairController and TapController | todo | |
| P7-08 | Repair UI | todo | |
| P7-09 | Line health in the lines pane and editor status | todo | |
| P7-10 | Threshold tuning | todo | gate G-06 |
| P8-01 | Upgrade planner | todo | |
| P8-02 | PropertiesPage, validation list, UpgradeDialog | todo | gate G-05 |
| P8-03 | Settings page | todo | |
| P8-04 | Packaging | todo | |
| P8-05 | Documentation | todo | |
