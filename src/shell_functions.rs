/// Shell function generation for gw-cd.
///
/// Outputs shell-specific function definitions for bash/zsh/fish/powershell.
/// Generate shell function for the specified shell.
pub fn generate(shell: &str) -> Option<String> {
    match shell {
        "bash" | "zsh" => Some(BASH_ZSH_FUNCTION.to_string()),
        "fish" => Some(FISH_FUNCTION.to_string()),
        "powershell" | "pwsh" => Some(POWERSHELL_FUNCTION.to_string()),
        _ => None,
    }
}

const BASH_ZSH_FUNCTION: &str = r#"# git-worktree-manager shell functions for bash/zsh
# Source this file to enable shell functions:
#   source <(gw _shell-function bash)

# Navigate to a worktree by branch name
# If no argument is provided, show interactive worktree selector
# Use -g/--global to search across all registered repositories
# Supports repo:branch notation (auto-enables global mode)
gw-cd() {
    local branch=""
    local global_mode=0

    # Parse arguments
    while [ $# -gt 0 ]; do
        case "$1" in
            -g|--global)
                global_mode=1
                shift
                ;;
            -*)
                echo "Error: Unknown option '$1'" >&2
                echo "Usage: gw-cd [-g|--global] [branch|repo:branch]" >&2
                return 1
                ;;
            *)
                branch="$1"
                shift
                ;;
        esac
    done

    # Auto-detect repo:branch notation → enable global mode
    if [ $global_mode -eq 0 ] && [[ "$branch" == *:* ]]; then
        global_mode=1
    fi

    local worktree_path

    if [ -z "$branch" ]; then
        # No argument — interactive selector
        if [ $global_mode -eq 1 ]; then
            worktree_path=$(gw _path -g --interactive)
        else
            worktree_path=$(gw _path --interactive)
        fi
        if [ $? -ne 0 ]; then return 1; fi
    elif [ $global_mode -eq 1 ]; then
        # Global mode: delegate to gw _path -g
        worktree_path=$(gw _path -g "$branch")
        if [ $? -ne 0 ]; then return 1; fi
    else
        # Local mode: get worktree path from git directly
        worktree_path=$(git worktree list --porcelain 2>/dev/null | awk -v branch="$branch" '
            /^worktree / { path=$2 }
            /^branch / && $2 == "refs/heads/"branch { print path; exit }
        ')
    fi

    if [ -z "$worktree_path" ]; then
        echo "Error: No worktree found for branch '$branch'" >&2
        return 1
    fi

    if [ -d "$worktree_path" ]; then
        cd "$worktree_path" || return 1
        echo "Switched to worktree: $worktree_path"
    else
        echo "Error: Worktree directory not found: $worktree_path" >&2
        return 1
    fi
}

# Tab completion for gw-cd (bash)
_gw_cd_completion() {
    local cur="${COMP_WORDS[COMP_CWORD]}"
    local has_global=0

    # Remove colon from word break chars for repo:branch completion
    COMP_WORDBREAKS=${COMP_WORDBREAKS//:}

    # Check if -g or --global is already in the command
    local i
    for i in "${COMP_WORDS[@]}"; do
        case "$i" in -g|--global) has_global=1 ;; esac
    done

    # If current word starts with -, complete flags
    if [[ "$cur" == -* ]]; then
        COMPREPLY=($(compgen -W "-g --global" -- "$cur"))
        return
    fi

    local branches
    if [ $has_global -eq 1 ]; then
        # Global mode: get repo:branch from all registered repos
        branches=$(gw _path --list-branches -g 2>/dev/null)
    else
        # Local mode: get branches directly from git
        branches=$(git worktree list --porcelain 2>/dev/null | grep "^branch " | sed 's/^branch refs\/heads\///' | sort -u)
    fi
    COMPREPLY=($(compgen -W "$branches" -- "$cur"))
}

# Register completion for bash
if [ -n "$BASH_VERSION" ]; then
    complete -F _gw_cd_completion gw-cd
fi

# Tab completion for zsh
if [ -n "$ZSH_VERSION" ]; then
    # Register clap completion for gw CLI inline
    # (eliminates need for ~/.zfunc/_gw file and FPATH setup)
    _gw_completion() {
        eval $(env _GW_COMPLETE=complete_zsh COMP_WORDS="${words[*]}" COMP_CWORD=$((CURRENT-1)) gw --generate-completion zsh 2>/dev/null)
    }

    _gw_cd_zsh() {
        local has_global=0
        local i
        for i in "${words[@]}"; do
            case "$i" in -g|--global) has_global=1 ;; esac
        done

        # Complete flags
        if [[ "$PREFIX" == -* ]]; then
            local -a flags
            flags=('-g:Search all registered repositories' '--global:Search all registered repositories')
            _describe 'flags' flags
            return
        fi

        local -a branches
        if [ $has_global -eq 1 ]; then
            branches=(${(f)"$(gw _path --list-branches -g 2>/dev/null)"})
        else
            branches=(${(f)"$(git worktree list --porcelain 2>/dev/null | grep '^branch ' | sed 's/^branch refs\/heads\///' | sort -u)"})
        fi
        compadd -a branches
    }
    compdef _gw_cd_zsh gw-cd
fi

# Backward compatibility: cw-cd alias
cw-cd() { gw-cd "$@"; }
if [ -n "$BASH_VERSION" ]; then
    complete -F _gw_cd_completion cw-cd
fi
if [ -n "$ZSH_VERSION" ]; then
    compdef _gw_cd_zsh cw-cd
fi
"#;

const FISH_FUNCTION: &str = r#"# git-worktree-manager shell functions for fish
# Source this file to enable shell functions:
#   gw _shell-function fish | source

# Navigate to a worktree by branch name
# If no argument is provided, show interactive worktree selector
# Use -g/--global to search across all registered repositories
# Supports repo:branch notation (auto-enables global mode)
function gw-cd
    set -l global_mode 0
    set -l branch ""

    # Parse arguments
    for arg in $argv
        switch $arg
            case -g --global
                set global_mode 1
            case '-*'
                echo "Error: Unknown option '$arg'" >&2
                echo "Usage: gw-cd [-g|--global] [branch|repo:branch]" >&2
                return 1
            case '*'
                set branch $arg
        end
    end

    # Auto-detect repo:branch notation → enable global mode
    if test $global_mode -eq 0; and string match -q '*:*' -- "$branch"
        set global_mode 1
    end

    set -l worktree_path

    if test -z "$branch"
        # No argument — interactive selector
        if test $global_mode -eq 1
            set worktree_path (gw _path -g --interactive)
        else
            set worktree_path (gw _path --interactive)
        end
        if test $status -ne 0
            return 1
        end
    else if test $global_mode -eq 1
        # Global mode: delegate to gw _path -g
        set worktree_path (gw _path -g "$branch")
        if test $status -ne 0
            return 1
        end
    else
        # Local mode: get worktree path from git directly
        set worktree_path (git worktree list --porcelain 2>/dev/null | awk -v branch="$branch" '
            /^worktree / { path=$2 }
            /^branch / && $2 == "refs/heads/"branch { print path; exit }
        ')
    end

    if test -z "$worktree_path"
        if test -z "$branch"
            echo "Error: No worktree found (not in a git repository?)" >&2
        else
            echo "Error: No worktree found for branch '$branch'" >&2
        end
        return 1
    end

    if test -d "$worktree_path"
        cd "$worktree_path"; or return 1
        echo "Switched to worktree: $worktree_path"
    else
        echo "Error: Worktree directory not found: $worktree_path" >&2
        return 1
    end
end

# Tab completion for gw-cd
# Complete -g/--global flag
complete -c gw-cd -s g -l global -d 'Search all registered repositories'

# Complete branch names: global mode if -g is present, otherwise local git
complete -c gw-cd -f -n '__fish_contains_opt -s g global' -a '(gw _path --list-branches -g 2>/dev/null)'
complete -c gw-cd -f -n 'not __fish_contains_opt -s g global' -a '(git worktree list --porcelain 2>/dev/null | grep "^branch " | sed "s|^branch refs/heads/||" | sort -u)'

# Backward compatibility: cw-cd alias
function cw-cd; gw-cd $argv; end
complete -c cw-cd -w gw-cd
"#;

const POWERSHELL_FUNCTION: &str = r#"# git-worktree-manager shell functions for PowerShell
# Source this file to enable shell functions:
#   gw _shell-function powershell | Out-String | Invoke-Expression

# Navigate to a worktree by branch name
# If no argument is provided, show interactive worktree selector
# Use -g to search across all registered repositories
# Supports repo:branch notation (auto-enables global mode)
function gw-cd {
    param(
        [Parameter(Mandatory=$false, Position=0)]
        [string]$Branch,
        [Alias('global')]
        [switch]$g
    )

    # Auto-detect repo:branch notation → enable global mode
    if (-not $g -and $Branch -match ':') {
        $g = [switch]::Present
    }

    $worktreePath = $null

    if (-not $Branch) {
        # No argument — interactive selector
        if ($g) {
            $worktreePath = gw _path -g --interactive
        } else {
            $worktreePath = gw _path --interactive
        }
        if ($LASTEXITCODE -ne 0) {
            return
        }
    } elseif ($g) {
        # Global mode: delegate to gw _path -g
        $worktreePath = gw _path -g $Branch
        if ($LASTEXITCODE -ne 0) {
            return
        }
    } else {
        # Local mode: get worktree path from git directly
        $worktreePath = git worktree list --porcelain 2>&1 |
            Where-Object { $_ -is [string] } |
            ForEach-Object {
                if ($_ -match '^worktree (.+)$') { $path = $Matches[1] }
                if ($_ -match "^branch refs/heads/$Branch$") { $path }
            } | Select-Object -First 1
    }

    if (-not $worktreePath) {
        if (-not $Branch) {
            Write-Error "Error: No worktree found (not in a git repository?)"
        } else {
            Write-Error "Error: No worktree found for branch '$Branch'"
        }
        return
    }

    if (Test-Path -Path $worktreePath -PathType Container) {
        Set-Location -Path $worktreePath
        Write-Host "Switched to worktree: $worktreePath"
    } else {
        Write-Error "Error: Worktree directory not found: $worktreePath"
        return
    }
}

# Backward compatibility: cw-cd alias
Set-Alias -Name cw-cd -Value gw-cd

# Tab completion for gw-cd
Register-ArgumentCompleter -CommandName gw-cd -ParameterName Branch -ScriptBlock {
    param($commandName, $parameterName, $wordToComplete, $commandAst, $fakeBoundParameters)

    $branches = $null
    if ($fakeBoundParameters.ContainsKey('g')) {
        # Global mode: get repo:branch from all registered repos
        $branches = gw _path --list-branches -g 2>&1 |
            Where-Object { $_ -is [string] -and $_.Trim() } |
            Sort-Object -Unique
    } else {
        # Local mode: get branches from git
        $branches = git worktree list --porcelain 2>&1 |
            Where-Object { $_ -is [string] } |
            Select-String -Pattern '^branch ' |
            ForEach-Object { $_ -replace '^branch refs/heads/', '' } |
            Sort-Object -Unique
    }

    # Filter branches that match the current word
    $branches | Where-Object { $_ -like "$wordToComplete*" } |
        ForEach-Object {
            [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)
        }
}

# Tab completion for cw-cd (backward compat)
Register-ArgumentCompleter -CommandName cw-cd -ParameterName Branch -ScriptBlock {
    param($commandName, $parameterName, $wordToComplete, $commandAst, $fakeBoundParameters)

    $branches = $null
    if ($fakeBoundParameters.ContainsKey('g')) {
        $branches = gw _path --list-branches -g 2>&1 |
            Where-Object { $_ -is [string] -and $_.Trim() } |
            Sort-Object -Unique
    } else {
        $branches = git worktree list --porcelain 2>&1 |
            Where-Object { $_ -is [string] } |
            Select-String -Pattern '^branch ' |
            ForEach-Object { $_ -replace '^branch refs/heads/', '' } |
            Sort-Object -Unique
    }

    $branches | Where-Object { $_ -like "$wordToComplete*" } |
        ForEach-Object {
            [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)
        }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_bash() {
        let result = generate("bash");
        assert!(result.is_some());
        let script = result.unwrap();
        assert!(script.contains("gw-cd()"));
        assert!(script.contains("_gw_cd_completion"));
        assert!(script.contains("cw-cd"));
        assert!(script.contains("BASH_VERSION"));
        assert!(script.contains("ZSH_VERSION"));
        assert!(script.contains("_gw_cd_zsh"));
    }

    #[test]
    fn test_generate_zsh() {
        let result = generate("zsh");
        assert!(result.is_some());
        let script = result.unwrap();
        assert!(script.contains("compdef _gw_cd_zsh gw-cd"));
        assert!(script.contains("compdef _gw_cd_zsh cw-cd"));
    }

    #[test]
    fn test_generate_fish() {
        let result = generate("fish");
        assert!(result.is_some());
        let script = result.unwrap();
        assert!(script.contains("function gw-cd"));
        assert!(script.contains("complete -c gw-cd"));
        assert!(script.contains("function cw-cd"));
        assert!(script.contains("complete -c cw-cd -w gw-cd"));
    }

    #[test]
    fn test_generate_powershell() {
        let result = generate("powershell");
        assert!(result.is_some());
        let script = result.unwrap();
        assert!(script.contains("function gw-cd"));
        assert!(script.contains("Register-ArgumentCompleter"));
        assert!(script.contains("Set-Alias -Name cw-cd -Value gw-cd"));
    }

    #[test]
    fn test_generate_pwsh_alias() {
        let result = generate("pwsh");
        assert!(result.is_some());
        // pwsh should return the same as powershell
        assert_eq!(result, generate("powershell"));
    }

    #[test]
    fn test_generate_unknown() {
        assert!(generate("unknown").is_none());
        assert!(generate("").is_none());
    }

    /// Verify bash/zsh script has valid syntax using `bash -n`.
    #[test]
    fn test_bash_script_syntax() {
        let script = generate("bash").unwrap();

        // bash -n: check syntax without executing
        let output = std::process::Command::new("bash")
            .arg("-n")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                child.stdin.take().unwrap().write_all(script.as_bytes())?;
                child.wait_with_output()
            });

        match output {
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                assert!(
                    out.status.success(),
                    "bash -n failed for generated bash/zsh script:\n{}",
                    stderr
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("bash not found, skipping syntax check");
            }
            Err(e) => panic!("failed to run bash -n: {}", e),
        }
    }

    /// Verify fish script has valid syntax using `fish --no-execute`.
    #[test]
    fn test_fish_script_syntax() {
        let script = generate("fish").unwrap();

        let output = std::process::Command::new("fish")
            .arg("--no-execute")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                child.stdin.take().unwrap().write_all(script.as_bytes())?;
                child.wait_with_output()
            });

        match output {
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                assert!(
                    out.status.success(),
                    "fish --no-execute failed for generated fish script:\n{}",
                    stderr
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("fish not found, skipping syntax check");
            }
            Err(e) => panic!("failed to run fish --no-execute: {}", e),
        }
    }
}
