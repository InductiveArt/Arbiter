use std::collections::HashSet;

// The names a comment mentions that nothing in the file defines, uses or quotes.
// Only underscored words count as names: prose never writes them by accident.
pub fn overreaches(
    comments: &[(String, usize)],
    idents: &HashSet<String>,
    quoted: &str,
) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    for (comment, line) in comments {
        for token in tokens(comment) {
            if idents.contains(&token) || quoted.contains(&token) {
                continue;
            }
            found.push((token, *line));
        }
    }
    found
}

fn tokens(comment: &str) -> Vec<String> {
    comment
        .split(|part: char| !part.is_ascii_alphanumeric() && part != '_')
        .filter(|token| {
            token.len() > 2
                && token.contains('_')
                && !token.starts_with('_')
                && !token.ends_with('_')
        })
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::overreaches;

    #[test]
    fn a_name_the_file_reaches_passes_and_a_foreign_one_is_found() {
        let comments = vec![(
            "# fed by dashboard_frontend after test_files".to_string(),
            4,
        )];
        let idents: HashSet<String> = ["test_files".to_string()].into_iter().collect();
        let found = overreaches(&comments, &idents, "");
        assert_eq!(found, [("dashboard_frontend".to_string(), 4)]);
    }

    #[test]
    fn a_name_quoted_in_a_string_is_within_reach() {
        let comments = vec![("# read from PY_HARNESS_VERDICT".to_string(), 1)];
        let found = overreaches(
            &comments,
            &HashSet::new(),
            "os.environ['PY_HARNESS_VERDICT']",
        );
        assert!(found.is_empty());
    }
}
