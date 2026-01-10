use chrono::NaiveDate;
use date_time_parser::DateParser;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTaskInput {
    pub title: String,
    pub due_date: Option<NaiveDate>,
    pub tags: Vec<String>,
}

pub fn parse_task_input(raw: &str, today: NaiveDate) -> ParsedTaskInput {
    let due_date = DateParser::parse_relative(raw, today);

    let mut title_tokens = Vec::new();
    let mut tags = Vec::new();
    for token in raw.split_whitespace() {
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
        let parsed = parse_task_input("Study by 11:59 am tomorrow #focus #School", today);

        assert_eq!(
            parsed.due_date,
            NaiveDate::from_ymd_opt(2026, 3, 8),
            "tomorrow should resolve relative to provided date"
        );
        assert_eq!(parsed.tags, vec!["focus".to_string(), "school".to_string()]);
        assert_eq!(parsed.title, "Study by 11:59 am tomorrow");
    }

    #[test]
    fn strips_punctuation_and_deduplicates_tags() {
        let today = NaiveDate::from_ymd_opt(2026, 3, 7).expect("valid fixed date");
        let parsed = parse_task_input("Fix login #P1, #backend. #p1", today);

        assert_eq!(parsed.due_date, None);
        assert_eq!(parsed.tags, vec!["p1".to_string(), "backend".to_string()]);
        assert_eq!(parsed.title, "Fix login");
    }
}
