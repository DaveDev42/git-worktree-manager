fn main() {
    // Windows' default main-thread stack is 1 MiB, which the debug-build
    // clap-derive parser for our 55+ command variants can blow through
    // (STATUS_STACK_OVERFLOW). Run on a thread with an 8 MiB stack to match
    // Linux/macOS behavior.
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(git_worktree_manager::entrypoint::run)
        .expect("spawn main thread")
        .join()
        .expect("main thread panicked");
}
