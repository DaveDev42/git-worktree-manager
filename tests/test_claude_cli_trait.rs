//! Verifies the ClaudeCli trait can be implemented and its methods invoked.

use git_worktree_manager::operations::setup_claude::claude_cli::{ClaudeCli, ClaudeCliError};
use std::cell::RefCell;
use std::path::Path;

struct FakeCli {
    calls: RefCell<Vec<String>>,
}

impl ClaudeCli for FakeCli {
    fn marketplace_add(&self, path: &Path) -> Result<(), ClaudeCliError> {
        self.calls
            .borrow_mut()
            .push(format!("add:{}", path.display()));
        Ok(())
    }
    fn marketplace_update(&self, name: &str) -> Result<(), ClaudeCliError> {
        self.calls.borrow_mut().push(format!("mp-update:{}", name));
        Ok(())
    }
    fn plugin_install(&self, slug: &str) -> Result<(), ClaudeCliError> {
        self.calls.borrow_mut().push(format!("install:{}", slug));
        Ok(())
    }
    fn plugin_update(&self, slug: &str) -> Result<(), ClaudeCliError> {
        self.calls.borrow_mut().push(format!("update:{}", slug));
        Ok(())
    }
    fn is_available(&self) -> bool {
        true
    }
}

#[test]
fn fake_cli_records_calls() {
    let fake = FakeCli {
        calls: RefCell::new(Vec::new()),
    };
    assert!(fake.is_available());
    fake.marketplace_add(Path::new("/tmp/mp")).unwrap();
    fake.plugin_install("gw@gw-local").unwrap();
    fake.marketplace_update("gw-local").unwrap();
    fake.plugin_update("gw@gw-local").unwrap();

    let calls = fake.calls.borrow();
    assert_eq!(calls.len(), 4);
    assert_eq!(calls[0], "add:/tmp/mp");
    assert_eq!(calls[1], "install:gw@gw-local");
    assert_eq!(calls[2], "mp-update:gw-local");
    assert_eq!(calls[3], "update:gw@gw-local");
}
