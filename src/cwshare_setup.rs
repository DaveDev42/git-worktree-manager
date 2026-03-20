/// Setup prompt for .cwshare file creation.
///
/// Mirrors src/git_worktree_manager/cwshare_setup.py.
use std::path::Path;

use console::style;

use crate::git;

/// Common files that users might want to share across worktrees.
const COMMON_SHARED_FILES: &[&str] = &[
    ".env",
    ".env.local",
    ".env.development",
    ".env.test",
    "config/local.json",
    "config/local.yaml",
    "config/local.yml",
    ".vscode/settings.json",
];

/// Check if user has been prompted for .cwshare in this repo.
pub fn is_cwshare_prompted(repo: &Path) -> bool {
    git::get_config("cwshare.prompted", Some(repo))
        .map(|v| v == "true")
        .unwrap_or(false)
}

/// Mark that user has been prompted for .cwshare.
pub fn mark_cwshare_prompted(repo: &Path) {
    let _ = git::set_config("cwshare.prompted", "true", Some(repo));
}

/// Check if .cwshare file exists.
pub fn has_cwshare_file(repo: &Path) -> bool {
    repo.join(".cwshare").exists()
}

/// Detect common files that exist and might be worth sharing.
pub fn detect_common_files(repo: &Path) -> Vec<String> {
    COMMON_SHARED_FILES
        .iter()
        .filter(|f| repo.join(f).exists())
        .map(|f| f.to_string())
        .collect()
}

/// Create a .cwshare file with template content.
pub fn create_cwshare_template(repo: &Path, suggested_files: &[String]) {
    let cwshare_path = repo.join(".cwshare");

    let mut template = String::from(
        "# .cwshare - Files to copy to new worktrees\n\
         #\n\
         # Files listed here will be automatically copied when you run 'gw new'.\n\
         # Useful for environment files and local configs not tracked in git.\n\
         #\n\
         # Format:\n\
         #   - One file/directory path per line (relative to repo root)\n\
         #   - Lines starting with # are comments\n\
         #   - Empty lines are ignored\n",
    );

    if !suggested_files.is_empty() {
        template.push_str("#\n# Detected files in this repository (uncomment to enable):\n\n");
        for file in suggested_files {
            template.push_str(&format!("# {}\n", file));
        }
    } else {
        template.push_str("#\n# No common files detected. Add your own below:\n\n");
    }

    let _ = std::fs::write(&cwshare_path, template);
}

/// Prompt user to create .cwshare file on first run in this repo.
pub fn prompt_cwshare_setup() {
    // Check if in git repo
    let repo = match git::get_repo_root(None) {
        Ok(r) => r,
        Err(_) => return,
    };

    // Don't prompt in non-interactive environments
    if git::is_non_interactive() {
        return;
    }

    // Check if .cwshare already exists
    if has_cwshare_file(&repo) {
        if !is_cwshare_prompted(&repo) {
            mark_cwshare_prompted(&repo);
        }
        return;
    }

    // Check if already prompted
    if is_cwshare_prompted(&repo) {
        return;
    }

    // Detect common files
    let detected_files = detect_common_files(&repo);

    // Prompt user
    println!("\n{}", style(".cwshare File Setup").cyan().bold());
    println!(
        "\nWould you like to create a {} file?",
        style(".cwshare").cyan()
    );
    println!("This lets you automatically copy files to new worktrees (like .env, configs).\n");

    if !detected_files.is_empty() {
        println!(
            "{}",
            style("Detected files that you might want to share:").bold()
        );
        for file in &detected_files {
            println!("  {} {}", style("•").dim(), file);
        }
        println!();
    }

    // Ask user
    use std::io::Write;
    print!("Create .cwshare file? [Y/n]: ");
    let _ = std::io::stdout().flush();

    let mut input = String::new();
    match std::io::stdin().read_line(&mut input) {
        Ok(_) => {}
        Err(_) => {
            mark_cwshare_prompted(&repo);
            println!(
                "\n{}\n",
                style("You can create .cwshare manually anytime.").dim()
            );
            return;
        }
    }

    let input = input.trim().to_lowercase();

    // Mark as prompted regardless of answer
    mark_cwshare_prompted(&repo);

    if input.is_empty() || input == "y" || input == "yes" {
        create_cwshare_template(&repo, &detected_files);
        println!(
            "\n{} Created {}",
            style("*").green().bold(),
            repo.join(".cwshare").display()
        );
        println!("\n{}", style("Next steps:").bold());
        println!("  1. Review and edit .cwshare to uncomment files you want to share");
        println!(
            "  2. Add to git: {}",
            style("git add .cwshare && git commit").cyan()
        );
        println!(
            "  3. Files will be copied when you run: {}",
            style("gw new <branch>").cyan()
        );
        println!();
    } else {
        println!(
            "\n{}\n",
            style("You can create .cwshare manually anytime.").dim()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_has_cwshare_file_returns_false_when_missing() {
        let dir = TempDir::new().unwrap();
        assert!(!has_cwshare_file(dir.path()));
    }

    #[test]
    fn test_has_cwshare_file_returns_true_when_present() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".cwshare"), "# test").unwrap();
        assert!(has_cwshare_file(dir.path()));
    }

    #[test]
    fn test_detect_common_files_empty_dir() {
        let dir = TempDir::new().unwrap();
        let detected = detect_common_files(dir.path());
        assert!(detected.is_empty());
    }

    #[test]
    fn test_detect_common_files_finds_env() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".env"), "SECRET=123").unwrap();
        std::fs::write(dir.path().join(".env.local"), "LOCAL=1").unwrap();

        let detected = detect_common_files(dir.path());
        assert_eq!(detected, vec![".env", ".env.local"]);
    }

    #[test]
    fn test_detect_common_files_finds_nested() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("config")).unwrap();
        std::fs::write(dir.path().join("config/local.yaml"), "key: val").unwrap();

        let detected = detect_common_files(dir.path());
        assert_eq!(detected, vec!["config/local.yaml"]);
    }

    #[test]
    fn test_detect_common_files_finds_vscode_settings() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".vscode")).unwrap();
        std::fs::write(dir.path().join(".vscode/settings.json"), "{}").unwrap();

        let detected = detect_common_files(dir.path());
        assert_eq!(detected, vec![".vscode/settings.json"]);
    }

    #[test]
    fn test_create_cwshare_template_no_suggestions() {
        let dir = TempDir::new().unwrap();
        create_cwshare_template(dir.path(), &[]);

        let content = std::fs::read_to_string(dir.path().join(".cwshare")).unwrap();
        assert!(content.contains("# .cwshare - Files to copy to new worktrees"));
        assert!(content.contains("# No common files detected. Add your own below:"));
        assert!(!content.contains("Detected files"));
    }

    #[test]
    fn test_create_cwshare_template_with_suggestions() {
        let dir = TempDir::new().unwrap();
        let files = vec![".env".to_string(), ".env.local".to_string()];
        create_cwshare_template(dir.path(), &files);

        let content = std::fs::read_to_string(dir.path().join(".cwshare")).unwrap();
        assert!(content.contains("# .cwshare - Files to copy to new worktrees"));
        assert!(content.contains("# Detected files in this repository (uncomment to enable):"));
        assert!(content.contains("# .env\n"));
        assert!(content.contains("# .env.local\n"));
        assert!(!content.contains("No common files detected"));
    }

    #[test]
    fn test_create_cwshare_template_creates_file() {
        let dir = TempDir::new().unwrap();
        assert!(!dir.path().join(".cwshare").exists());

        create_cwshare_template(dir.path(), &[]);
        assert!(dir.path().join(".cwshare").exists());
    }

    #[test]
    fn test_is_cwshare_prompted_false_without_git() {
        // Non-git directory should return false (git config will fail)
        let dir = TempDir::new().unwrap();
        assert!(!is_cwshare_prompted(dir.path()));
    }
}
