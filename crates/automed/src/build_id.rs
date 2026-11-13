//! Build provenance and conservative ordering of ledger writers.
use std::{fmt, str::FromStr};

pub const BUILD_ID: &str = env!("AUTOME_BUILD_ID");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildId {
    Git {
        count: u32,
        sha: String,
        text: String,
    },
    Unknown(String),
}

fn timestamp(text: &str) -> bool {
    text.len() == 16
        && text.as_bytes()[8] == b'T'
        && text.as_bytes()[15] == b'Z'
        && text
            .bytes()
            .enumerate()
            .all(|(i, b)| i == 8 || i == 15 || b.is_ascii_digit())
}

impl FromStr for BuildId {
    type Err = &'static str;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if !text.is_ascii() {
            return Err("构建标识必须是 ASCII");
        }
        if let Some(rest) = text.strip_prefix("unknown-") {
            if let Some((version, time)) = rest.rsplit_once('-')
                && !version.is_empty()
                && !version.contains('\n')
                && timestamp(time)
            {
                return Ok(Self::Unknown(text.to_owned()));
            }
        } else if text.len() == 37 {
            let bytes = text.as_bytes();
            if bytes[7] == b'-'
                && bytes[24] == b'-'
                && bytes[..7].iter().all(u8::is_ascii_digit)
                && timestamp(&text[8..24])
                && bytes[25..]
                    .iter()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
            {
                return Ok(Self::Git {
                    count: text[..7].parse().unwrap(),
                    sha: text[25..].into(),
                    text: text.into(),
                });
            }
        }
        Err("构建标识格式无效")
    }
}

impl fmt::Display for BuildId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Git { text, .. } | Self::Unknown(text) => f.write_str(text),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Advance,
    Same,
    Older,
    Incomparable,
}

pub fn verdict(current: &str, recorded: Option<&str>) -> Verdict {
    let Some(recorded) = recorded else {
        return Verdict::Advance;
    };
    let (Ok(c), Ok(m)) = (current.parse::<BuildId>(), recorded.parse::<BuildId>()) else {
        return Verdict::Older;
    };
    if current == recorded {
        return Verdict::Same;
    }
    match (c, m) {
        (BuildId::Git { count: c, .. }, BuildId::Git { count: m, .. }) => match c.cmp(&m) {
            std::cmp::Ordering::Greater => Verdict::Advance,
            std::cmp::Ordering::Less => Verdict::Older,
            std::cmp::Ordering::Equal => Verdict::Incomparable,
        },
        (BuildId::Git { .. }, BuildId::Unknown(_)) => Verdict::Advance,
        _ => Verdict::Older,
    }
}

/// Return a test identity only when it cannot raise the compiled identity.
pub fn effective_override<'a>(compiled: &'a str, requested: &'a str) -> Option<&'a str> {
    (requested.parse::<BuildId>().is_ok()
        && matches!(
            verdict(requested, Some(compiled)),
            Verdict::Same | Verdict::Older
        ))
    .then_some(requested)
}

pub fn older_kernel_message(current: &str, recorded: &str) -> String {
    format!(
        "这个内核（{current}）比推进过账本的内核（{recorded}）旧，已停止推进；请重新安装或设置 AUTOMED_BIN"
    )
}

#[cfg(test)]
#[path = "../build_support.rs"]
mod build_support;
#[cfg(test)]
mod tests;
