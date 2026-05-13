//! `gw config` command implementation.
//!
//! Surface (intentionally small):
//!   - `gw config list`              — merged view + scope annotation
//!   - `gw config get <key>`         — resolved value (single line)
//!   - `gw config set <key> <value>` — write to global, `--repo` to override
//!   - `gw config edit`              — TUI editor switching global ↔ repo
//!
//! Scope model:
//!   - **global**: `~/.config/git-worktree-manager/config.json`
//!   - **repo**:   `<repo-root>/.cwconfig.json`
//!
//! Resolved value follows the same precedence as runtime
//! `load_effective_config`: defaults < global < repo.
//!
//! The settable surface is the [`ConfigKey`] enum — clap derives the value
//! parser from it, so `gw config set <key>` auto-completes / errors against
//! the same list of keys the code understands. New keys go here and become
//! settable everywhere at once.

use std::path::{Path, PathBuf};

use clap::ValueEnum;
use console::style;
use serde_json::{json, Value};

use crate::config::{get_config_path, Config};
use crate::error::{CwError, Result};
use crate::git;

/// Every key gw exposes through `gw config get/set`.
///
/// Keys here mirror the JSON paths in [`crate::config::Config`]. The
/// `ValueEnum` derive gives clap a closed enumeration so a typo at the CLI
/// fails fast instead of silently writing a key the code never reads.
///
/// Order is the order users see in `gw config list` — keep related keys
/// adjacent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum ConfigKey {
    #[value(name = "ai-tool.command")]
    AiToolCommand,
    #[value(name = "ai-tool.args")]
    AiToolArgs,
    #[value(name = "ai-tool.guard")]
    AiToolGuard,
    #[value(name = "launch.method")]
    LaunchMethod,
    #[value(name = "launch.tmux-session-prefix")]
    LaunchTmuxSessionPrefix,
    #[value(name = "launch.wezterm-ready-timeout")]
    LaunchWeztermReadyTimeout,
    #[value(name = "update.auto-check")]
    UpdateAutoCheck,
    #[value(name = "hooks.post-new")]
    HooksPostNew,
    #[value(name = "hooks.pre-rm")]
    HooksPreRm,
}

impl ConfigKey {
    pub const ALL: &'static [ConfigKey] = &[
        ConfigKey::AiToolCommand,
        ConfigKey::AiToolArgs,
        ConfigKey::AiToolGuard,
        ConfigKey::LaunchMethod,
        ConfigKey::LaunchTmuxSessionPrefix,
        ConfigKey::LaunchWeztermReadyTimeout,
        ConfigKey::UpdateAutoCheck,
        ConfigKey::HooksPostNew,
        ConfigKey::HooksPreRm,
    ];

    /// Dotted user-facing key (matches the `#[value(name=...)]` form).
    pub fn name(self) -> &'static str {
        match self {
            ConfigKey::AiToolCommand => "ai-tool.command",
            ConfigKey::AiToolArgs => "ai-tool.args",
            ConfigKey::AiToolGuard => "ai-tool.guard",
            ConfigKey::LaunchMethod => "launch.method",
            ConfigKey::LaunchTmuxSessionPrefix => "launch.tmux-session-prefix",
            ConfigKey::LaunchWeztermReadyTimeout => "launch.wezterm-ready-timeout",
            ConfigKey::UpdateAutoCheck => "update.auto-check",
            ConfigKey::HooksPostNew => "hooks.post-new",
            ConfigKey::HooksPreRm => "hooks.pre-rm",
        }
    }

    /// JSON Pointer path into the serialized [`Config`] tree.
    ///
    /// Kept separate from [`Self::name`] because the on-disk schema uses
    /// snake_case (matching `#[derive(Serialize)]` field names) while the
    /// CLI surface uses kebab-case for consistency with the rest of clap.
    fn json_path(self) -> &'static [&'static str] {
        match self {
            ConfigKey::AiToolCommand => &["ai_tool", "command"],
            ConfigKey::AiToolArgs => &["ai_tool", "args"],
            ConfigKey::AiToolGuard => &["ai_tool", "guard"],
            ConfigKey::LaunchMethod => &["launch", "method"],
            ConfigKey::LaunchTmuxSessionPrefix => &["launch", "tmux_session_prefix"],
            ConfigKey::LaunchWeztermReadyTimeout => &["launch", "wezterm_ready_timeout"],
            ConfigKey::UpdateAutoCheck => &["update", "auto_check"],
            ConfigKey::HooksPostNew => &["hooks", "post_new"],
            ConfigKey::HooksPreRm => &["hooks", "pre_rm"],
        }
    }
}

// ---------------------------------------------------------------------------
// Value get / set on a serde_json::Value
// ---------------------------------------------------------------------------

/// Look up a [`ConfigKey`] in a serialized config tree. Returns `None` when
/// any segment along the path is missing or null.
pub fn lookup_value(root: &Value, key: ConfigKey) -> Option<&Value> {
    let mut cur = root;
    for seg in key.json_path() {
        cur = cur.get(*seg)?;
        if cur.is_null() {
            return None;
        }
    }
    Some(cur)
}

/// Parse a user-typed string into the JSON value [`ConfigKey`] expects.
///
/// Strings stay strings; booleans accept `true`/`false`; numbers parse via
/// `serde_json`; `args` accepts a JSON array literal or a space-separated
/// shell-ish list. Empty string means "unset" — the caller decides whether
/// to remove the field or leave it default.
pub fn parse_value_for(key: ConfigKey, input: &str) -> Result<Value> {
    let trimmed = input.trim();
    match key {
        ConfigKey::AiToolGuard | ConfigKey::UpdateAutoCheck => match trimmed {
            "true" | "1" | "yes" | "on" => Ok(Value::Bool(true)),
            "false" | "0" | "no" | "off" => Ok(Value::Bool(false)),
            other => Err(CwError::Config(format!(
                "{} expects a boolean (true/false), got: {}",
                key.name(),
                other
            ))),
        },
        ConfigKey::LaunchWeztermReadyTimeout => trimmed
            .parse::<f64>()
            .map_err(|e| {
                CwError::Config(format!(
                    "{} expects a number, got {:?}: {}",
                    key.name(),
                    trimmed,
                    e
                ))
            })
            .and_then(|n| {
                // `Number::from_f64` returns None for inf/-inf/NaN — JSON has
                // no representation for non-finite floats. Surface that as an
                // error rather than silently writing `null`.
                serde_json::Number::from_f64(n)
                    .map(Value::Number)
                    .ok_or_else(|| {
                        CwError::Config(format!(
                            "{} expects a finite number (got {})",
                            key.name(),
                            n
                        ))
                    })
            }),
        ConfigKey::AiToolArgs => {
            // Accept a JSON array literal first (lets users pass exact tokens
            // including ones with spaces). Fall back to whitespace splitting,
            // which matches what users naturally type.
            if trimmed.starts_with('[') {
                serde_json::from_str(trimmed).map_err(|e| {
                    CwError::Config(format!("{} got malformed JSON array: {}", key.name(), e))
                })
            } else if trimmed.is_empty() {
                Ok(json!([]))
            } else {
                Ok(Value::Array(
                    trimmed
                        .split_whitespace()
                        .map(|s| Value::String(s.to_string()))
                        .collect(),
                ))
            }
        }
        _ => Ok(Value::String(trimmed.to_string())),
    }
}

/// Set `key` to `value` inside `root`, creating intermediate objects as
/// needed. Errors only if an intermediate path collides with a non-object
/// (the only way this can happen is if the file was hand-edited).
pub fn set_value(root: &mut Value, key: ConfigKey, value: Value) -> Result<()> {
    let path = key.json_path();
    let (last, parents) = path.split_last().expect("ConfigKey paths are non-empty");

    if !root.is_object() {
        *root = Value::Object(serde_json::Map::new());
    }
    let mut cur = root;
    for seg in parents {
        let map = cur.as_object_mut().expect("ensured object above");
        let entry = map
            .entry((*seg).to_string())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        if !entry.is_object() {
            return Err(CwError::Config(format!(
                "config field `{}` is not an object — refusing to overwrite \
                 (run `gw config edit` to fix manually)",
                seg
            )));
        }
        cur = entry;
    }
    let map = cur.as_object_mut().expect("parent is an object");
    map.insert((*last).to_string(), value);
    Ok(())
}

/// Remove `key` from `root`. No-op if the path doesn't exist. Empty parent
/// objects are NOT pruned — we want a stable file shape and `serde` fills
/// missing fields with defaults on load anyway.
pub fn unset_value(root: &mut Value, key: ConfigKey) {
    let path = key.json_path();
    let (last, parents) = match path.split_last() {
        Some(p) => p,
        None => return,
    };
    let mut cur = root;
    for seg in parents {
        cur = match cur.get_mut(*seg) {
            Some(v) if v.is_object() => v,
            _ => return,
        };
    }
    if let Some(map) = cur.as_object_mut() {
        map.remove(*last);
    }
}

// ---------------------------------------------------------------------------
// File I/O for each scope
// ---------------------------------------------------------------------------

/// Where the global config file lives (delegates to [`crate::config`]).
pub fn global_path() -> PathBuf {
    get_config_path()
}

/// Resolve the repo-local `.cwconfig.json` path for the given cwd. Returns
/// `Err` when cwd is not inside a git repository — `--repo` is a no-op
/// outside one and we surface that as an error rather than silently
/// writing under the user's home.
pub fn repo_path(cwd: &Path) -> Result<PathBuf> {
    let repo = git::get_repo_root(Some(cwd)).map_err(|_| {
        CwError::Config("--repo / repo scope requires running inside a git repository".to_string())
    })?;
    Ok(repo.join(".cwconfig.json"))
}

/// Read a JSON file as a `Value`, returning `Value::Object({})` when the
/// file does not exist. Parse errors propagate.
pub fn read_json_or_empty(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(Value::Object(serde_json::Map::new()));
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| CwError::Config(format!("failed to read {}: {}", path.display(), e)))?;
    if content.trim().is_empty() {
        return Ok(Value::Object(serde_json::Map::new()));
    }
    serde_json::from_str(&content)
        .map_err(|e| CwError::Config(format!("failed to parse {}: {}", path.display(), e)))
}

/// Write `value` to `path` pretty-printed, creating parent directories.
pub fn write_json(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            CwError::Config(format!("failed to create {}: {}", parent.display(), e))
        })?;
    }
    let pretty = serde_json::to_string_pretty(value)
        .map_err(|e| CwError::Config(format!("failed to serialize config: {}", e)))?;
    // Trailing newline so the file plays nicely with editors / `cat`.
    let with_nl = format!("{}\n", pretty);
    std::fs::write(path, with_nl)
        .map_err(|e| CwError::Config(format!("failed to write {}: {}", path.display(), e)))
}

// ---------------------------------------------------------------------------
// Scope abstraction
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Global,
    Repo,
}

impl Scope {
    pub fn label(self) -> &'static str {
        match self {
            Scope::Global => "global",
            Scope::Repo => "repo",
        }
    }

    /// Toggle between the two scopes. Used by the edit TUI's Tab key.
    pub fn other(self) -> Self {
        match self {
            Scope::Global => Scope::Repo,
            Scope::Repo => Scope::Global,
        }
    }
}

// ---------------------------------------------------------------------------
// Commands: list / get / set
// ---------------------------------------------------------------------------

/// `gw config list` — render every known key with its resolved value and the
/// scope that supplied it.
pub fn list_cmd() -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let global_v = read_json_or_empty(&global_path())?;
    let repo_path_opt = git::get_repo_root(Some(&cwd))
        .ok()
        .map(|r| r.join(".cwconfig.json"));
    let repo_v = match &repo_path_opt {
        Some(p) => read_json_or_empty(p)?,
        None => Value::Object(serde_json::Map::new()),
    };

    let default_v = serde_json::to_value(Config::default())
        .map_err(|e| CwError::Config(format!("default config not serializable: {}", e)))?;

    println!();
    println!(
        "  {}  {}",
        style("global:").dim(),
        style(global_path().display()).cyan()
    );
    match &repo_path_opt {
        Some(p) => println!(
            "  {}    {}",
            style("repo:").dim(),
            style(p.display()).cyan()
        ),
        None => println!(
            "  {}    {}",
            style("repo:").dim(),
            style("(not in a git repo)").dim()
        ),
    }
    println!();

    // Two-column-ish formatting: key (left), value + scope (right). Keys
    // top out at 30 chars in the current schema; pad to the longest name
    // so columns line up without a tabulate dependency.
    let key_col = ConfigKey::ALL
        .iter()
        .map(|k| k.name().len())
        .max()
        .unwrap_or(0);

    for key in ConfigKey::ALL {
        let g = lookup_value(&global_v, *key);
        let r = lookup_value(&repo_v, *key);
        let d = lookup_value(&default_v, *key);
        let (value, tag) = match (r, g, d) {
            (Some(rv), Some(_), _) => (rv.clone(), style("[override: repo]").yellow()),
            (Some(rv), None, _) => (rv.clone(), style("[repo]").yellow()),
            (None, Some(gv), _) => (gv.clone(), style("[global]").green()),
            (None, None, Some(dv)) => (dv.clone(), style("[default]").dim()),
            (None, None, None) => (Value::Null, style("[unset]").dim()),
        };
        println!(
            "  {key:<key_col$}  {value}  {tag}",
            key = key.name(),
            value = render_value(&value),
        );
    }
    println!();
    Ok(())
}

/// Render a config value compactly for `list` / `get`.
fn render_value(v: &Value) -> String {
    match v {
        Value::Null => "(unset)".to_string(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Array(a) => {
            let inner: Vec<String> = a.iter().map(render_value).collect();
            format!("[{}]", inner.join(", "))
        }
        Value::Object(_) => v.to_string(),
    }
}

/// `gw config get <key>` — print the resolved value (repo > global > default).
/// Exits non-zero with no output when nothing is set, matching `git config`'s
/// convention for script consumers.
pub fn get_cmd(key: ConfigKey) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let global_v = read_json_or_empty(&global_path())?;
    let repo_v = git::get_repo_root(Some(&cwd))
        .ok()
        .map(|r| read_json_or_empty(&r.join(".cwconfig.json")))
        .transpose()?
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));

    if let Some(v) = lookup_value(&repo_v, key) {
        println!("{}", render_value(v));
        return Ok(());
    }
    if let Some(v) = lookup_value(&global_v, key) {
        println!("{}", render_value(v));
        return Ok(());
    }
    let default_v = serde_json::to_value(Config::default())
        .map_err(|e| CwError::Config(format!("default config not serializable: {}", e)))?;
    if let Some(v) = lookup_value(&default_v, key) {
        println!("{}", render_value(v));
        return Ok(());
    }
    Err(CwError::ExitCode(1))
}

/// `gw config set <key> <value> [--repo]`.
///
/// Default scope is global, matching `git config` semantics — `--repo`
/// writes to `<repo-root>/.cwconfig.json`.
pub fn set_cmd(key: ConfigKey, value: &str, scope: Scope) -> Result<()> {
    let parsed = parse_value_for(key, value)?;
    let target = match scope {
        Scope::Global => global_path(),
        Scope::Repo => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            repo_path(&cwd)?
        }
    };
    let mut root = read_json_or_empty(&target)?;
    set_value(&mut root, key, parsed.clone())?;
    write_json(&target, &root)?;
    println!(
        "{} {} = {}  {}",
        style("set").green().bold(),
        key.name(),
        render_value(&parsed),
        style(format!("[{}]", scope.label())).dim(),
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn empty() -> Value {
        Value::Object(serde_json::Map::new())
    }

    #[test]
    fn config_key_all_matches_name() {
        for k in ConfigKey::ALL {
            assert!(k.name().contains('.'));
            assert!(!k.json_path().is_empty());
        }
    }

    #[test]
    fn set_then_lookup_string() {
        let mut root = empty();
        set_value(
            &mut root,
            ConfigKey::AiToolCommand,
            Value::String("codex".into()),
        )
        .unwrap();
        let got = lookup_value(&root, ConfigKey::AiToolCommand).unwrap();
        assert_eq!(got, &Value::String("codex".into()));
    }

    #[test]
    fn set_creates_intermediate_objects() {
        let mut root = empty();
        set_value(
            &mut root,
            ConfigKey::HooksPostNew,
            Value::String("echo hi".into()),
        )
        .unwrap();
        // hooks.post_new exists; hooks.pre_rm path is absent (no extra fill).
        assert_eq!(
            lookup_value(&root, ConfigKey::HooksPostNew).unwrap(),
            &Value::String("echo hi".into())
        );
        assert!(lookup_value(&root, ConfigKey::HooksPreRm).is_none());
    }

    #[test]
    fn unset_removes_key_only() {
        let mut root = json!({"hooks": {"post_new": "x", "pre_rm": "y"}});
        unset_value(&mut root, ConfigKey::HooksPostNew);
        assert!(lookup_value(&root, ConfigKey::HooksPostNew).is_none());
        assert_eq!(
            lookup_value(&root, ConfigKey::HooksPreRm).unwrap(),
            &Value::String("y".into())
        );
    }

    #[test]
    fn unset_missing_key_noop() {
        let mut root = empty();
        unset_value(&mut root, ConfigKey::HooksPostNew);
        assert!(lookup_value(&root, ConfigKey::HooksPostNew).is_none());
    }

    #[test]
    fn parse_bool_variants() {
        assert_eq!(
            parse_value_for(ConfigKey::AiToolGuard, "true").unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            parse_value_for(ConfigKey::AiToolGuard, "no").unwrap(),
            Value::Bool(false)
        );
        assert!(parse_value_for(ConfigKey::AiToolGuard, "maybe").is_err());
    }

    #[test]
    fn parse_number_for_timeout() {
        let v = parse_value_for(ConfigKey::LaunchWeztermReadyTimeout, "7.5").unwrap();
        assert!(v.is_number());
        assert!((v.as_f64().unwrap() - 7.5).abs() < 1e-9);
    }

    #[test]
    fn parse_number_rejects_non_finite() {
        // serde_json::Number cannot represent inf/NaN; we surface that as an
        // error instead of silently writing `null` to the file.
        for input in ["inf", "-inf", "NaN"] {
            let err = parse_value_for(ConfigKey::LaunchWeztermReadyTimeout, input).unwrap_err();
            let msg = format!("{err}");
            assert!(
                msg.contains("finite") || msg.contains("expects a number"),
                "unexpected error for {input:?}: {msg}"
            );
        }
    }

    #[test]
    fn parse_empty_string_value_is_empty_string() {
        // Documenting the divergence between `gw config set <key> ""` (writes
        // an empty string for non-array keys) and the TUI's empty buffer (which
        // calls `unset_value` instead). Two surfaces, two semantics — captured
        // here so a future change is a conscious one.
        let v = parse_value_for(ConfigKey::AiToolCommand, "").unwrap();
        assert_eq!(v, Value::String(String::new()));
    }

    #[test]
    fn parse_args_whitespace_form() {
        let v = parse_value_for(ConfigKey::AiToolArgs, "--continue --resume").unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], Value::String("--continue".into()));
        assert_eq!(arr[1], Value::String("--resume".into()));
    }

    #[test]
    fn parse_args_json_form() {
        let v = parse_value_for(ConfigKey::AiToolArgs, r#"["a","b c"]"#).unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[1], Value::String("b c".into()));
    }

    #[test]
    fn parse_args_empty_is_empty_array() {
        let v = parse_value_for(ConfigKey::AiToolArgs, "  ").unwrap();
        assert!(v.as_array().unwrap().is_empty());
    }

    #[test]
    fn set_rejects_non_object_intermediate() {
        // hooks is a string here instead of an object — should refuse.
        let mut root = json!({"hooks": "oops"});
        let err = set_value(
            &mut root,
            ConfigKey::HooksPostNew,
            Value::String("x".into()),
        )
        .unwrap_err();
        assert!(format!("{err}").contains("not an object"));
    }
}
