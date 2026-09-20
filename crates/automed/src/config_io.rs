//! Reading and writing the two configuration files. Technical design §3.3, §9.
//!
//! - `~/.autome/config.toml` — the global defaults, always complete.
//! - `<repo>/.autome/config.toml` — the project's sparse overrides, committed
//!   with the repository so a second machine picks up the same Loop setup.
//!
//! Two properties matter more than convenience here:
//!
//! 1. **Sparseness survives a round trip.** "Restore default" removes a key;
//!    if saving rewrote the file from a fully-resolved struct, every field
//!    would become an override and later changes to the global defaults would
//!    silently stop reaching the project. Writes therefore go through
//!    `toml_edit`, which mutates the existing document in place.
//! 2. **A hand-edited file is never silently discarded.** The project config is
//!    a committed, human-editable file; people will open it. A parse failure is
//!    reported with the line and column, not swallowed into a default.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use autome_domain::config::{
    GlobalConfig, LoopOverrides, ProjectConfig, RoleConfig, RoleOverrides,
};
use autome_domain::role::{Role, Runtime};
use toml_edit::{DocumentMut, Item, Table, value};

#[derive(Debug)]
pub enum ConfigError {
    Io { path: String, detail: String },
    Parse { path: String, detail: String },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io { path, detail } => write!(f, "读写 {path} 失败：{detail}"),
            ConfigError::Parse { path, detail } => write!(f, "{path} 格式错误：{detail}"),
        }
    }
}

impl std::error::Error for ConfigError {}

pub type Result<T> = std::result::Result<T, ConfigError>;

/// `<repo>/.autome/config.toml`.
pub fn project_config_path(repo: &Path) -> PathBuf {
    repo.join(".autome").join("config.toml")
}

/// `<autome_home>/config.toml`.
///
/// The directory is passed in rather than read from the environment. That
/// keeps the whole call chain free of ambient state: two installs, or two
/// tests, can run against different roots at the same time without racing on
/// a process-global variable.
pub fn global_config_path(autome_home: &Path) -> PathBuf {
    autome_home.join("config.toml")
}

/// The default root, `~/.autome`, resolved once at startup by `main`.
pub fn default_global_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("AUTOME_HOME") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".autome")
}

/// Where the ledger lives: `<AUTOME_HOME>/state/automed.sqlite3`.
///
/// Under the home rather than beside `config.toml` because the two are for
/// different readers — the config is the user's to edit, and everything under
/// `state/` is the core's to own. One home either way: a machine's Autome
/// state is one thing to find, to copy, and to back up.
pub fn default_db_path(autome_home: &Path) -> PathBuf {
    autome_home.join("state").join("automed.sqlite3")
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// Loads the global config, creating it from the shipped defaults when absent.
///
/// A missing file is the normal first-run state, not an error: writing the
/// defaults out immediately means the user has something to edit and the file
/// the UI displays is the file on disk.
pub fn load_global(autome_home: &Path) -> Result<GlobalConfig> {
    let path = global_config_path(autome_home);
    if !path.exists() {
        let config = GlobalConfig::default();
        save_global(autome_home, &config)?;
        return Ok(config);
    }
    let text = read(&path)?;
    let mut config: GlobalConfig = parse_global(&text).map_err(|detail| ConfigError::Parse {
        path: path.display().to_string(),
        detail,
    })?;
    // A hand-edited file may be missing a role; fill it rather than making
    // every later `role()` call fallible.
    config.repair();
    Ok(config)
}

/// Loads a project's sparse overrides. A missing file means "override
/// nothing", which is exactly what a freshly initialised project has.
pub fn load_project(repo: &Path) -> Result<ProjectConfig> {
    let path = project_config_path(repo);
    if !path.exists() {
        return Ok(ProjectConfig::default());
    }
    let text = read(&path)?;
    parse_project(&text).map_err(|detail| ConfigError::Parse {
        path: path.display().to_string(),
        detail,
    })
}

fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|e| ConfigError::Io {
        path: path.display().to_string(),
        detail: e.to_string(),
    })
}

/// Parses the global document. The on-disk shape is flatter than the Rust
/// struct — `[loop]` rather than a `loop_defaults` field — so the mapping is
/// explicit rather than derived.
fn parse_global(text: &str) -> std::result::Result<GlobalConfig, String> {
    let doc: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut config = GlobalConfig::default();

    if let Some(t) = doc.get("loop").and_then(toml::Value::as_table) {
        if let Some(v) = t.get("parallel").and_then(toml::Value::as_integer) {
            config.loop_defaults.parallel = v.clamp(0, u32::MAX as i64) as u32;
        }
        if let Some(v) = t.get("design_rounds").and_then(toml::Value::as_integer) {
            config.loop_defaults.design_rounds = v.clamp(0, u32::MAX as i64) as u32;
        }
        if let Some(v) = t.get("budget_factor").and_then(toml::Value::as_integer) {
            config.loop_defaults.budget_factor = v.clamp(0, u32::MAX as i64) as u32;
        }
    }

    // An unrecognised value falls back to the default rather than failing the
    // load: a typo in the appearance must not stop the app from starting,
    // which is what refusing the whole file would do.
    if let Some(v) = doc
        .get("ui")
        .and_then(toml::Value::as_table)
        .and_then(|t| t.get("theme"))
        .and_then(toml::Value::as_str)
    {
        config.ui.theme = autome_domain::config::Theme::parse(v).unwrap_or_default();
    }

    if let Some(roles) = doc.get("roles").and_then(toml::Value::as_table) {
        for (name, table) in roles {
            let Some(role) = Role::parse(name) else {
                // An unknown role key is a typo in a hand-edited file. Ignoring
                // it silently would leave the user staring at a setting that
                // does nothing.
                return Err(format!(
                    "未知角色 `{name}`，可选：plan/review/adjudicate/impl/audit"
                ));
            };
            let Some(table) = table.as_table() else {
                return Err(format!("[roles.{name}] 不是一个表"));
            };
            let base = config.role(role).clone();
            config.roles.insert(role, read_role(role, table, base)?);
        }
    }
    Ok(config)
}

fn read_role(
    role: Role,
    table: &toml::Table,
    base: RoleConfig,
) -> std::result::Result<RoleConfig, String> {
    let mut cfg = base;
    if let Some(v) = table.get("enabled") {
        cfg.enabled = v
            .as_bool()
            .ok_or_else(|| format!("[roles.{role}] enabled 必须是 true/false"))?;
    }
    if let Some(v) = table.get("runtime") {
        let s = v
            .as_str()
            .ok_or_else(|| format!("[roles.{role}] runtime 必须是字符串"))?;
        cfg.runtime = Runtime::parse(s)
            .ok_or_else(|| format!("[roles.{role}] runtime `{s}` 未知，可选：claude/codex"))?;
    }
    if let Some(v) = table.get("model") {
        cfg.model = v
            .as_str()
            .ok_or_else(|| format!("[roles.{role}] model 必须是字符串"))?
            .to_string();
    }
    if let Some(v) = table.get("effort") {
        // An empty string is how a TOML file says "CLI default", since TOML
        // has no way to write an explicit null.
        let s = v
            .as_str()
            .ok_or_else(|| format!("[roles.{role}] effort 必须是字符串"))?;
        cfg.effort = if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        };
    }
    if let Some(v) = table.get("skills") {
        cfg.skills = read_skills(role, v)?;
    }
    Ok(cfg)
}

fn read_skills(role: Role, v: &toml::Value) -> std::result::Result<Vec<String>, String> {
    let arr = v
        .as_array()
        .ok_or_else(|| format!("[roles.{role}] skills 必须是字符串数组"))?;
    let mut out = Vec::new();
    for item in arr {
        let s = item
            .as_str()
            .ok_or_else(|| format!("[roles.{role}] skills 的元素必须是字符串"))?;
        if !s.is_empty() && !out.iter().any(|e| e == s) {
            out.push(s.to_string());
        }
    }
    Ok(out)
}

/// Parses a project document. Every field is optional; absence means inherit.
fn parse_project(text: &str) -> std::result::Result<ProjectConfig, String> {
    let doc: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut config = ProjectConfig::default();

    if let Some(t) = doc.get("loop").and_then(toml::Value::as_table) {
        config.loop_overrides = LoopOverrides {
            parallel: t
                .get("parallel")
                .and_then(toml::Value::as_integer)
                .map(|v| v.clamp(0, u32::MAX as i64) as u32),
            design_rounds: t
                .get("design_rounds")
                .and_then(toml::Value::as_integer)
                .map(|v| v.clamp(0, u32::MAX as i64) as u32),
            budget_factor: t
                .get("budget_factor")
                .and_then(toml::Value::as_integer)
                .map(|v| v.clamp(0, u32::MAX as i64) as u32),
            protocol: t
                .get("protocol")
                .and_then(toml::Value::as_str)
                .map(str::to_string)
                .filter(|s| !s.trim().is_empty()),
        };
    }

    if let Some(roles) = doc.get("roles").and_then(toml::Value::as_table) {
        for (name, table) in roles {
            let Some(role) = Role::parse(name) else {
                return Err(format!(
                    "未知角色 `{name}`，可选：plan/review/adjudicate/impl/audit"
                ));
            };
            let Some(table) = table.as_table() else {
                return Err(format!("[roles.{name}] 不是一个表"));
            };
            let mut over = RoleOverrides::default();
            if let Some(v) = table.get("enabled") {
                over.enabled = Some(
                    v.as_bool()
                        .ok_or_else(|| format!("[roles.{name}] enabled 必须是 true/false"))?,
                );
            }
            if let Some(v) = table.get("runtime") {
                let s = v
                    .as_str()
                    .ok_or_else(|| format!("[roles.{name}] runtime 必须是字符串"))?;
                over.runtime = Some(Runtime::parse(s).ok_or_else(|| {
                    format!("[roles.{name}] runtime `{s}` 未知，可选：claude/codex")
                })?);
            }
            if let Some(v) = table.get("model") {
                over.model = Some(
                    v.as_str()
                        .ok_or_else(|| format!("[roles.{name}] model 必须是字符串"))?
                        .to_string(),
                );
            }
            if let Some(v) = table.get("effort") {
                let s = v
                    .as_str()
                    .ok_or_else(|| format!("[roles.{name}] effort 必须是字符串"))?;
                over.effort = Some(if s.is_empty() {
                    None
                } else {
                    Some(s.to_string())
                });
            }
            if let Some(v) = table.get("skills") {
                over.skills = Some(read_skills(role, v)?);
            }
            if !over.is_empty() {
                config.roles.insert(role, over);
            }
        }
    }
    Ok(config)
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

/// Writes the global config, preserving whatever comments and ordering the
/// existing file has.
pub fn save_global(autome_home: &Path, config: &GlobalConfig) -> Result<()> {
    let path = global_config_path(autome_home);
    let existing = if path.exists() {
        read(&path)?
    } else {
        GLOBAL_TEMPLATE.to_string()
    };
    let mut doc: DocumentMut =
        existing
            .parse()
            .map_err(|e: toml_edit::TomlError| ConfigError::Parse {
                path: path.display().to_string(),
                detail: e.to_string(),
            })?;

    let loop_table = ensure_table(doc.as_table_mut(), "loop");
    loop_table["parallel"] = value(config.loop_defaults.parallel as i64);
    loop_table["design_rounds"] = value(config.loop_defaults.design_rounds as i64);
    loop_table["budget_factor"] = value(config.loop_defaults.budget_factor as i64);

    let ui_table = ensure_table(doc.as_table_mut(), "ui");
    ui_table["theme"] = value(config.ui.theme.as_str());

    let roles_table = ensure_table(doc.as_table_mut(), "roles");
    roles_table.set_implicit(true);
    for role in Role::ALL {
        let cfg = config.role(role);
        let t = ensure_table(roles_table, role.as_str());
        t["enabled"] = value(cfg.enabled);
        t["runtime"] = value(cfg.runtime.as_str());
        t["model"] = value(cfg.model.as_str());
        t["effort"] = value(cfg.effort.clone().unwrap_or_default());
        t["skills"] = array_of_strings(&cfg.skills);
    }

    write_atomically(&path, doc.to_string().as_bytes())
}

/// Writes a project's sparse overrides.
///
/// Keys that are no longer overridden are *removed*, and a role table that
/// ends up empty is removed with them — that is what makes "restore default"
/// actually restore inheritance rather than freeze the current value.
pub fn save_project(repo: &Path, config: &ProjectConfig) -> Result<()> {
    let path = project_config_path(repo);
    let existing = if path.exists() {
        read(&path)?
    } else {
        PROJECT_TEMPLATE.to_string()
    };
    let mut doc: DocumentMut =
        existing
            .parse()
            .map_err(|e: toml_edit::TomlError| ConfigError::Parse {
                path: path.display().to_string(),
                detail: e.to_string(),
            })?;

    {
        let loop_table = ensure_table(doc.as_table_mut(), "loop");
        set_or_remove_int(loop_table, "parallel", config.loop_overrides.parallel);
        set_or_remove_int(
            loop_table,
            "design_rounds",
            config.loop_overrides.design_rounds,
        );
        set_or_remove_int(
            loop_table,
            "budget_factor",
            config.loop_overrides.budget_factor,
        );
        match &config.loop_overrides.protocol {
            Some(tag) => loop_table["protocol"] = value(tag.as_str()),
            None => {
                loop_table.remove("protocol");
            }
        }
    }

    {
        let roles_table = ensure_table(doc.as_table_mut(), "roles");
        roles_table.set_implicit(true);
        for role in Role::ALL {
            let over = config.roles.get(&role);
            match over {
                None => {
                    roles_table.remove(role.as_str());
                }
                Some(over) if over.is_empty() => {
                    roles_table.remove(role.as_str());
                }
                Some(over) => {
                    let t = ensure_table(roles_table, role.as_str());
                    match over.enabled {
                        Some(v) => t["enabled"] = value(v),
                        None => {
                            t.remove("enabled");
                        }
                    }
                    match over.runtime {
                        Some(v) => t["runtime"] = value(v.as_str()),
                        None => {
                            t.remove("runtime");
                        }
                    }
                    match &over.model {
                        Some(v) => t["model"] = value(v.as_str()),
                        None => {
                            t.remove("model");
                        }
                    }
                    match &over.effort {
                        Some(v) => t["effort"] = value(v.clone().unwrap_or_default()),
                        None => {
                            t.remove("effort");
                        }
                    }
                    match &over.skills {
                        Some(v) => t["skills"] = array_of_strings(v),
                        None => {
                            t.remove("skills");
                        }
                    }
                }
            }
        }
    }

    // An entirely empty `[loop]` table is noise in a committed file.
    if doc
        .get("loop")
        .and_then(Item::as_table)
        .is_some_and(|t| t.is_empty())
    {
        doc.as_table_mut().remove("loop");
    }

    write_atomically(&path, doc.to_string().as_bytes())
}

fn set_or_remove_int(table: &mut Table, key: &str, v: Option<u32>) {
    match v {
        Some(v) => table[key] = value(v as i64),
        None => {
            table.remove(key);
        }
    }
}

fn array_of_strings(items: &[String]) -> Item {
    let mut arr = toml_edit::Array::new();
    for s in items {
        arr.push(s.as_str());
    }
    value(arr)
}

/// Gets or inserts a sub-table, leaving an existing one (and its comments)
/// alone.
fn ensure_table<'a>(parent: &'a mut Table, key: &str) -> &'a mut Table {
    if !parent.contains_key(key) {
        parent.insert(key, Item::Table(Table::new()));
    }
    parent[key]
        .as_table_mut()
        .expect("just inserted or already a table")
}

/// Writes through a sibling temporary file and renames, so a crash mid-write
/// cannot leave a half-written config that the next start would refuse to
/// parse.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ConfigError::Io {
            path: parent.display().to_string(),
            detail: e.to_string(),
        })?;
    }
    let tmp = path.with_extension(format!("toml.tmp{}", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(|e| ConfigError::Io {
        path: tmp.display().to_string(),
        detail: e.to_string(),
    })?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        ConfigError::Io {
            path: path.display().to_string(),
            detail: e.to_string(),
        }
    })
}

/// The header written into a brand-new global config.
const GLOBAL_TEMPLATE: &str = r#"# Autome 全局默认配置。
# 所有项目在未覆盖的字段上跟随这里；项目覆盖写在各仓库的 .autome/config.toml。
# 改动对下一个启动的会话生效，正在运行的会话不受影响。
#
# 双模型纪律：review 与 plan、audit 与 impl 不得使用相同的 runtime:model。
# effort 留空表示使用该 CLI 的默认值。
"#;

/// The header written into a brand-new project config. Deliberately explains
/// the sparseness, because this file is committed and people will read it.
const PROJECT_TEMPLATE: &str = r#"# 本项目的 Loop 配置覆盖。
# 只写需要覆盖的字段；未出现的字段继承 ~/.autome/config.toml 的全局默认值。
# 删除一个字段即恢复继承——不要为了"固定住"当前值而把全局值抄进来。
#
# 本文件随仓库提交，用于跨机器同步。任务内的 Agent 不得修改 .autome/ 下的内容。
"#;

/// The skills bound to each role, flattened for the UI's binding view.
pub fn skill_bindings(
    config: &GlobalConfig,
    project: &ProjectConfig,
) -> BTreeMap<String, Vec<Role>> {
    let resolved = autome_domain::config::resolve(config, project);
    let mut by_skill: BTreeMap<String, Vec<Role>> = BTreeMap::new();
    for r in &resolved.roles {
        for skill in &r.config.skills {
            by_skill.entry(skill.clone()).or_default().push(r.role);
        }
    }
    by_skill
}

#[cfg(test)]
mod tests {
    use super::*;
    use autome_domain::config::{Provenance, resolve};
    use std::sync::atomic::{AtomicU32, Ordering};

    /// The ledger follows the home, so pointing `AUTOME_HOME` somewhere else
    /// moves the whole installation rather than half of it. A test that ran
    /// against the developer's real `~/.autome` would be writing their state,
    /// so every suite here passes its own home — and that only works because
    /// the path is derived from it.
    #[test]
    fn the_database_lives_under_the_home_it_belongs_to() {
        let home = Path::new("/tmp/some-home");
        assert_eq!(
            default_db_path(home),
            Path::new("/tmp/some-home/state/automed.sqlite3")
        );
        // And it is not beside config.toml, which is the user's to edit.
        assert_ne!(default_db_path(home).parent(), Some(home));
    }

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path =
                std::env::temp_dir().join(format!("automed-cfg-{tag}-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            TempDir(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write_project(repo: &Path, text: &str) {
        let p = project_config_path(repo);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    // ---- project parsing ------------------------------------------------

    #[test]
    fn a_missing_project_file_means_override_nothing() {
        let dir = TempDir::new("missing");
        assert_eq!(load_project(dir.path()).unwrap(), ProjectConfig::default());
    }

    #[test]
    fn a_sparse_project_file_parses_only_what_it_states() {
        let dir = TempDir::new("sparse");
        write_project(
            dir.path(),
            r#"
[roles.impl]
model = "claude-sonnet-5"
"#,
        );
        let project = load_project(dir.path()).unwrap();
        assert_eq!(project.roles.len(), 1);
        let over = project.overrides_for(Role::Impl);
        assert_eq!(over.model.as_deref(), Some("claude-sonnet-5"));
        assert_eq!(over.runtime, None, "runtime is not overridden");
        assert_eq!(over.enabled, None);
    }

    #[test]
    fn an_empty_effort_string_means_cli_default_not_absent() {
        let dir = TempDir::new("effort");
        write_project(
            dir.path(),
            r#"
[roles.plan]
effort = ""
"#,
        );
        let project = load_project(dir.path()).unwrap();
        // `Some(None)`: overridden, to "no explicit effort".
        assert_eq!(project.overrides_for(Role::Plan).effort, Some(None));
        let resolved = resolve(&GlobalConfig::default(), &project);
        assert_eq!(resolved.role(Role::Plan).config.effort, None);
        assert_eq!(resolved.role(Role::Plan).provenance, Provenance::Project);
    }

    #[test]
    fn loop_overrides_parse() {
        let dir = TempDir::new("loop");
        write_project(dir.path(), "[loop]\nparallel = 2\n");
        let project = load_project(dir.path()).unwrap();
        assert_eq!(project.loop_overrides.parallel, Some(2));
        assert_eq!(project.loop_overrides.design_rounds, None);
    }

    #[test]
    fn an_unknown_role_key_is_an_error_not_a_silent_no_op() {
        let dir = TempDir::new("typo");
        write_project(dir.path(), "[roles.implement]\nmodel = \"x\"\n");
        let err = load_project(dir.path()).unwrap_err();
        assert!(err.to_string().contains("implement"), "{err}");
    }

    #[test]
    fn an_unknown_runtime_names_the_valid_choices() {
        let dir = TempDir::new("runtime");
        write_project(dir.path(), "[roles.impl]\nruntime = \"gemini\"\n");
        let err = load_project(dir.path()).unwrap_err();
        assert!(err.to_string().contains("claude/codex"), "{err}");
    }

    #[test]
    fn a_wrong_type_is_reported_rather_than_coerced() {
        let dir = TempDir::new("type");
        write_project(dir.path(), "[roles.impl]\nenabled = \"yes\"\n");
        assert!(load_project(dir.path()).is_err());
    }

    #[test]
    fn malformed_toml_reports_the_path() {
        let dir = TempDir::new("malformed");
        write_project(dir.path(), "[roles.impl\nmodel =");
        let err = load_project(dir.path()).unwrap_err();
        assert!(err.to_string().contains("config.toml"), "{err}");
    }

    #[test]
    fn duplicate_skills_are_collapsed_on_read() {
        let dir = TempDir::new("dupskills");
        write_project(
            dir.path(),
            "[roles.impl]\nskills = [\"a\", \"a\", \"b\", \"\"]\n",
        );
        let project = load_project(dir.path()).unwrap();
        assert_eq!(
            project.overrides_for(Role::Impl).skills,
            Some(vec!["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn an_empty_role_table_is_not_recorded_as_an_override() {
        let dir = TempDir::new("emptyrole");
        write_project(dir.path(), "[roles.impl]\n");
        assert!(load_project(dir.path()).unwrap().roles.is_empty());
    }

    // ---- project writing -------------------------------------------------

    #[test]
    fn saving_then_loading_a_project_config_round_trips() {
        let dir = TempDir::new("roundtrip");
        let mut project = ProjectConfig::default();
        project.roles.insert(
            Role::Audit,
            RoleOverrides {
                runtime: Some(Runtime::Claude),
                model: Some("claude-sonnet-5".into()),
                skills: Some(vec!["vitest".into()]),
                ..Default::default()
            },
        );
        project.loop_overrides.parallel = Some(4);
        save_project(dir.path(), &project).unwrap();
        assert_eq!(load_project(dir.path()).unwrap(), project);
    }

    #[test]
    fn saving_does_not_turn_inherited_fields_into_overrides() {
        let dir = TempDir::new("stays-sparse");
        let mut project = ProjectConfig::default();
        project.roles.insert(
            Role::Impl,
            RoleOverrides {
                effort: Some(Some("low".into())),
                ..Default::default()
            },
        );
        save_project(dir.path(), &project).unwrap();

        let text = std::fs::read_to_string(project_config_path(dir.path())).unwrap();
        assert!(text.contains("effort"));
        assert!(
            !text.contains("model"),
            "model must stay inherited:\n{text}"
        );
        assert!(
            !text.contains("runtime"),
            "runtime must stay inherited:\n{text}"
        );

        // And the round trip still inherits from a *changed* global.
        let mut global = GlobalConfig::default();
        global.roles.get_mut(&Role::Impl).unwrap().model = "claude-opus-6".into();
        let resolved = resolve(&global, &load_project(dir.path()).unwrap());
        assert_eq!(resolved.role(Role::Impl).config.model, "claude-opus-6");
        assert_eq!(
            resolved.role(Role::Impl).config.effort.as_deref(),
            Some("low")
        );
    }

    #[test]
    fn removing_an_override_removes_the_key_from_the_file() {
        let dir = TempDir::new("remove-key");
        let mut project = ProjectConfig::default();
        project.roles.insert(
            Role::Audit,
            RoleOverrides {
                model: Some("m".into()),
                effort: Some(Some("high".into())),
                ..Default::default()
            },
        );
        save_project(dir.path(), &project).unwrap();

        // "Restore default" for effort only.
        project.roles.get_mut(&Role::Audit).unwrap().effort = None;
        save_project(dir.path(), &project).unwrap();
        let text = std::fs::read_to_string(project_config_path(dir.path())).unwrap();
        assert!(text.contains("model"));
        assert!(!text.contains("effort"), "{text}");
    }

    #[test]
    fn removing_the_last_override_removes_the_whole_role_table() {
        let dir = TempDir::new("remove-table");
        let mut project = ProjectConfig::default();
        project.roles.insert(
            Role::Audit,
            RoleOverrides {
                model: Some("m".into()),
                ..Default::default()
            },
        );
        save_project(dir.path(), &project).unwrap();
        assert!(
            std::fs::read_to_string(project_config_path(dir.path()))
                .unwrap()
                .contains("[roles.audit]")
        );

        project.roles.remove(&Role::Audit);
        save_project(dir.path(), &project).unwrap();
        let text = std::fs::read_to_string(project_config_path(dir.path())).unwrap();
        assert!(!text.contains("[roles.audit]"), "{text}");
        assert_eq!(load_project(dir.path()).unwrap(), ProjectConfig::default());
    }

    #[test]
    fn user_comments_survive_a_save() {
        let dir = TempDir::new("comments");
        write_project(
            dir.path(),
            "# 这个项目的审计必须用 Codex，见 RFC-7\n[roles.audit]\nmodel = \"gpt-5.4\"\n",
        );
        let mut project = load_project(dir.path()).unwrap();
        project.roles.get_mut(&Role::Audit).unwrap().model = Some("gpt-5.9".into());
        save_project(dir.path(), &project).unwrap();
        let text = std::fs::read_to_string(project_config_path(dir.path())).unwrap();
        assert!(text.contains("RFC-7"), "comment was lost:\n{text}");
        assert!(text.contains("gpt-5.9"));
    }

    #[test]
    fn a_new_project_file_carries_the_explanatory_header() {
        let dir = TempDir::new("template");
        let mut project = ProjectConfig::default();
        project.loop_overrides.parallel = Some(2);
        save_project(dir.path(), &project).unwrap();
        let text = std::fs::read_to_string(project_config_path(dir.path())).unwrap();
        assert!(text.contains("继承"), "{text}");
        assert!(text.contains("随仓库提交"), "{text}");
    }

    #[test]
    fn an_all_inherited_project_config_writes_no_loop_table() {
        let dir = TempDir::new("no-loop");
        save_project(dir.path(), &ProjectConfig::default()).unwrap();
        let text = std::fs::read_to_string(project_config_path(dir.path())).unwrap();
        assert!(!text.contains("[loop]"), "{text}");
    }

    // ---- global ----------------------------------------------------------

    #[test]
    fn the_global_config_is_created_from_defaults_on_first_load() {
        let dir = TempDir::new("global-first");
        let config = load_global(dir.path()).unwrap();
        assert_eq!(config, GlobalConfig::default());
        assert!(global_config_path(dir.path()).exists());
        let text = std::fs::read_to_string(global_config_path(dir.path())).unwrap();
        assert!(text.contains("双模型纪律"), "{text}");
    }

    #[test]
    fn a_global_config_round_trips_through_save_and_load() {
        let dir = TempDir::new("global-round");
        let mut config = GlobalConfig::default();
        config.loop_defaults.parallel = 5;
        config.roles.get_mut(&Role::Plan).unwrap().effort = None;
        config.roles.get_mut(&Role::Audit).unwrap().skills = vec!["vitest".into()];
        save_global(dir.path(), &config).unwrap();
        assert_eq!(load_global(dir.path()).unwrap(), config);
    }

    #[test]
    fn a_global_file_missing_a_role_is_repaired_on_load() {
        let dir = TempDir::new("global-repair");
        std::fs::write(
            global_config_path(dir.path()),
            "[roles.plan]\nruntime = \"claude\"\nmodel = \"m\"\n",
        )
        .unwrap();
        let config = load_global(dir.path()).unwrap();
        assert_eq!(config.roles.len(), Role::ALL.len());
        assert_eq!(config.role(Role::Plan).model, "m");
        assert_eq!(config.role(Role::Audit).runtime, Runtime::Codex);
    }

    #[test]
    fn a_partially_specified_global_role_keeps_the_shipped_defaults_for_the_rest() {
        let dir = TempDir::new("global-partial");
        std::fs::write(
            global_config_path(dir.path()),
            "[roles.audit]\nmodel = \"gpt-9\"\n",
        )
        .unwrap();
        let config = load_global(dir.path()).unwrap();
        assert_eq!(config.role(Role::Audit).model, "gpt-9");
        assert_eq!(
            config.role(Role::Audit).runtime,
            Runtime::Codex,
            "unspecified fields keep the shipped default"
        );
    }

    // ---- misc ------------------------------------------------------------

    #[test]
    fn an_atomic_write_leaves_no_temporary_file_behind() {
        let dir = TempDir::new("atomic");
        save_project(dir.path(), &ProjectConfig::default()).unwrap();
        let entries: Vec<String> = std::fs::read_dir(dir.path().join(".autome"))
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec!["config.toml".to_string()], "{entries:?}");
    }

    #[test]
    fn skill_bindings_groups_roles_by_skill() {
        let mut global = GlobalConfig::default();
        global.roles.get_mut(&Role::Impl).unwrap().skills = vec!["shared".into()];
        let mut project = ProjectConfig::default();
        project.roles.insert(
            Role::Audit,
            RoleOverrides {
                skills: Some(vec!["shared".into(), "only-audit".into()]),
                ..Default::default()
            },
        );
        let bindings = skill_bindings(&global, &project);
        assert_eq!(bindings["shared"], vec![Role::Impl, Role::Audit]);
        assert_eq!(bindings["only-audit"], vec![Role::Audit]);
    }

    #[test]
    fn the_global_config_path_is_a_child_of_the_given_root() {
        assert_eq!(
            global_config_path(Path::new("/tmp/somewhere")),
            PathBuf::from("/tmp/somewhere/config.toml")
        );
    }
}
