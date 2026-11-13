//! A small argument parser.
//!
//! The command set is a dozen verbs with a handful of flags each, which is not
//! enough to justify a dependency — the core has none either. Flags may sit
//! anywhere on the line; `--` ends them.

use std::collections::BTreeMap;

/// Flags that take a value. Everything else starting with `-` is a switch.
const VALUED: [&str; 13] = [
    "-p",
    "--project",
    "--session",
    "--interval",
    "--docs-repo",
    "--attach",
    "--doc",
    "--tag",
    "--brief",
    "--file",
    "--kind",
    "--after",
    "--before",
];

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Args {
    pub positional: Vec<String>,
    /// Every value a flag was given, in order: `--attach` may repeat.
    flags: BTreeMap<String, Vec<String>>,
}

impl Args {
    pub fn parse(argv: &[String]) -> Result<Args, String> {
        let mut out = Args::default();
        let mut i = 0;
        let mut only_positional = false;
        while i < argv.len() {
            let a = &argv[i];
            i += 1;
            if only_positional || !a.starts_with('-') || a == "-" {
                out.positional.push(a.clone());
                continue;
            }
            if a == "--" {
                only_positional = true;
                continue;
            }
            // `--flag=value` as well as `--flag value`.
            let (name, inline) = match a.split_once('=') {
                Some((n, v)) if n.starts_with("--") => (n.to_string(), Some(v.to_string())),
                _ => (a.clone(), None),
            };
            let name = canonical(&name).to_string();
            if VALUED.contains(&name.as_str()) {
                let value = match inline {
                    Some(v) => v,
                    None => {
                        let v = argv
                            .get(i)
                            .ok_or_else(|| format!("{name} 需要一个值"))?
                            .clone();
                        i += 1;
                        v
                    }
                };
                out.flags.entry(name).or_default().push(value);
            } else {
                out.flags.entry(name).or_default();
            }
        }
        Ok(out)
    }

    pub fn has(&self, flag: &str) -> bool {
        self.flags.contains_key(canonical(flag))
    }

    pub fn value(&self, flag: &str) -> Option<&str> {
        self.flags
            .get(canonical(flag))
            .and_then(|v| v.last())
            .map(String::as_str)
    }

    pub fn values(&self, flag: &str) -> Vec<String> {
        self.flags.get(canonical(flag)).cloned().unwrap_or_default()
    }

    /// The Nth positional argument, or a usage error naming what was missing.
    pub fn need(&self, n: usize, what: &str) -> Result<&str, String> {
        self.positional
            .get(n)
            .map(String::as_str)
            .ok_or_else(|| format!("缺少参数：{what}"))
    }

    /// Flags the caller did not expect, so a typo is an error rather than
    /// silently ignored.
    pub fn unknown(&self, allowed: &[&str]) -> Option<String> {
        self.flags
            .keys()
            .find(|k| !allowed.iter().any(|a| canonical(a) == k.as_str()))
            .cloned()
    }
}

/// Short forms map onto their long names.
fn canonical(flag: &str) -> &str {
    match flag {
        "-p" => "--project",
        "-f" => "--follow",
        "-h" => "--help",
        "-V" => "--version",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &[&str]) -> Args {
        Args::parse(&s.iter().map(|x| x.to_string()).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn flags_may_come_anywhere_and_short_forms_are_the_long_ones() {
        let a = parse(&["task", "new", "-p", "shop", "add a cart", "--json"]);
        assert_eq!(a.positional, vec!["task", "new", "add a cart"]);
        assert_eq!(a.value("--project"), Some("shop"));
        assert!(a.has("--json"));
    }

    #[test]
    fn an_equals_sign_carries_the_value_and_a_flag_may_repeat() {
        let a = parse(&["--interval=5", "--attach", "a.png", "--attach", "b.png"]);
        assert_eq!(a.value("--interval"), Some("5"));
        assert_eq!(a.values("--attach"), vec!["a.png", "b.png"]);
    }

    #[test]
    fn a_double_dash_lets_a_request_start_with_a_dash() {
        let a = parse(&["task", "new", "--", "-v 输出太多"]);
        assert_eq!(a.positional, vec!["task", "new", "-v 输出太多"]);
    }

    #[test]
    fn a_valued_flag_at_the_end_is_an_error() {
        assert!(Args::parse(&["-p".to_string()]).is_err());
    }

    #[test]
    fn unexpected_flags_are_reported() {
        let a = parse(&["status", "--jsno"]);
        assert_eq!(a.unknown(&["--json"]), Some("--jsno".into()));
        assert_eq!(parse(&["status", "--json"]).unknown(&["--json"]), None);
    }
}

#[cfg(test)]
mod task_card {
    use super::*;
    #[test]
    fn brief_and_kind_take_values_and_a_dash_is_a_value() {
        let a = Args::parse(
            &["task", "new", "--brief", "-", "--kind", "bug", "--draft"].map(String::from),
        )
        .unwrap();
        assert_eq!(a.positional, ["task", "new"]);
        assert_eq!(a.value("--brief"), Some("-"));
        assert_eq!(a.value("--kind"), Some("bug"));
        assert!(a.has("--draft"));
    }
}

#[cfg(test)]
mod merge_auth_tests {
    use super::*;
    #[test]
    fn merge_auth_merge_cancel_is_parsed() {
        for args in [["merge", "cart", "--cancel"], ["--cancel", "merge", "cart"]] {
            let a = Args::parse(&args.map(String::from)).unwrap();
            assert_eq!(a.positional, ["merge", "cart"]);
            assert!(a.has("--cancel"));
        }
        let a = Args::parse(&["merge", "cart"].map(String::from)).unwrap();
        assert!(!a.has("--cancel"));
    }
}

#[test]
fn deps_cli_args() {
    let a = Args::parse(
        &[
            "queue",
            "move",
            "T-1",
            "--before=T-2",
            "--after",
            "T-3",
            "--after",
            "T-4,T-5",
        ]
        .map(String::from),
    )
    .unwrap();
    assert_eq!(a.positional, ["queue", "move", "T-1"]);
    assert_eq!(a.value("--before"), Some("T-2"));
    assert_eq!(a.values("--after"), ["T-3", "T-4,T-5"]);
    for flag in ["--before", "--after"] {
        assert!(Args::parse(&[flag.to_string()]).is_err());
    }
}
