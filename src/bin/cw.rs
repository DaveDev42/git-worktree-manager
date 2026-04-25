fn main() {
    // See gw.rs for rationale (Windows 1 MiB default main-thread stack).
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(git_worktree_manager::entrypoint::run)
        .expect("spawn main thread")
        .join()
        .expect("main thread panicked");
}
