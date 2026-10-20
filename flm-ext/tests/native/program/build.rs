//! Links the program's WebAssembly module with a stack of 64 MiB, flm-core's
//! `WORKER_STACK_SIZE` (`flm_core::lang::guard`): the export `build_registry` marks its thread
//! as a worker thread, since a program built for `wasm32-unknown-unknown` cannot start one,
//! and flm-core's parse then runs on the module's own stack.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        println!("cargo:rustc-link-arg-cdylib=-zstack-size=67108864");
    }
}
