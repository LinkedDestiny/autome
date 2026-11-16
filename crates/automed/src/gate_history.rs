//! Mechanical, deliberately broad retrieval of previously observed gate failures.
use std::{collections::BTreeSet, fs, path::Path};

/// Keep only closed segments, deduplicate in log order, and bound ledger growth.
pub fn cases(text: &str) -> Vec<String> {
    let mut pending: Option<Vec<&str>> = None;
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for line in text.lines() {
        if line.starts_with("autome-gate-cases begin ") {
            pending = Some(Vec::new());
        } else if line == "autome-gate-cases end" {
            if let Some(lines) = pending.take() {
                for line in lines {
                    if out.len() < 2000 && seen.insert(line) {
                        out.push(line.to_owned());
                    }
                }
            }
        } else if let Some(lines) = &mut pending {
            let fields: Vec<_> = line.split('\t').collect();
            if matches!(fields.as_slice(), ["case", file, title] if !file.is_empty() && !title.is_empty())
                || matches!(fields.as_slice(), ["setup", name] if !name.is_empty())
            {
                lines.push(line);
            }
        }
    }
    out
}

fn token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-'
}
fn identifier(s: &str) -> bool {
    let Some((prefix, number)) = s.split_once('-') else {
        return false;
    };
    prefix.starts_with(|c: char| c.is_ascii_uppercase())
        && prefix.chars().all(|c| c.is_ascii_alphanumeric())
        && !number.is_empty()
        && number
            .trim_end_matches(|c: char| c.is_ascii_lowercase())
            .chars()
            .all(|c| c.is_ascii_digit())
        && number.chars().next().is_some_and(|c| c.is_ascii_digit())
        && number.chars().filter(|c| c.is_ascii_lowercase()).count() <= 1
}
fn names(catalog: &[String]) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    for row in catalog {
        let fields: Vec<_> = row.split('\t').collect();
        match fields.as_slice() {
            ["case", file, title] => {
                out.push((file.rsplit('/').next().unwrap_or(file).to_string(), false));
                let id = title.split(|c| !token_char(c)).find(|s| identifier(s));
                out.push((id.unwrap_or(title).to_string(), id.is_some()));
            }
            ["setup", name] => out.push((name.to_string(), false)),
            _ => {}
        }
    }
    out
}
fn failure(text: &str) -> Option<usize> {
    [
        "失败",
        "未通过",
        "不通过",
        "超时",
        "ETIMEDOUT",
        "timed out",
        "timeout",
        "FAILED",
        "failed",
        "竞态",
        "偶发",
        "flaky",
        "未能",
        "无法",
        "阻断",
        "报错",
    ]
    .iter()
    .filter_map(|word| text.find(word))
    .min()
}
fn named(text: &str, names: &[(String, bool)]) -> bool {
    if names.is_empty() {
        return text
            .split(|c: char| !(c.is_alphanumeric() || "_.-".contains(c)))
            .any(|s| {
                ["spec", "test"].iter().any(|kind| {
                    [
                        "js", "jsx", "ts", "tsx", "mjs", "mjsx", "mts", "mtsx", "cjs", "cjsx",
                        "cts", "ctsx",
                    ]
                    .iter()
                    .any(|ext| {
                        s.strip_suffix(&format!(".{kind}.{ext}"))
                            .is_some_and(|s| !s.is_empty())
                    })
                })
            });
    }
    names.iter().any(|(name, boundary)| {
        text.match_indices(name).any(|(i, _)| {
            !boundary
                || (!text[..i].chars().next_back().is_some_and(token_char)
                    && !text[i + name.len()..]
                        .chars()
                        .next()
                        .is_some_and(token_char))
        })
    })
}
#[derive(Debug)]
struct Record {
    path: String,
    line: usize,
    text: String,
}
fn records(path: &str, text: &str, offset: usize) -> Vec<Record> {
    let mut out: Vec<Record> = Vec::new();
    let mut current: Option<Record> = None;
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        let list = line.len() == trimmed.len()
            && (trimmed.starts_with("- ")
                || trimmed.starts_with("* ")
                || trimmed.starts_with("+ ")
                || trimmed
                    .split_once(". ")
                    .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())));
        if line.trim().is_empty() || list || trimmed.starts_with('|') || line.starts_with('#') {
            if let Some(record) = current.take() {
                out.push(record);
            }
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
        }
        let record = current.get_or_insert_with(|| Record {
            path: path.into(),
            line: i + offset + 1,
            text: String::new(),
        });
        if !record.text.is_empty() {
            record.text.push('\n');
        }
        record.text.push_str(line);
        if trimmed.starts_with('|') {
            out.push(current.take().unwrap());
        }
    }
    if let Some(record) = current {
        out.push(record);
    }
    out
}
fn excerpt(text: &str) -> &str {
    let center = failure(text).unwrap_or(0);
    let mut start = center.saturating_sub(100);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    let mut end = (start + 300).min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[start..end]
}

/// I/O failures are visible in the brief, never disguised as an empty match set.
pub fn collect(
    root: &Path,
    doc_dir: &str,
    slug: &str,
    task: &str,
    round: u32,
    catalog: &[String],
) -> std::io::Result<String> {
    let design_path = format!("{doc_dir}/{slug}.md");
    let design = fs::read_to_string(root.join(&design_path))?;
    let mut all = Vec::new();
    if let Some(start) = design.lines().position(|line| line.trim() == "## Backlog") {
        let section = design
            .lines()
            .skip(start + 1)
            .take_while(|line| !line.starts_with("## "))
            .collect::<Vec<_>>()
            .join("\n");
        all.extend(records(&design_path, &section, start + 1));
    }
    let dir = root.join(doc_dir).join("evidence");
    let mut paths = match fs::read_dir(dir) {
        Ok(entries) => entries
            .map(|e| e.map(|e| e.path()))
            .collect::<std::io::Result<Vec<_>>>()?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    paths.sort();
    for path in paths.iter().filter(|p| {
        p.file_name()
            .is_some_and(|n| n.to_string_lossy().ends_with("-audit.md"))
    }) {
        let shown = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
        all.extend(records(&shown, &fs::read_to_string(path)?, 0));
    }
    let names = names(catalog);
    all.retain(|r| failure(&r.text).is_some() && named(&r.text, &names));
    let relative = format!(".autome/output/gate-history/{task}-{round}.md");
    let path = root.join(&relative);
    fs::create_dir_all(path.parent().unwrap())?;
    let full = all
        .iter()
        .map(|r| format!("## {}:{}\n\n{}\n\n", r.path, r.line, r.text))
        .collect::<String>();
    fs::write(path, full)?;
    let mut brief = format!(
        "\n## 门禁用例的既往失败记录\n\n机械匹配、可能误报。共 {} 条；完整提取：`{relative}`。\n",
        all.len()
    );
    if names.is_empty() {
        brief.push_str("未取得门禁用例清单，只按文件名式匹配。\n");
    }
    let mut shown = 0;
    for r in &all {
        let line = format!(
            "- {}:{}：{}\n",
            r.path,
            r.line,
            excerpt(&r.text).replace('\n', " ")
        );
        if shown == 60 || brief.len() + line.len() > 12 * 1024 - 100 {
            break;
        }
        brief.push_str(&line);
        shown += 1;
    }
    if all.is_empty() {
        brief.push_str("没有匹配的记录。\n");
    }
    if shown < all.len() {
        brief.push_str(&format!("另有 {} 条，见完整提取。\n", all.len() - shown));
    }
    Ok(brief)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new(design: &str, audit: &str) -> Self {
            let root = std::env::temp_dir().join(format!("gate-history-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(root.join("docs/x/evidence")).unwrap();
            fs::write(root.join("docs/x/x.md"), design).unwrap();
            fs::write(root.join("docs/x/evidence/M-01-r1-audit.md"), audit).unwrap();
            Self(root)
        }
        fn run(&self, catalog: &[String]) -> String {
            collect(&self.0, "docs/x", "x", "T-1", 2, catalog).unwrap()
        }
        fn full(&self) -> String {
            fs::read_to_string(self.0.join(".autome/output/gate-history/T-1-2.md")).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn catalog() -> Vec<String> {
        vec![
            "case\tcontrols.spec.ts\tB-3 会话".into(),
            "case\tlive.spec.ts\tC-2 数据".into(),
            "setup\tprepare()".into(),
        ]
    }
    #[test]
    fn gate_history_closed_union_and_limit() {
        let text = "autome-gate-cases begin a\ncase\tx.spec.ts\tB-3 a\nsetup\tprepare()\nautome-gate-cases end\nautome-gate-cases begin b\nsetup\tprepare()\nsetup\tsetup\nautome-gate-cases end\nautome-gate-cases begin open\nsetup\tignored";
        assert_eq!(
            cases(text),
            ["case\tx.spec.ts\tB-3 a", "setup\tprepare()", "setup\tsetup"]
        );
        assert!(cases("nothing").is_empty());
        assert!(
            cases("autome-gate-cases begin a\ncase\t\t\nsetup\t\nautome-gate-cases end").is_empty()
        );
        let rows = (0..2001)
            .map(|i| format!("setup\t{i}\n"))
            .collect::<String>();
        assert_eq!(
            cases(&format!(
                "autome-gate-cases begin a\n{rows}autome-gate-cases end"
            ))
            .len(),
            2000
        );
    }
    #[test]
    fn gate_history_multiline_record() {
        let f = Fixture::new(
            "",
            "- B-3 验证\n  只有续行记录超时\n- 无关\n  失败\n\nC-2 用例\n这里未通过\n\n| prepare() | ETIMEDOUT |\n",
        );
        let result = f.run(&catalog());
        assert!(result.contains("共 3 条"));
        assert!(result.contains("只有续行记录超时"));
        assert!(result.contains("audit.md:1"));
        assert!(result.contains("audit.md:6"));
    }
    #[test]
    fn gate_history_sixty_one_full_records_survive() {
        let audit = (1..=61)
            .map(|i| format!("- B-3 条目{i}\n  {}失败原因{i}\n", "前置说明".repeat(100)))
            .collect::<String>();
        let f = Fixture::new("", &audit);
        let result = f.run(&catalog());
        assert!(result.contains("共 61 条"));
        assert!(result.contains("另有 "));
        assert!(!result.contains("失败原因61"));
        assert!(f.full().contains("失败原因61"));
        assert_eq!(f.full().matches("## docs/").count(), 61);
        assert!(result.len() <= 12 * 1024);
        assert!(result.contains("失败原因1"));
    }
    #[test]
    fn gate_history_number_boundaries() {
        for (id, yes, no) in [
            ("B-3", "B-3", "B-31 B-03 BL-13 XB-3 B-3-x"),
            ("C-2", "C-2", "C-2b"),
            ("C-2b", "C-2b", "C-2 C-2b0"),
        ] {
            let n = names(&[format!("case\tx.spec.ts\t{id} 标题")]);
            assert!(named(&format!("（{yes}）失败"), &n));
            assert!(!named(no, &n), "{id}");
        }
        assert!(named(
            "无编号标题失败",
            &names(&["case\tx.spec.ts\t无编号标题".into()])
        ));
        assert!(!named(
            "无编号失败",
            &names(&["case\tx.spec.ts\t无编号标题".into()])
        ));
    }
    #[test]
    fn gate_history_only_backlog_and_audit() {
        let f = Fixture::new(
            "B-3 失败 outside\n## Backlog\n- B-3 失败 backlog\n## 争议项\nB-3 失败 outside2",
            "B-3 失败 audit",
        );
        fs::write(f.0.join("docs/x/evidence/M-01-r1-impl.md"), "B-3 失败 impl").unwrap();
        let result = f.run(&catalog());
        assert!(result.contains("共 2 条"));
        assert!(result.contains("x.md:3"));
        assert!(!result.contains("outside"));
        assert!(!result.contains("失败 impl"));
        fs::remove_file(f.0.join("docs/x/x.md")).unwrap();
        assert!(collect(&f.0, "docs/x", "x", "T-1", 3, &catalog()).is_err());
    }
    #[test]
    fn gate_history_same_unit_requires_failure() {
        let f = Fixture::new("", "B-3 成功\n\n无关失败\n\n- C-2 通过\n- 无关超时\n");
        assert!(f.run(&catalog()).contains("没有匹配的记录"));
    }
    #[test]
    fn gate_history_t98_original_line_baseline() {
        let text = include_str!("../tests/fixtures/gate-history-t98.txt");
        let c = vec![
            "case\tcontrols.spec.ts\tB-3 会话".into(),
            "case\tlive.spec.ts\tC-2 数据".into(),
            "case\tlive.spec.ts\tD-1 状态".into(),
            "setup\tglobalSetup".into(),
            "setup\tsetup".into(),
            "setup\tw7-flow.sh".into(),
            "setup\tprepare()".into(),
        ];
        let n = names(&c);
        let original = text
            .lines()
            .filter(|line| failure(line).is_some() && named(line, &n))
            .collect::<Vec<_>>();
        assert_eq!(original.len(), 8, "核实设计所说的原按行规则 8 条");
        let f = Fixture::new("", text);
        f.run(&c);
        for line in original {
            assert!(f.full().contains(line), "{line}");
        }
        assert!(f.full().contains("BL-13"));
    }
    #[test]
    fn gate_history_count_and_utf8_byte_caps() {
        let f = Fixture::new(
            "",
            &(0..61)
                .map(|i| format!("- B-3 失败{i}\n"))
                .collect::<String>(),
        );
        let result = f.run(&catalog());
        assert_eq!(result.lines().filter(|l| l.starts_with("- ")).count(), 60);
        assert!(result.contains("另有 1 条"));
        let text = format!("{}失败{}", "界".repeat(200), "后".repeat(200));
        assert!(excerpt(&text).len() <= 300);
        assert!(excerpt(&text).contains("失败"));
        let f = Fixture::new(
            "",
            &(0..60)
                .map(|_| format!("- B-3 {text}\n"))
                .collect::<String>(),
        );
        let result = f.run(&catalog());
        assert!(result.len() <= 12 * 1024);
        assert!(result.contains("另有 "));
    }
    #[test]
    fn gate_history_fallback_file_pattern() {
        let f = Fixture::new(
            "",
            "B-3 失败\n\na.spec.ts 失败\n\nb.test.mtsx timeout\n\nc.test.css 失败",
        );
        let result = f.run(&[]);
        assert!(result.contains("只按文件名式"));
        assert!(result.contains("共 2 条"));
        assert!(!result.contains("B-3"));
        let empty = Fixture::new("", "");
        assert!(empty.run(&[]).contains("没有匹配的记录"));
    }
}
