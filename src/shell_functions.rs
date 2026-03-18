/// Shell function generation for gw-cd.
///
/// Outputs shell-specific function definitions for bash/zsh/fish.
/// Generate shell function for the specified shell.
pub fn generate(shell: &str) -> Option<String> {
    match shell {
        "bash" | "zsh" => Some(BASH_ZSH_FUNCTION.to_string()),
        "fish" => Some(FISH_FUNCTION.to_string()),
        _ => None,
    }
}

const BASH_ZSH_FUNCTION: &str = r#"# git-worktree-manager shell functions for bash/zsh
# Source this file to enable shell functions:
#   source <(gw _shell-function bash)

# Navigate to a worktree by branch name
gw-cd() {
    local branch=""
    local global_mode=0

    while [ $# -gt 0 ]; do
        case "$1" in
            -g|--global) global_mode=1; shift ;;
            -*) echo "Error: Unknown option '$1'" >&2; echo "Usage: gw-cd [-g|--global] [branch|repo:branch]" >&2; return 1 ;;
            *) branch="$1"; shift ;;
        esac
    done

    if [ $global_mode -eq 0 ] && [[ "$branch" == *:* ]]; then
        global_mode=1
    fi

    local worktree_path

    if [ -z "$branch" ]; then
        if [ $global_mode -eq 1 ]; then
            worktree_path=$(gw _path -g --interactive)
        else
            worktree_path=$(gw _path --interactive)
        fi
        if [ $? -ne 0 ]; then return 1; fi
    elif [ $global_mode -eq 1 ]; then
        worktree_path=$(gw _path -g "$branch")
        if [ $? -ne 0 ]; then return 1; fi
    else
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

_gw_cd_completion() {
    local cur="${COMP_WORDS[COMP_CWORD]}"
    local has_global=0
    COMP_WORDBREAKS=${COMP_WORDBREAKS//:}
    local i
    for i in "${COMP_WORDS[@]}"; do
        case "$i" in -g|--global) has_global=1 ;; esac
    done
    if [[ "$cur" == -* ]]; then
        COMPREPLY=($(compgen -W "-g --global" -- "$cur"))
        return
    fi
    local branches
    if [ $has_global -eq 1 ]; then
        branches=$(gw _path --list-branches -g 2>/dev/null)
    else
        branches=$(git worktree list --porcelain 2>/dev/null | grep "^branch " | sed 's/^branch refs\/heads\///' | sort -u)
    fi
    COMPREPLY=($(compgen -W "$branches" -- "$cur"))
}

if [ -n "$BASH_VERSION" ]; then
    complete -F _gw_cd_completion gw-cd
fi

if [ -n "$ZSH_VERSION" ]; then
    _gw_cd_zsh() {
        local has_global=0
        local i
        for i in "${words[@]}"; do
            case "$i" in -g|--global) has_global=1 ;; esac
        done
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

function gw-cd
    set -l global_mode 0
    set -l branch ""

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

    if test $global_mode -eq 0; and string match -q '*:*' -- "$branch"
        set global_mode 1
    end

    set -l worktree_path

    if test -z "$branch"
        if test $global_mode -eq 1
            set worktree_path (gw _path -g --interactive)
        else
            set worktree_path (gw _path --interactive)
        end
        if test $status -ne 0
            return 1
        end
    else if test $global_mode -eq 1
        set worktree_path (gw _path -g "$branch")
        if test $status -ne 0
            return 1
        end
    else
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

complete -c gw-cd -s g -l global -d 'Search all registered repositories'
complete -c gw-cd -f -n '__fish_contains_opt -s g global' -a '(gw _path --list-branches -g 2>/dev/null)'
complete -c gw-cd -f -n 'not __fish_contains_opt -s g global' -a '(git worktree list --porcelain 2>/dev/null | grep "^branch " | sed "s|^branch refs/heads/||" | sort -u)'

# Backward compatibility: cw-cd alias
function cw-cd; gw-cd $argv; end
complete -c cw-cd -w gw-cd
"#;
