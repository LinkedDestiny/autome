//! The compact Bug document header; no filesystem or scheduler state.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BugStatus {
    Located,
    CannotReproduce,
    NotABug,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BugReview {
    Pending,
    Passed,
    Rejected,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BugDoc {
    pub status: BugStatus,
    pub fix_round: u32,
    pub fix_limit: u32,
    pub repro_command: String,
    pub root_cause: String,
    pub review: BugReview,
    pub repos: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BugDocError {
    pub detail: String,
}
impl std::fmt::Display for BugDocError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.detail.fmt(f)
    }
}
impl std::error::Error for BugDocError {}
fn error(key: &str) -> BugDocError {
    BugDocError {
        detail: format!("Bug 文档字段 {key} 缺失、重复或值无效"),
    }
}

pub fn parse(text: &str) -> Result<BugDoc, BugDocError> {
    let mut fields = BTreeMap::new();
    let mut fenced = false;
    for (index, line) in text.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if line.starts_with("## ") {
            break;
        }
        if let Some((key, value)) = line.split_once([':', '：'])
            && ["status", "fix-round", "复现命令", "根因", "复核", "repos"].contains(&key)
            && fields.insert(key, value.trim()).is_some()
        {
            return Err(error(&format!("{key}（第 {} 行）", index + 1)));
        }
    }
    let field = |key| fields.get(key).copied().ok_or_else(|| error(key));
    let status = match field("status")? {
        "已定位" => BugStatus::Located,
        "无法复现" => BugStatus::CannotReproduce,
        "不是 Bug" => BugStatus::NotABug,
        _ => return Err(error("status")),
    };
    let review = match field("复核")? {
        "待复核" => BugReview::Pending,
        "通过" => BugReview::Passed,
        "打回" => BugReview::Rejected,
        _ => return Err(error("复核")),
    };
    let (round, limit) = field("fix-round")?
        .split_once('/')
        .ok_or_else(|| error("fix-round"))?;
    let number = |s: &str| {
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
            return Err(error("fix-round"));
        }
        s.parse::<u32>().map_err(|_| error("fix-round"))
    };
    let repro_command = field("复现命令")?.to_string();
    let root_cause = field("根因")?.to_string();
    if status == BugStatus::Located {
        for (key, value) in [("复现命令", &repro_command), ("根因", &root_cause)] {
            if value.is_empty() || value == "无" {
                return Err(error(key));
            }
        }
    }
    Ok(BugDoc {
        status,
        fix_round: number(round)?,
        fix_limit: number(limit)?,
        repro_command,
        root_cause,
        review,
        repos: fields.get("repos").map_or_else(Vec::new, |s| {
            s.split([',', '，'])
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const VALID: &str = "# Bug\nstatus: 已定位\nfix-round: 0/3\n复现命令: cargo test regression\n根因: off by one\n复核: 待复核\n";
    #[test]
    fn reads_all_five_fields_with_either_colon() {
        let doc = parse(VALID).unwrap();
        assert_eq!(doc, parse(&VALID.replace(':', "：")).unwrap());
        assert_eq!((doc.fix_round, doc.fix_limit), (0, 3));
        assert_eq!(doc.repro_command, "cargo test regression");
        assert_eq!(doc.root_cause, "off by one");
    }
    #[test]
    fn status_values_are_exactly_three() {
        for (value, expected) in [
            ("已定位", BugStatus::Located),
            ("无法复现", BugStatus::CannotReproduce),
            ("不是 Bug", BugStatus::NotABug),
        ] {
            assert_eq!(
                parse(&VALID.replace("已定位", value)).unwrap().status,
                expected
            );
        }
        for value in ["", "不是Bug", "已完成", "located"] {
            assert!(parse(&VALID.replace("已定位", value)).is_err());
        }
    }
    #[test]
    fn a_missing_or_duplicate_field_names_the_key() {
        for key in ["status", "fix-round", "复现命令", "根因", "复核"] {
            let line = VALID
                .lines()
                .find(|s| s.starts_with(&format!("{key}:")))
                .unwrap();
            for text in [
                VALID.replace(line, ""),
                format!("{VALID}{line}\n"),
                VALID.replace(line, &format!(" {line}")),
            ] {
                assert!(parse(&text).unwrap_err().detail.contains(key));
            }
        }
        assert!(parse("").is_err());
    }
    #[test]
    fn located_requires_a_command_and_a_root_cause() {
        for value in ["", "无"] {
            for original in ["cargo test regression", "off by one"] {
                assert!(parse(&VALID.replace(original, value)).is_err());
                for status in ["无法复现", "不是 Bug"] {
                    assert!(
                        parse(&VALID.replace(original, value).replace("已定位", status)).is_ok()
                    );
                }
            }
        }
    }
    #[test]
    fn fix_round_must_be_k_over_n() {
        for value in ["0/0", "01/03", "4294967295/4294967295", "9/1"] {
            assert!(parse(&VALID.replace("0/3", value)).is_ok(), "{value}");
        }
        for value in [
            "",
            "0",
            "-1/3",
            "+1/3",
            "1/",
            "/3",
            "1/2/3",
            "1 /3",
            "１/３",
            "4294967296/3",
        ] {
            assert!(parse(&VALID.replace("0/3", value)).is_err(), "{value}");
        }
    }
    #[test]
    fn review_values_are_exactly_three() {
        for (value, expected) in [
            ("待复核", BugReview::Pending),
            ("通过", BugReview::Passed),
            ("打回", BugReview::Rejected),
        ] {
            assert_eq!(
                parse(&VALID.replace("待复核", value)).unwrap().review,
                expected
            );
        }
        for value in ["", "待审", "passed"] {
            assert!(parse(&VALID.replace("待复核", value)).is_err());
        }
    }
    #[test]
    fn lines_after_the_first_section_or_inside_a_fence_are_ignored() {
        assert_eq!(
            parse(VALID),
            parse(&format!(
                "```text\nstatus: wrong\n## fake\n```\n{VALID}## 现象\nstatus: wrong"
            ))
        );
        assert!(parse(&format!("```\n{VALID}```")).is_err());
        assert!(parse(&format!("## 现象\n{VALID}")).is_err());
    }
    #[test]
    fn repos_is_optional_at_most_once_and_read_as_a_name_list() {
        assert!(parse(VALID).unwrap().repos.is_empty());
        assert_eq!(
            parse(&format!("{VALID}repos： a, ， b，c,\n"))
                .unwrap()
                .repos,
            ["a", "b", "c"]
        );
        assert_eq!(parse(&format!("{VALID}repos: a")).unwrap().repos, ["a"]);
        assert!(
            parse(&format!("{VALID}repos:\nrepos: a"))
                .unwrap_err()
                .detail
                .contains("repos")
        );
    }
}
