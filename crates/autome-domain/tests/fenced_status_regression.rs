use autome_domain::status_block::{self, DocStatus, ParseError};

const HEADER: &str = "status: 设计中\ndesign-round: 0/3\nimplementation-round: 0/0\ncurrent-milestone: 无\ncurrent-milestone-reopens: 0\nconvergence-mode: normal\nnext-action: 无\n";

fn check_fenced_header(fence: &str) {
    let bare = format!("# 任务\n{HEADER}\n## 背景\n测试任务\n");
    let expected = status_block::parse(&bare).expect("无围栏对照必须有效");
    let fenced = format!("# 任务\n{fence}\n{HEADER}```\n\n## 背景\n测试任务\n");

    // The task permits either accepting this presentation or rejecting it
    // with an actionable fence diagnostic. Do not choose a repair policy here.
    match status_block::parse(&fenced) {
        Ok(actual) => assert_eq!(actual, expected, "接受围栏时必须保留完整解析结果"),
        Err(error) => {
            let message = error.to_string();
            assert!(
                message.contains("代码围栏") && message.contains("第 2 行"),
                "拒绝头部围栏时应指出代码围栏及其起始行，实际：{message}"
            );
        }
    }
}

#[test]
fn bare_fence_is_accepted_or_reports_its_location() {
    check_fenced_header("```");
}

#[test]
fn text_fence_is_accepted_or_reports_its_location() {
    check_fenced_header("```text");
}

#[test]
fn genuinely_missing_status_is_still_rejected() {
    let doc = format!("# 任务\n{}", HEADER.replace("status: 设计中\n", ""));
    assert_eq!(
        status_block::parse(&doc).unwrap_err(),
        ParseError::MissingField {
            field: "status".to_string()
        }
    );
}

#[test]
fn body_examples_do_not_replace_the_real_header() {
    let doc =
        format!("# 任务\n{HEADER}\n## 示例\n```text\nstatus: 已完成\ndesign-round: 9/15\n```\n");
    let parsed = status_block::parse(&doc).unwrap();
    assert_eq!(parsed.status, DocStatus::Designing);
    assert_eq!(parsed.design_round, 0);
}
