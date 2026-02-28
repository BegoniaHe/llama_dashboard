use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let llama_cpp_dir = manifest_dir.join("../../reference/llama.cpp");
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    // ── Determine build mode ──────────────────────────────────────────
    //
    // Mode A — **Prebuilt**: set `LLAMA_PREBUILT_DIR` to a directory that
    //   contains `lib/{libllama.a, libggml*.a, …}` and `include/`.
    //   CMake is skipped entirely; only linking + bindgen run.
    //   This is the recommended path for Docker / cross-compilation
    //   where llama.cpp was already compiled in a prior build stage.
    //
    // Mode B — **CMake** (default): build llama.cpp from source located
    //   in `reference/llama.cpp`.

    let (lib_dir, include_dir) = if let Ok(prebuilt) = env::var("LLAMA_PREBUILT_DIR") {
        let prebuilt = PathBuf::from(&prebuilt);
        assert!(
            prebuilt.exists(),
            "LLAMA_PREBUILT_DIR={} does not exist",
            prebuilt.display()
        );
        let lib = if prebuilt.join("lib64").exists() {
            prebuilt.join("lib64")
        } else {
            prebuilt.join("lib")
        };
        let inc = prebuilt.join("include");
        println!(
            "cargo:warning=Using prebuilt llama.cpp from {}",
            prebuilt.display()
        );
        println!("cargo:rerun-if-env-changed=LLAMA_PREBUILT_DIR");
        (lib, inc)
    } else {
        // ── CMake build ───────────────────────────────────────────────
        assert!(
            llama_cpp_dir.join("CMakeLists.txt").exists(),
            "llama.cpp source not found at {}. \
             Run `git submodule update --init --recursive`.",
            llama_cpp_dir.display()
        );

        let mut cfg = cmake::Config::new(&llama_cpp_dir);
        cfg.define("BUILD_SHARED_LIBS", "OFF")
            .define("LLAMA_BUILD_SERVER", "OFF")
            .define("LLAMA_BUILD_TESTS", "OFF")
            .define("LLAMA_BUILD_EXAMPLES", "OFF")
            .define("LLAMA_BUILD_TOOLS", "OFF")
            .define("LLAMA_BUILD_COMMON", "OFF");

        // GPU backend detection
        if env::var("CARGO_FEATURE_CUDA").is_ok() {
            cfg.define("GGML_CUDA", "ON");
        }
        if env::var("CARGO_FEATURE_VULKAN").is_ok() {
            cfg.define("GGML_VULKAN", "ON");
        }
        if env::var("CARGO_FEATURE_ROCM").is_ok() {
            let rocm = env::var("ROCM_PATH").unwrap_or_else(|_| "/opt/rocm".into());
            cfg.define("GGML_HIP", "ON")
                .define("CMAKE_HIP_COMPILER_ROCM_ROOT", &rocm)
                .define("CMAKE_HIP_FLAGS", format!("--rocm-path={rocm}"));
            if let Ok(targets) = env::var("AMDGPU_TARGETS") {
                cfg.define("AMDGPU_TARGETS", &targets);
            }
        }
        if target_os == "macos" {
            cfg.define("GGML_METAL", "ON");
        }

        let dst = cfg.build();

        let lib = if dst.join("lib64").exists() {
            dst.join("lib64")
        } else {
            dst.join("lib")
        };
        let inc = dst.join("include");
        (lib, inc)
    };

    // ── Link libraries ────────────────────────────────────────────────
    println!("cargo:rustc-link-search=native={}", lib_dir.display());

    // Core llama library
    println!("cargo:rustc-link-lib=static=llama");

    // ggml libraries — probe which ones exist
    for name in &["ggml", "ggml-base", "ggml-cpu"] {
        if lib_dir.join(format!("lib{name}.a")).exists() {
            println!("cargo:rustc-link-lib=static={name}");
        }
    }

    // GPU-specific libraries
    if env::var("CARGO_FEATURE_CUDA").is_ok() && lib_dir.join("libggml-cuda.a").exists() {
        println!("cargo:rustc-link-lib=static=ggml-cuda");
        for lib in &["cuda", "cublas", "culibos", "cudart"] {
            println!("cargo:rustc-link-lib={lib}");
        }
    }
    if env::var("CARGO_FEATURE_VULKAN").is_ok() && lib_dir.join("libggml-vulkan.a").exists() {
        println!("cargo:rustc-link-lib=static=ggml-vulkan");
        println!("cargo:rustc-link-lib=vulkan");
    }
    if env::var("CARGO_FEATURE_ROCM").is_ok() && lib_dir.join("libggml-hip.a").exists() {
        println!("cargo:rustc-link-lib=static=ggml-hip");
        let rocm = env::var("ROCM_PATH").unwrap_or_else(|_| "/opt/rocm".into());
        println!("cargo:rustc-link-search=native={rocm}/lib");
        for lib in &[
            "amdhip64",
            "hipblas",
            "hiprtc",
            "rocblas",
            "hsa-runtime64",
            "amd_comgr",
        ] {
            println!("cargo:rustc-link-lib={lib}");
        }
    }

    // Platform system libraries
    match target_os.as_str() {
        "linux" => {
            println!("cargo:rustc-link-lib=stdc++");
            println!("cargo:rustc-link-lib=m");
            println!("cargo:rustc-link-lib=pthread");
            println!("cargo:rustc-link-lib=gomp"); // OpenMP (used by ggml-cpu)
        }
        "macos" => {
            if lib_dir.join("libggml-metal.a").exists() {
                println!("cargo:rustc-link-lib=static=ggml-metal");
            }
            for fw in &["Accelerate", "Metal", "MetalKit", "Foundation"] {
                println!("cargo:rustc-link-lib=framework={fw}");
            }
            println!("cargo:rustc-link-lib=c++");
        }
        "windows" => {
            println!("cargo:rustc-link-lib=msvcrt");
        }
        _ => {}
    }

    // ── Generate Rust bindings ────────────────────────────────────────
    let mut builder = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", include_dir.display()));

    // Also search ggml include dir from source tree (may be separate)
    let ggml_include = llama_cpp_dir.join("ggml/include");
    if ggml_include.exists() {
        builder = builder.clang_arg(format!("-I{}", ggml_include.display()));
    }

    let bindings = builder
        .allowlist_function("llama_.*")
        .allowlist_function("ggml_.*")
        .allowlist_type("llama_.*")
        .allowlist_type("ggml_.*")
        .allowlist_var("LLAMA_.*")
        .allowlist_var("GGML_.*")
        .derive_default(true)
        .size_t_is_usize(true)
        .generate()
        .expect("Failed to generate bindings");

    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out.join("bindings.rs"))
        .expect("Failed to write bindings");

    println!("cargo:rerun-if-changed=wrapper.h");
}
