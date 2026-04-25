use git_worktree_manager::operations::claude_session::encode_project_dir;
use std::path::Path;

#[test]
fn encode_simple_path() {
    let p = Path::new("/Users/dave/Projects/github.com/git-worktree-manager");
    assert_eq!(
        encode_project_dir(p),
        "-Users-dave-Projects-github-com-git-worktree-manager"
    );
}

#[test]
fn encode_path_with_dots() {
    let p = Path::new("/Users/dave/Projects/github.com/foo.bar");
    assert_eq!(
        encode_project_dir(p),
        "-Users-dave-Projects-github-com-foo-bar"
    );
}

#[test]
fn encode_path_with_trailing_slash() {
    let p = Path::new("/tmp/foo/");
    assert_eq!(encode_project_dir(p), "-tmp-foo");
}
