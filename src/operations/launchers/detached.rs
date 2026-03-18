/// Detached launcher — run process fully detached from terminal.
use std::path::Path;
use std::process::{Command, Stdio};

/// Run command fully detached (survives terminal close).
pub fn run(path: &Path, cmd: &str) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            let _ = Command::new("bash")
                .args(["-lc", cmd])
                .current_dir(path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .pre_exec(|| {
                    libc::setsid();
                    Ok(())
                })
                .spawn();
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const DETACHED_PROCESS: u32 = 0x00000008;

        let _ = Command::new("cmd")
            .args(["/C", cmd])
            .current_dir(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS)
            .spawn();
    }
}
