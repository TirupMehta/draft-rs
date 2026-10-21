# Third-party notices

Draft is distributed with the third-party components below. The list is generated from `Cargo.toml` and `Cargo.lock` using:

```text
cargo tree --workspace --edges normal --format "{p}|{l}" --prefix none
```

The license column is the license expression reported by each crate. The full text of each license is distributed with the crate itself. Platform-specific crates for non-Windows targets are included because they are part of the dependency graph.

| Crate | Version | License |
| --- | --- | --- |
| `adler2` | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| `aho-corasick` | 1.1.5 | Unlicense OR MIT |
| `aligned` | 0.4.3 | MIT OR Apache-2.0 |
| `aligned-vec` | 0.6.4 | MIT |
| `anyhow` | 1.0.104 | MIT OR Apache-2.0 |
| `arboard` | 3.7.0 | MIT OR Apache-2.0 |
| `arrayref` | 0.3.9 | BSD-2-Clause |
| `arrayvec` | 0.7.8 | MIT OR Apache-2.0 |
| `as-slice` | 0.2.1 | MIT OR Apache-2.0 |
| `async_zip` | 0.0.17 | MIT |
| `async-channel` | 1.9.0 | Apache-2.0 OR MIT |
| `async-channel` | 2.5.0 | Apache-2.0 OR MIT |
| `async-compression` | 0.4.51 | MIT OR Apache-2.0 |
| `async-executor` | 1.14.0 | Apache-2.0 OR MIT |
| `async-fs` | 2.2.0 | Apache-2.0 OR MIT |
| `async-global-executor` | 2.4.1 | Apache-2.0 OR MIT |
| `async-io` | 2.6.0 | Apache-2.0 OR MIT |
| `async-lock` | 3.4.2 | Apache-2.0 OR MIT |
| `async-net` | 2.0.0 | Apache-2.0 OR MIT |
| `async-process` | 2.5.0 | Apache-2.0 OR MIT |
| `async-std` | 1.13.2 | Apache-2.0 OR MIT |
| `async-task` | 4.7.1 | Apache-2.0 OR MIT |
| `atomic` | 0.5.3 | Apache-2.0/MIT |
| `atomic-waker` | 1.1.2 | Apache-2.0 OR MIT |
| `av1-grain` | 0.2.5 | BSD-2-Clause |
| `avif-serialize` | 0.8.9 | BSD-3-Clause |
| `av-scenechange` | 0.14.1 | MIT |
| `backtrace` | 0.3.76 | MIT OR Apache-2.0 |
| `base64` | 0.22.1 | MIT OR Apache-2.0 |
| `bit_field` | 0.10.3 | Apache-2.0/MIT |
| `bitflags` | 1.3.2 | MIT/Apache-2.0 |
| `bitflags` | 2.13.2 | MIT OR Apache-2.0 |
| `bitstream-io` | 4.10.0 | MIT/Apache-2.0 |
| `block-buffer` | 0.10.4 | MIT OR Apache-2.0 |
| `block-buffer` | 0.12.1 | MIT OR Apache-2.0 |
| `blocking` | 1.7.0 | Apache-2.0 OR MIT |
| `bstr` | 1.13.1 | MIT OR Apache-2.0 |
| `bytemuck` | 1.25.2 | Zlib OR Apache-2.0 OR MIT |
| `byteorder-lite` | 0.1.0 | Unlicense OR MIT |
| `bytes` | 1.12.1 | MIT |
| `cfg-if` | 1.0.5 | MIT OR Apache-2.0 |
| `clipboard-win` | 5.4.1 | BSL-1.0 |
| `color_quant` | 1.1.0 | MIT |
| `color-eyre` | 0.6.5 | MIT OR Apache-2.0 |
| `color-spantrace` | 0.3.0 | MIT OR Apache-2.0 |
| `compression-codecs` | 0.4.46 | MIT OR Apache-2.0 |
| `compression-core` | 0.4.34 | MIT OR Apache-2.0 |
| `concurrent-queue` | 2.5.0 | Apache-2.0 OR MIT |
| `const-oid` | 0.10.2 | Apache-2.0 OR MIT |
| `convert_case` | 0.4.0 | MIT |
| `core_maths` | 0.1.1 | MIT |
| `cpal` | 0.16.0 | Apache-2.0 |
| `cpufeatures` | 0.2.17 | MIT OR Apache-2.0 |
| `cpufeatures` | 0.3.1 | MIT OR Apache-2.0 |
| `crc32fast` | 1.5.2 | MIT OR Apache-2.0 |
| `crossbeam-channel` | 0.5.17 | MIT OR Apache-2.0 |
| `crossbeam-deque` | 0.8.8 | MIT OR Apache-2.0 |
| `crossbeam-epoch` | 0.9.21 | MIT OR Apache-2.0 |
| `crossbeam-queue` | 0.3.14 | MIT OR Apache-2.0 |
| `crossbeam-utils` | 0.8.23 | MIT OR Apache-2.0 |
| `crypto-common` | 0.1.7 | MIT OR Apache-2.0 |
| `crypto-common` | 0.2.2 | MIT OR Apache-2.0 |
| `ctor` | 0.4.3 | Apache-2.0 OR MIT |
| `dasp_sample` | 0.11.0 | MIT OR Apache-2.0 |
| `data-url` | 0.3.2 | MIT OR Apache-2.0 |
| `deflate64` | 0.1.12 | MIT |
| `deranged` | 0.5.9 | MIT OR Apache-2.0 |
| `digest` | 0.10.7 | MIT OR Apache-2.0 |
| `digest` | 0.11.3 | MIT OR Apache-2.0 |
| `dirs` | 4.0.0 | MIT OR Apache-2.0 |
| `dirs` | 6.0.0 | MIT OR Apache-2.0 |
| `dirs-sys` | 0.3.7 | MIT OR Apache-2.0 |
| `dirs-sys` | 0.5.0 | MIT OR Apache-2.0 |
| `dpi` | 0.1.2 | Apache-2.0 AND MIT |
| `dtor` | 0.0.6 | Apache-2.0 OR MIT |
| `dunce` | 1.0.5 | CC0-1.0 OR MIT-0 OR Apache-2.0 |
| `dyn-clone` | 1.0.20 | MIT OR Apache-2.0 |
| `either` | 1.19.0 | MIT OR Apache-2.0 |
| `encoding_rs` | 0.8.35 | (Apache-2.0 OR MIT) AND BSD-3-Clause |
| `equator` | 0.4.2 | MIT |
| `equivalent` | 1.0.3 | Apache-2.0 OR MIT |
| `erased-serde` | 0.4.10 | MIT OR Apache-2.0 |
| `errno` | 0.3.14 | MIT OR Apache-2.0 |
| `error-code` | 3.4.0 | BSL-1.0 |
| `etagere` | 0.2.15 | MIT/Apache-2.0 |
| `euclid` | 0.22.14 | MIT OR Apache-2.0 |
| `event-listener` | 2.5.3 | Apache-2.0 OR MIT |
| `event-listener` | 5.4.2 | Apache-2.0 OR MIT |
| `event-listener-strategy` | 0.5.4 | Apache-2.0 OR MIT |
| `exr` | 1.74.2 | BSD-3-Clause |
| `eyre` | 0.6.14 | MIT OR Apache-2.0 |
| `fastrand` | 1.9.0 | Apache-2.0 OR MIT |
| `fastrand` | 2.5.0 | Apache-2.0 OR MIT |
| `fax` | 0.2.7 | MIT |
| `fdeflate` | 0.3.7 | MIT OR Apache-2.0 |
| `filetime` | 0.2.29 | MIT/Apache-2.0 |
| `flate2` | 1.1.10 | MIT OR Apache-2.0 |
| `float_next_after` | 1.0.0 | MIT |
| `float-cmp` | 0.9.0 | MIT |
| `flume` | 0.11.1 | Apache-2.0/MIT |
| `fnv` | 1.0.7 | Apache-2.0 / MIT |
| `foldhash` | 0.2.0 | Zlib |
| `fontdb` | 0.23.0 | MIT |
| `form_urlencoded` | 1.2.2 | MIT OR Apache-2.0 |
| `futf` | 0.1.5 | MIT / Apache-2.0 |
| `futures` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-channel` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-core` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-executor` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-io` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-lite` | 1.13.0 | Apache-2.0 OR MIT |
| `futures-lite` | 2.6.1 | Apache-2.0 OR MIT |
| `futures-sink` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-task` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-util` | 0.3.34 | MIT OR Apache-2.0 |
| `generic-array` | 0.14.7 | MIT |
| `getrandom` | 0.2.17 | MIT OR Apache-2.0 |
| `getrandom` | 0.3.4 | MIT OR Apache-2.0 |
| `getrandom` | 0.4.3 | MIT OR Apache-2.0 |
| `gif` | 0.14.2 | MIT OR Apache-2.0 |
| `globset` | 0.4.19 | Unlicense OR MIT |
| `gpui` | 0.2.2 | Apache-2.0 |
| `gpui_collections` | 0.2.2 | Apache-2.0 |
| `gpui_http_client` | 0.2.2 | Apache-2.0 |
| `gpui_perf` | 0.2.2 | Apache-2.0 |
| `gpui_refineable` | 0.2.2 | Apache-2.0 |
| `gpui_semantic_version` | 0.2.2 | Apache-2.0 |
| `gpui_sum_tree` | 0.2.2 | Apache-2.0 |
| `gpui_util` | 0.2.2 | Apache-2.0 |
| `grid` | 0.18.0 | MIT |
| `h2` | 0.4.20 | MIT |
| `half` | 2.7.1 | MIT OR Apache-2.0 |
| `hashbrown` | 0.17.1 | MIT OR Apache-2.0 |
| `heck` | 0.5.0 | MIT OR Apache-2.0 |
| `home` | 0.5.11 | MIT OR Apache-2.0 |
| `hound` | 3.5.1 | Apache-2.0 |
| `http` | 1.5.0 | MIT OR Apache-2.0 |
| `httparse` | 1.10.1 | MIT OR Apache-2.0 |
| `http-body` | 1.1.0 | MIT |
| `http-body-util` | 0.1.5 | MIT |
| `hybrid-array` | 0.4.15 | MIT OR Apache-2.0 |
| `hyper` | 1.12.0 | MIT |
| `hyper-rustls` | 0.27.10 | Apache-2.0 OR ISC OR MIT |
| `hyper-util` | 0.1.21 | MIT |
| `icu_collections` | 2.1.1 | Unicode-3.0 |
| `icu_locale_core` | 2.1.1 | Unicode-3.0 |
| `icu_normalizer` | 2.1.1 | Unicode-3.0 |
| `icu_normalizer_data` | 2.1.1 | Unicode-3.0 |
| `icu_properties` | 2.1.2 | Unicode-3.0 |
| `icu_properties_data` | 2.1.2 | Unicode-3.0 |
| `icu_provider` | 2.1.1 | Unicode-3.0 |
| `idna` | 1.1.0 | MIT OR Apache-2.0 |
| `idna_adapter` | 1.2.1 | Apache-2.0 OR MIT |
| `image` | 0.25.9 | MIT OR Apache-2.0 |
| `imagesize` | 0.13.0 | MIT |
| `image-webp` | 0.2.4 | MIT OR Apache-2.0 |
| `imgref` | 1.12.3 | CC0-1.0 OR Apache-2.0 |
| `indenter` | 0.3.4 | MIT OR Apache-2.0 |
| `indexmap` | 2.14.2 | Apache-2.0 OR MIT |
| `inventory` | 0.3.25 | MIT OR Apache-2.0 |
| `ipnet` | 2.12.2 | MIT OR Apache-2.0 |
| `itertools` | 0.14.0 | MIT OR Apache-2.0 |
| `itoa` | 1.0.18 | MIT OR Apache-2.0 |
| `keyboard-types` | 0.7.0 | MIT OR Apache-2.0 |
| `kurbo` | 0.11.3 | Apache-2.0 OR MIT |
| `kv-log-macro` | 1.0.7 | MIT OR Apache-2.0 |
| `lazy_static` | 1.5.1 | MIT OR Apache-2.0 |
| `lebe` | 0.5.3 | BSD-3-Clause |
| `libc` | 0.2.191 | MIT OR Apache-2.0 |
| `libm` | 0.2.16 | MIT |
| `litemap` | 0.8.3 | Unicode-3.0 |
| `lock_api` | 0.4.14 | MIT OR Apache-2.0 |
| `log` | 0.4.34 | MIT OR Apache-2.0 |
| `loop9` | 0.1.5 | MIT |
| `lyon` | 1.0.19 | MIT OR Apache-2.0 |
| `lyon_algorithms` | 1.0.22 | MIT OR Apache-2.0 |
| `lyon_geom` | 1.0.20 | MIT OR Apache-2.0 |
| `lyon_path` | 1.0.19 | MIT OR Apache-2.0 |
| `lyon_tessellation` | 1.0.23 | MIT OR Apache-2.0 |
| `mac` | 0.1.1 | MIT/Apache-2.0 |
| `matchers` | 0.2.0 | MIT |
| `maybe-rayon` | 0.1.1 | MIT |
| `memchr` | 2.8.3 | Unlicense OR MIT |
| `memmap2` | 0.9.11 | MIT OR Apache-2.0 |
| `mime` | 0.3.17 | MIT OR Apache-2.0 |
| `mime_guess` | 2.0.5 | MIT |
| `miniz_oxide` | 0.8.9 | MIT OR Zlib OR Apache-2.0 |
| `miniz_oxide` | 0.9.1 | MIT OR Zlib OR Apache-2.0 |
| `mio` | 1.2.4 | MIT |
| `moxcms` | 0.7.11 | BSD-3-Clause OR Apache-2.0 |
| `muda` | 0.19.3 | Apache-2.0 OR MIT |
| `nanorand` | 0.7.0 | Zlib |
| `new_debug_unreachable` | 1.0.6 | MIT |
| `no_std_io2` | 0.9.4 | Apache-2.0 OR MIT |
| `nom` | 8.0.0 | MIT |
| `nu-ansi-term` | 0.50.3 | MIT |
| `num_cpus` | 1.17.0 | MIT OR Apache-2.0 |
| `num-bigint` | 0.4.8 | MIT OR Apache-2.0 |
| `num-complex` | 0.4.6 | MIT OR Apache-2.0 |
| `num-conv` | 0.1.0 | MIT OR Apache-2.0 |
| `num-integer` | 0.1.47 | MIT OR Apache-2.0 |
| `num-rational` | 0.4.2 | MIT OR Apache-2.0 |
| `num-traits` | 0.2.19 | MIT OR Apache-2.0 |
| `once_cell` | 1.21.4 | MIT OR Apache-2.0 |
| `option-ext` | 0.2.0 | MPL-2.0 |
| `owo-colors` | 4.4.0 | MIT |
| `parking` | 2.2.1 | Apache-2.0 OR MIT |
| `parking_lot` | 0.12.5 | MIT OR Apache-2.0 |
| `parking_lot_core` | 0.9.12 | MIT OR Apache-2.0 |
| `percent-encoding` | 2.3.2 | MIT OR Apache-2.0 |
| `pico-args` | 0.5.0 | MIT |
| `pin-project` | 1.1.13 | Apache-2.0 OR MIT |
| `pin-project-lite` | 0.2.17 | Apache-2.0 OR MIT |
| `pin-utils` | 0.1.1 | MIT OR Apache-2.0 |
| `piper` | 0.2.5 | MIT OR Apache-2.0 |
| `png` | 0.17.16 | MIT OR Apache-2.0 |
| `png` | 0.18.1 | MIT OR Apache-2.0 |
| `polling` | 3.11.0 | Apache-2.0 OR MIT |
| `pollster` | 0.2.5 | Apache-2.0/MIT |
| `postage` | 0.5.0 | MIT |
| `potential_utf` | 0.1.6 | Unicode-3.0 |
| `powerfmt` | 0.2.1 | MIT OR Apache-2.0 |
| `ppv-lite86` | 0.2.21 | MIT OR Apache-2.0 |
| `primal-check` | 0.3.4 | MIT OR Apache-2.0 |
| `proc-macro2` | 1.0.107 | MIT OR Apache-2.0 |
| `proc-macro-error2` | 2.0.1 | MIT OR Apache-2.0 |
| `profiling` | 1.0.18 | MIT OR Apache-2.0 |
| `psm` | 0.1.31 | MIT OR Apache-2.0 |
| `pulp` | 0.22.3 | MIT |
| `pulp-wasm-simd-flag` | 0.1.1 | MIT |
| `pxfm` | 0.1.30 | BSD-3-Clause OR Apache-2.0 |
| `qoi` | 0.4.1 | MIT/Apache-2.0 |
| `quick-error` | 2.0.1 | MIT/Apache-2.0 |
| `quote` | 1.0.47 | MIT OR Apache-2.0 |
| `rand` | 0.9.5 | MIT OR Apache-2.0 |
| `rand_chacha` | 0.9.0 | MIT OR Apache-2.0 |
| `rand_core` | 0.9.5 | MIT OR Apache-2.0 |
| `rav1e` | 0.8.1 | BSD-2-Clause |
| `ravif` | 0.12.0 | BSD-3-Clause |
| `raw-cpuid` | 11.6.0 | MIT |
| `raw-window-handle` | 0.6.2 | MIT OR Apache-2.0 OR Zlib |
| `rayon` | 1.12.0 | MIT OR Apache-2.0 |
| `rayon-core` | 1.13.0 | MIT OR Apache-2.0 |
| `realfft` | 3.5.0 | MIT |
| `reborrow` | 0.5.5 | MIT |
| `ref-cast` | 1.0.27 | MIT OR Apache-2.0 |
| `regex` | 1.13.1 | MIT OR Apache-2.0 |
| `regex-automata` | 0.4.18 | MIT OR Apache-2.0 |
| `regex-syntax` | 0.8.11 | MIT OR Apache-2.0 |
| `resvg` | 0.45.1 | Apache-2.0 OR MIT |
| `rgb` | 0.8.53 | MIT |
| `ring` | 0.17.14 | Apache-2.0 AND ISC |
| `roxmltree` | 0.20.0 | MIT OR Apache-2.0 |
| `rubato` | 0.16.2 | MIT |
| `rustc-demangle` | 0.1.28 | MIT/Apache-2.0 |
| `rustc-hash` | 2.1.3 | Apache-2.0 OR MIT |
| `rust-embed` | 8.13.0 | MIT |
| `rust-embed-utils` | 8.13.0 | MIT |
| `rustfft` | 6.4.1 | MIT OR Apache-2.0 |
| `rustix` | 1.1.5 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| `rustls` | 0.23.46 | Apache-2.0 OR ISC OR MIT |
| `rustls-native-certs` | 0.8.5 | Apache-2.0 OR ISC OR MIT |
| `rustls-pemfile` | 2.2.0 | Apache-2.0 OR ISC OR MIT |
| `rustls-pki-types` | 1.15.1 | MIT OR Apache-2.0 |
| `rustls-webpki` | 0.103.15 | ISC |
| `rustybuzz` | 0.20.1 | MIT |
| `ryu` | 1.0.23 | Apache-2.0 OR BSL-1.0 |
| `same-file` | 1.0.6 | Unlicense/MIT |
| `schannel` | 0.1.30 | MIT |
| `schemars` | 1.2.3 | MIT |
| `scopeguard` | 1.2.0 | MIT OR Apache-2.0 |
| `seahash` | 4.1.0 | MIT |
| `serde` | 1.0.229 | MIT OR Apache-2.0 |
| `serde_core` | 1.0.229 | MIT OR Apache-2.0 |
| `serde_derive_internals` | 0.30.0 | MIT OR Apache-2.0 |
| `serde_fmt` | 1.1.0 | Apache-2.0 OR MIT |
| `serde_json` | 1.0.154 | MIT OR Apache-2.0 |
| `serde_json_lenient` | 0.2.4 | MIT/Apache-2.0 |
| `serde_urlencoded` | 0.7.1 | MIT/Apache-2.0 |
| `sha1_smol` | 1.0.1 | BSD-3-Clause |
| `sha2` | 0.10.9 | MIT OR Apache-2.0 |
| `sha2` | 0.11.0 | MIT OR Apache-2.0 |
| `sharded-slab` | 0.1.7 | MIT |
| `shlex` | 1.3.0 | MIT OR Apache-2.0 |
| `simd-adler32` | 0.3.10 | MIT |
| `simplecss` | 0.2.2 | Apache-2.0 OR MIT |
| `siphasher` | 1.0.4 | MIT OR Apache-2.0 |
| `slab` | 0.4.12 | MIT |
| `slotmap` | 1.1.1 | Zlib |
| `smallvec` | 1.16.3 | MIT OR Apache-2.0 |
| `smol` | 2.0.2 | Apache-2.0 OR MIT |
| `socket2` | 0.6.5 | MIT OR Apache-2.0 |
| `spin` | 0.9.9 | MIT |
| `stable_deref_trait` | 1.2.1 | MIT OR Apache-2.0 |
| `stacker` | 0.1.25 | MIT OR Apache-2.0 |
| `stacksafe` | 0.1.4 | Apache-2.0 |
| `static_assertions` | 1.1.0 | MIT OR Apache-2.0 |
| `strength_reduce` | 0.2.4 | MIT OR Apache-2.0 |
| `strict-num` | 0.1.1 | MIT |
| `strum` | 0.27.2 | MIT |
| `subtle` | 2.6.1 | BSD-3-Clause |
| `svg_fmt` | 0.4.5 | MIT/Apache-2.0 |
| `svgtypes` | 0.15.3 | Apache-2.0 OR MIT |
| `symlink` | 0.1.0 | MIT/Apache-2.0 |
| `syn` | 2.0.119 | MIT OR Apache-2.0 |
| `syn` | 3.0.9 | MIT OR Apache-2.0 |
| `sync_wrapper` | 1.0.2 | Apache-2.0 |
| `synstructure` | 0.14.0 | MIT |
| `taffy` | 0.9.0 | MIT |
| `take-until` | 0.2.0 | MIT |
| `tempfile` | 3.27.0 | MIT OR Apache-2.0 |
| `tendril` | 0.4.3 | MIT/Apache-2.0 |
| `thiserror` | 1.0.69 | MIT OR Apache-2.0 |
| `thiserror` | 2.0.21 | MIT OR Apache-2.0 |
| `thread_local` | 1.1.10 | MIT OR Apache-2.0 |
| `tiff` | 0.10.3 | MIT |
| `time` | 0.3.45 | MIT OR Apache-2.0 |
| `time-core` | 0.1.7 | MIT OR Apache-2.0 |
| `tiny-skia` | 0.11.4 | BSD-3-Clause |
| `tiny-skia-path` | 0.11.4 | BSD-3-Clause |
| `tinystr` | 0.8.4 | Unicode-3.0 |
| `tinyvec` | 1.13.3 | Zlib OR Apache-2.0 OR MIT |
| `tokio` | 1.54.0 | MIT |
| `tokio-rustls` | 0.26.6 | MIT OR Apache-2.0 |
| `tokio-socks` | 0.5.3 | MIT |
| `tokio-util` | 0.7.20 | MIT |
| `tower` | 0.5.3 | MIT |
| `tower-layer` | 0.3.3 | MIT |
| `tower-service` | 0.3.3 | MIT |
| `tracing` | 0.1.44 | MIT |
| `tracing-appender` | 0.2.5 | MIT |
| `tracing-core` | 0.1.36 | MIT |
| `tracing-error` | 0.2.1 | MIT |
| `tracing-log` | 0.2.0 | MIT |
| `tracing-subscriber` | 0.3.23 | MIT |
| `transcribe-cpp` | 0.4.0 | MIT |
| `transcribe-cpp-sys` | 0.4.0 | MIT |
| `transpose` | 0.2.3 | MIT OR Apache-2.0 |
| `tray-icon` | 0.24.2 | MIT OR Apache-2.0 |
| `try-lock` | 0.2.5 | MIT |
| `ttf-parser` | 0.25.1 | MIT OR Apache-2.0 |
| `typeid` | 1.0.3 | MIT OR Apache-2.0 |
| `typenum` | 1.20.1 | MIT OR Apache-2.0 |
| `unicase` | 2.10.0 | MIT OR Apache-2.0 |
| `unicode-bidi` | 0.3.18 | MIT OR Apache-2.0 |
| `unicode-bidi-mirroring` | 0.4.0 | MIT/Apache-2.0 |
| `unicode-ccc` | 0.4.0 | MIT/Apache-2.0 |
| `unicode-ident` | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| `unicode-properties` | 0.1.4 | MIT/Apache-2.0 |
| `unicode-script` | 0.5.8 | MIT OR Apache-2.0 |
| `unicode-segmentation` | 1.13.3 | MIT OR Apache-2.0 |
| `unicode-vo` | 0.1.0 | MIT/Apache-2.0 |
| `untrusted` | 0.9.0 | ISC |
| `ureq` | 2.12.1 | MIT OR Apache-2.0 |
| `url` | 2.5.8 | MIT OR Apache-2.0 |
| `usvg` | 0.45.1 | Apache-2.0 OR MIT |
| `utf-8` | 0.7.6 | MIT OR Apache-2.0 |
| `utf8_iter` | 1.0.4 | Apache-2.0 OR MIT |
| `uuid` | 1.26.1 | Apache-2.0 OR MIT |
| `v_frame` | 0.3.9 | BSD-2-Clause |
| `value-bag` | 1.14.1 | Apache-2.0 OR MIT |
| `value-bag-serde1` | 1.14.1 | Apache-2.0 OR MIT |
| `waker-fn` | 1.2.0 | Apache-2.0 OR MIT |
| `walkdir` | 2.5.0 | Unlicense/MIT |
| `want` | 0.3.2 | MIT |
| `webpki-roots` | 0.26.11 | CDLA-Permissive-2.0 |
| `webpki-roots` | 1.0.10 | CDLA-Permissive-2.0 |
| `weezl` | 0.1.12 | MIT OR Apache-2.0 |
| `which` | 6.0.3 | MIT |
| `winapi` | 0.3.9 | MIT/Apache-2.0 |
| `winapi-util` | 0.1.11 | Unlicense OR MIT |
| `windows` | 0.54.0 | MIT OR Apache-2.0 |
| `windows` | 0.61.3 | MIT OR Apache-2.0 |
| `windows_x86_64_msvc` | 0.52.6 | MIT OR Apache-2.0 |
| `windows_x86_64_msvc` | 0.53.1 | MIT OR Apache-2.0 |
| `windows-collections` | 0.2.0 | MIT OR Apache-2.0 |
| `windows-core` | 0.54.0 | MIT OR Apache-2.0 |
| `windows-core` | 0.61.2 | MIT OR Apache-2.0 |
| `windows-future` | 0.2.1 | MIT OR Apache-2.0 |
| `windows-link` | 0.1.3 | MIT OR Apache-2.0 |
| `windows-link` | 0.2.1 | MIT OR Apache-2.0 |
| `windows-numerics` | 0.2.0 | MIT OR Apache-2.0 |
| `windows-registry` | 0.4.0 | MIT OR Apache-2.0 |
| `windows-registry` | 0.5.3 | MIT OR Apache-2.0 |
| `windows-result` | 0.1.2 | MIT OR Apache-2.0 |
| `windows-result` | 0.3.4 | MIT OR Apache-2.0 |
| `windows-strings` | 0.3.1 | MIT OR Apache-2.0 |
| `windows-strings` | 0.4.2 | MIT OR Apache-2.0 |
| `windows-sys` | 0.59.0 | MIT OR Apache-2.0 |
| `windows-sys` | 0.61.2 | MIT OR Apache-2.0 |
| `windows-targets` | 0.52.6 | MIT OR Apache-2.0 |
| `windows-targets` | 0.53.5 | MIT OR Apache-2.0 |
| `windows-threading` | 0.1.0 | MIT OR Apache-2.0 |
| `winsafe` | 0.0.19 | MIT |
| `writeable` | 0.6.4 | Unicode-3.0 |
| `xmlwriter` | 0.1.0 | MIT |
| `y4m` | 0.8.0 | MIT |
| `yoke` | 0.8.3 | Unicode-3.0 |
| `zed-async-tar` | 0.5.0-zed | MIT/Apache-2.0 |
| `zed-reqwest` | 0.12.15-zed | MIT OR Apache-2.0 |
| `zerocopy` | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT |
| `zerofrom` | 0.1.8 | Unicode-3.0 |
| `zeroize` | 1.9.1 | Apache-2.0 OR MIT |
| `zerotrie` | 0.2.5 | Unicode-3.0 |
| `zerovec` | 0.11.8 | Unicode-3.0 |
| `zmij` | 1.0.23 | MIT |
| `zune-core` | 0.4.12 | MIT OR Apache-2.0 OR Zlib |
| `zune-core` | 0.5.3 | MIT OR Apache-2.0 OR Zlib |
| `zune-inflate` | 0.2.54 | MIT OR Apache-2.0 OR Zlib |
| `zune-jpeg` | 0.4.21 | MIT OR Apache-2.0 OR Zlib |
| `zune-jpeg` | 0.5.15 | MIT OR Apache-2.0 OR Zlib |

## Bundled typefaces

- **Inter** by Rasmus Andersson — SIL Open Font License 1.1. See `assets/fonts/Inter-OFL.txt`.
- **JetBrains Mono** by JetBrains — SIL Open Font License 1.1. See `assets/fonts/JetBrainsMono-OFL.txt`.

## Speech model

The speech model is **Whisper large-v3-turbo** by OpenAI, distributed as a GGUF build by handy-computer. It is distributed with Draft and is subject to its own terms. See <https://huggingface.co/handy-computer/whisper-large-v3-turbo-gguf>.

## Project license

Draft itself is released under the MIT License. See `LICENSE`.
