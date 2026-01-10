use chrono::NaiveDate;
use date_time_parser::DateParser;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTaskInput {
    pub title: String,
    pub due_date: Option<NaiveDate>,
    pub tags: Vec<String>,
}

pub fn parse_task_input(raw: &str, today: NaiveDate) -> ParsedTaskInput {
    let (without_date_segments, due_date) = extract_date_and_strip(raw, today);

    let mut title_tokens = Vec::new();
    let mut tags = Vec::new();
    for token in without_date_segments.split_whitespace() {
        if let Some(tag) = normalize_tag_token(token) {
            if !tags.contains(&tag) {
                tags.push(tag);
            }
            continue;
        }
        title_tokens.push(token);
    }

    let cleaned = title_tokens.join(" ").trim().to_string();
    let title = if cleaned.is_empty() {
        raw.trim().to_string()
    } else {
        cleaned
    };

    ParsedTaskInput {
        title,
        due_date,
        tags,
    }
}

fn extract_date_and_strip(raw: &str, today: NaiveDate) -> (String, Option<NaiveDate>) {
    let mut remaining = String::with_capacity(raw.len());
    let mut due_date = None;
    let mut cursor = 0usize;

    while let Some(start_rel) = raw[cursor..].find('[') {
        let start = cursor + start_rel;
        remaining.push_str(&raw[cursor..start]);

        let after_start = start + 1;
        let Some(end_rel) = raw[after_start..].find(']') else {
            remaining.push_str(&raw[start..]);
            cursor = raw.len();
            break;
        };
        let end = after_start + end_rel;
        let candidate = raw[after_start..end].trim();

        if due_date.is_none() {
            due_date = DateParser::parse_relative(candidate, today);
        }

        if due_date.is_none() {
            remaining.push_str(&raw[start..=end]);
        }
        cursor = end + 1;
    }

    if cursor < raw.len() {
        remaining.push_str(&raw[cursor..]);
    }

    (remaining, due_date)
}

fn normalize_tag_token(token: &str) -> Option<String> {
    if !token.starts_with('#') {
        return None;
    }

    let trimmed = token[1..]
        .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
        .to_lowercase();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::parse_task_input;

    #[test]
    fn parses_due_date_and_tags() {
        let today = NaiveDate::from_ymd_opt(2026, 3, 7).expect("valid fixed date");
        let parsed = parse_task_input("Study [11:59 am tomorrow] #focus #School", today);

        assert_eq!(
            parsed.due_date,
            NaiveDate::from_ymd_opt(2026, 3, 8),
            "tomorrow should resolve relative to provided date"
        );
        assert_eq!(parsed.tags, vec!["focus".to_string(), "school".to_string()]);
        assert_eq!(parsed.title, "Study");
    }

    #[test]
    fn strips_punctuation_and_deduplicates_tags() {
        let today = NaiveDate::from_ymd_opt(2026, 3, 7).expect("valid fixed date");
        let parsed = parse_task_input("Fix login #P1, #backend. #p1", today);

        assert_eq!(parsed.due_date, None);
        assert_eq!(parsed.tags, vec!["p1".to_string(), "backend".to_string()]);
        assert_eq!(parsed.title, "Fix login");
    }

    #[test]
    fn only_parses_date_when_bracketed() {
        let today = NaiveDate::from_ymd_opt(2026, 3, 7).expect("valid fixed date");
        let parsed = parse_task_input("test tomorrow", today);

        assert_eq!(parsed.due_date, None);
        assert_eq!(parsed.title, "test tomorrow");
    }
}
