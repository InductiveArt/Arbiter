use std::mem;

// One `.gitignore` file's lines as rules, in the file's order. Names are
// matched as bytes and in their own case: a name on disk is bytes, and no
// setting says here that case does not count.
pub struct Gitignore {
    rules: Vec<Rule>,
}

struct Rule {
    // A `!` opened the line: a match takes the path back in.
    back: bool,
    // A slash closed the line: the rule speaks of folders only.
    folders: bool,
    pattern: Pattern,
}

enum Pattern {
    // No slash before the end: one name, wherever it sits.
    Name(Vec<Token>),
    // A slash before the end: the whole path from the file's own folder.
    Path(Vec<Step>),
}

enum Step {
    // Two stars standing as a whole name: any number of names, none included.
    AnyDepth,
    Name(Vec<Token>),
}

enum Token {
    Byte(u8),
    AnyByte,
    AnyRun,
    Set(Set),
}

struct Set {
    negated: bool,
    members: Vec<Member>,
}

enum Member {
    Byte(u8),
    Range(u8, u8),
    Class(Class),
}

enum Class {
    Alnum,
    Alpha,
    Blank,
    Cntrl,
    Digit,
    Graph,
    Lower,
    Print,
    Punct,
    Space,
    Upper,
    Xdigit,
}

impl Gitignore {
    pub fn parsed(text: &[u8]) -> Gitignore {
        let text = text.strip_prefix(b"\xef\xbb\xbf").unwrap_or(text);
        Gitignore {
            rules: text.split(|byte| *byte == b'\n').filter_map(rule).collect(),
        }
    }

    // What the last rule matching the path says: left out, taken back in, or
    // nothing where no rule matches. The path is the names from the file's
    // own folder down to the one asked about.
    pub fn ignores(&self, path: &[&[u8]], folder: bool) -> Option<bool> {
        self.rules
            .iter()
            .rev()
            .find(|rule| rule.matches(path, folder))
            .map(|rule| !rule.back)
    }
}

impl Rule {
    fn matches(&self, path: &[&[u8]], folder: bool) -> bool {
        if self.folders && !folder {
            return false;
        }
        match &self.pattern {
            Pattern::Name(tokens) => path.last().is_some_and(|name| named(tokens, name)),
            Pattern::Path(steps) => walked(steps, path),
        }
    }
}

impl Set {
    fn holds(&self, byte: u8) -> bool {
        let among = self.members.iter().any(|member| match member {
            Member::Byte(one) => *one == byte,
            Member::Range(from, to) => (*from..=*to).contains(&byte),
            Member::Class(class) => class.holds(byte),
        });
        among != self.negated
    }
}

impl Class {
    fn known(name: &[u8]) -> Option<Class> {
        Some(match name {
            b"alnum" => Class::Alnum,
            b"alpha" => Class::Alpha,
            b"blank" => Class::Blank,
            b"cntrl" => Class::Cntrl,
            b"digit" => Class::Digit,
            b"graph" => Class::Graph,
            b"lower" => Class::Lower,
            b"print" => Class::Print,
            b"punct" => Class::Punct,
            b"space" => Class::Space,
            b"upper" => Class::Upper,
            b"xdigit" => Class::Xdigit,
            _ => return None,
        })
    }

    fn holds(&self, byte: u8) -> bool {
        match self {
            Class::Alnum => byte.is_ascii_alphanumeric(),
            Class::Alpha => byte.is_ascii_alphabetic(),
            Class::Blank => matches!(byte, b' ' | b'\t'),
            Class::Cntrl => byte.is_ascii_control(),
            Class::Digit => byte.is_ascii_digit(),
            Class::Graph => byte.is_ascii_graphic(),
            Class::Lower => byte.is_ascii_lowercase(),
            Class::Print => byte == b' ' || byte.is_ascii_graphic(),
            Class::Punct => byte.is_ascii_punctuation(),
            Class::Space => matches!(byte, b' ' | b'\t' | b'\n' | b'\r'),
            Class::Upper => byte.is_ascii_uppercase(),
            Class::Xdigit => byte.is_ascii_hexdigit(),
        }
    }
}

// One line as a rule, or nothing: a comment, a blank line, or a pattern that
// cannot be read and so matches nothing.
fn rule(line: &[u8]) -> Option<Rule> {
    if line.first() == Some(&b'#') {
        return None;
    }
    let line = unpadded(line.strip_suffix(b"\r").unwrap_or(line));
    let (back, line) = match line.strip_prefix(b"!") {
        Some(rest) => (true, rest),
        None => (false, line),
    };
    let (folders, line) = match line.strip_suffix(b"/") {
        Some(rest) => (true, rest),
        None => (false, line),
    };
    let pattern = if line.contains(&b'/') {
        let line = line.strip_prefix(b"/").unwrap_or(line);
        Pattern::Path(names(line)?.into_iter().map(step).collect())
    } else {
        Pattern::Name(names(line)?.pop()?)
    };
    Some(Rule {
        back,
        folders,
        pattern,
    })
}

// The line less its closing spaces, a space after a backslash being a
// character of the pattern.
fn unpadded(line: &[u8]) -> &[u8] {
    let mut spaces: Option<usize> = None;
    let mut at = 0;
    while at < line.len() {
        match line[at] {
            b' ' => spaces = spaces.or(Some(at)),
            b'\\' => {
                at += 1;
                spaces = None;
            }
            _ => spaces = None,
        }
        at += 1;
    }
    match spaces {
        Some(from) => &line[..from],
        None => line,
    }
}

// The pattern as names, cut at each slash, or nothing where it cannot be
// read: a backslash escaping nothing, a set never closed, a class unknown.
fn names(pattern: &[u8]) -> Option<Vec<Vec<Token>>> {
    let mut names = Vec::new();
    let mut name = Vec::new();
    let mut at = 0;
    while at < pattern.len() {
        match pattern[at] {
            b'/' => names.push(mem::take(&mut name)),
            b'\\' => {
                at += 1;
                match *pattern.get(at)? {
                    b'/' => names.push(mem::take(&mut name)),
                    byte => name.push(Token::Byte(byte)),
                }
            }
            b'*' => name.push(Token::AnyRun),
            b'?' => name.push(Token::AnyByte),
            b'[' => {
                let (set, after) = set(pattern, at + 1)?;
                name.push(Token::Set(set));
                at = after;
                continue;
            }
            byte => name.push(Token::Byte(byte)),
        }
        at += 1;
    }
    names.push(name);
    Some(names)
}

// The set whose opening bracket sits just before `at`, and where the pattern
// goes on after its closing one. The first member is read before a closing
// bracket is looked for, so a bracket placed first is a member.
fn set(pattern: &[u8], mut at: usize) -> Option<(Set, usize)> {
    let negated = matches!(pattern.get(at), Some(b'!' | b'^'));
    if negated {
        at += 1;
    }
    let mut members = Vec::new();
    // The byte a dash would open a range from: none after a range or a class.
    let mut before: Option<u8> = None;
    loop {
        match *pattern.get(at)? {
            b'\\' => {
                at += 1;
                let escaped = *pattern.get(at)?;
                members.push(Member::Byte(escaped));
                before = Some(escaped);
            }
            b'-' if before.is_some() && pattern.get(at + 1).is_some_and(|next| *next != b']') => {
                at += 1;
                let mut to = pattern[at];
                if to == b'\\' {
                    at += 1;
                    to = *pattern.get(at)?;
                }
                members.push(Member::Range(before?, to));
                before = None;
            }
            b'[' if pattern.get(at + 1) == Some(&b':') => {
                let from = at + 2;
                let close = from + pattern[from..].iter().position(|byte| *byte == b']')?;
                if close > from && pattern[close - 1] == b':' {
                    members.push(Member::Class(Class::known(&pattern[from..close - 1])?));
                    before = None;
                    at = close;
                } else {
                    members.push(Member::Byte(b'['));
                    before = Some(b'[');
                }
            }
            byte => {
                members.push(Member::Byte(byte));
                before = Some(byte);
            }
        }
        at += 1;
        if pattern.get(at) == Some(&b']') {
            return Some((Set { negated, members }, at + 1));
        }
    }
}

fn step(name: Vec<Token>) -> Step {
    let stars = name.iter().all(|token| matches!(token, Token::AnyRun));
    if stars && name.len() >= 2 {
        Step::AnyDepth
    } else {
        Step::Name(name)
    }
}

// Whether the tokens match the whole name. A run that took too little is
// lengthened one byte at a time and only the last run met is ever revisited,
// so the work stays within the name's length times the pattern's.
fn named(tokens: &[Token], name: &[u8]) -> bool {
    let mut token = 0;
    let mut byte = 0;
    let mut run: Option<(usize, usize)> = None;
    while byte < name.len() {
        let fits = match tokens.get(token) {
            Some(Token::AnyRun) => {
                run = Some((token + 1, byte));
                token += 1;
                continue;
            }
            Some(Token::Byte(one)) => *one == name[byte],
            Some(Token::AnyByte) => true,
            Some(Token::Set(set)) => set.holds(name[byte]),
            None => false,
        };
        if fits {
            token += 1;
            byte += 1;
        } else if let Some((after, from)) = run {
            run = Some((after, from + 1));
            token = after;
            byte = from + 1;
        } else {
            return false;
        }
    }
    tokens[token..]
        .iter()
        .all(|token| matches!(token, Token::AnyRun))
}

// Whether the steps match the whole path, the same way a name is matched:
// any depth is a run of names. Closing the pattern, it takes every name left
// and needs one, so it speaks of what a folder holds and never of the folder.
fn walked(steps: &[Step], path: &[&[u8]]) -> bool {
    let mut step = 0;
    let mut part = 0;
    let mut run: Option<(usize, usize)> = None;
    while part < path.len() {
        let fits = match steps.get(step) {
            Some(Step::AnyDepth) if step + 1 == steps.len() => return true,
            Some(Step::AnyDepth) => {
                run = Some((step + 1, part));
                step += 1;
                continue;
            }
            Some(Step::Name(tokens)) => named(tokens, path[part]),
            None => false,
        };
        if fits {
            step += 1;
            part += 1;
        } else if let Some((after, from)) = run {
            run = Some((after, from + 1));
            step = after;
            part = from + 1;
        } else {
            return false;
        }
    }
    step == steps.len()
}

#[cfg(test)]
mod tests {
    use super::Gitignore;

    fn says(text: &str, path: &str, folder: bool) -> Option<bool> {
        let parts: Vec<&[u8]> = path.split('/').map(str::as_bytes).collect();
        Gitignore::parsed(text.as_bytes()).ignores(&parts, folder)
    }

    #[test]
    fn a_blank_line_and_a_comment_give_no_rule_and_an_escaped_hash_is_a_name() {
        assert_eq!(says("\n# note\n", "note", false), None);
        assert_eq!(says("#x\n", "#x", false), None);
        assert_eq!(says("\\#x\n", "#x", false), Some(true));
    }

    #[test]
    fn a_pattern_with_no_slash_matches_a_name_at_any_depth() {
        assert_eq!(says("*.log\n", "a.log", false), Some(true));
        assert_eq!(says("*.log\n", "deep/er/a.log", false), Some(true));
        assert_eq!(says("*.log\n", "a.logs", false), None);
        assert_eq!(says("build\n", "x/build", true), Some(true));
        assert_eq!(says("build\n", "x/build", false), Some(true));
    }

    #[test]
    fn a_name_is_matched_in_its_own_case() {
        assert_eq!(says("build\n", "build", true), Some(true));
        assert_eq!(says("build\n", "Build", true), None);
        assert_eq!(says("*.LOG\n", "a.log", false), None);
    }

    #[test]
    fn a_closing_slash_speaks_of_folders_only() {
        assert_eq!(says("out/\n", "out", true), Some(true));
        assert_eq!(says("out/\n", "out", false), None);
        assert_eq!(says("out/\n", "deep/out", true), Some(true));
    }

    #[test]
    fn a_slash_before_the_end_ties_the_pattern_to_the_file_s_own_folder() {
        assert_eq!(says("/build\n", "build", true), Some(true));
        assert_eq!(says("/build\n", "deep/build", true), None);
        assert_eq!(says("docs/draft\n", "docs/draft", false), Some(true));
        assert_eq!(says("docs/draft\n", "x/docs/draft", false), None);
        assert_eq!(says("docs/*.md\n", "docs/a.md", false), Some(true));
        assert_eq!(
            says("docs/*.md\n", "docs/x/a.md", false),
            None,
            "a star stays within one name"
        );
        assert_eq!(says("/docs/\n", "docs", true), Some(true));
        assert_eq!(says("/docs/\n", "docs", false), None);
    }

    #[test]
    fn a_star_a_question_mark_and_a_set_match_within_one_name() {
        assert_eq!(says("a?c\n", "abc", false), Some(true));
        assert_eq!(says("a?c\n", "ac", false), None);
        assert_eq!(says("[ab]x\n", "ax", false), Some(true));
        assert_eq!(says("[ab]x\n", "cx", false), None);
        assert_eq!(says("[!ab]x\n", "cx", false), Some(true));
        assert_eq!(says("[!ab]x\n", "ax", false), None);
        assert_eq!(says("[^ab]x\n", "cx", false), Some(true));
        assert_eq!(says("[a-c]1\n", "b1", false), Some(true));
        assert_eq!(says("[a-c]1\n", "d1", false), None);
        assert_eq!(says("[[:digit:]]x\n", "7x", false), Some(true));
        assert_eq!(says("[[:digit:]]x\n", "ax", false), None);
        assert_eq!(
            says("[]a]x\n", "]x", false),
            Some(true),
            "a closing bracket that opens the set is one of its members"
        );
        assert_eq!(says("a*b*c\n", "aXbYc", false), Some(true));
        assert_eq!(says("a*b*c\n", "aXbY", false), None);
        assert_eq!(
            says("*a*a*b\n", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaac", false),
            None
        );
        assert_eq!(says("\\*\n", "*", false), Some(true));
        assert_eq!(says("\\*\n", "a", false), None);
        assert_eq!(says("*\n", ".hidden", false), Some(true));
    }

    #[test]
    fn two_stars_as_a_whole_name_stand_for_any_depth() {
        assert_eq!(says("**/foo\n", "foo", false), Some(true));
        assert_eq!(says("**/foo\n", "a/b/foo", false), Some(true));
        assert_eq!(says("foo/**\n", "foo/a", false), Some(true));
        assert_eq!(says("foo/**\n", "foo/a/b", false), Some(true));
        assert_eq!(
            says("foo/**\n", "foo", true),
            None,
            "what is inside the folder, never the folder"
        );
        assert_eq!(says("a/**/b\n", "a/b", false), Some(true));
        assert_eq!(says("a/**/b\n", "a/x/y/b", false), Some(true));
        assert_eq!(says("a/**/b\n", "a/x/c", false), None);
        assert_eq!(says("a/**/b/**/c\n", "a/b/x/b/y/c", false), Some(true));
        assert_eq!(says("a**b/c\n", "aXXb/c", false), Some(true));
        assert_eq!(
            says("a**b/c\n", "a/b/c", false),
            None,
            "two stars inside a name are one star"
        );
        assert_eq!(says("**\n", "a", false), Some(true));
    }

    #[test]
    fn the_last_line_that_matches_decides_and_a_bang_takes_a_path_back() {
        assert_eq!(says("*.log\n!keep.log\n", "keep.log", false), Some(false));
        assert_eq!(says("*.log\n!keep.log\n", "x.log", false), Some(true));
        assert_eq!(says("!keep.log\n*.log\n", "keep.log", false), Some(true));
        assert_eq!(says("!keep.log\n", "other", false), None);
        assert_eq!(says("\\!x\n", "!x", false), Some(true));
    }

    #[test]
    fn closing_spaces_are_dropped_unless_escaped_and_a_line_may_end_either_way() {
        assert_eq!(says("name   \n", "name", false), Some(true));
        assert_eq!(says("name\\ \n", "name ", false), Some(true));
        assert_eq!(says("name\\ \n", "name", false), None);
        assert_eq!(says(" name\n", "name", false), None);
        assert_eq!(says("a.log\r\nb.log\r\n", "a.log", false), Some(true));
        assert_eq!(says("a.log\r\nb.log\r\n", "b.log", false), Some(true));
        assert_eq!(says("a.log", "a.log", false), Some(true));
    }

    #[test]
    fn a_byte_order_mark_opening_the_file_is_not_part_of_its_first_pattern() {
        assert_eq!(says("\u{feff}a.log\n", "a.log", false), Some(true));
    }

    #[test]
    fn a_pattern_that_cannot_be_read_matches_nothing() {
        assert_eq!(says("[ab\n", "a", false), None);
        assert_eq!(
            says("[ab\nok\n", "ok", false),
            Some(true),
            "the lines around it still speak"
        );
        assert_eq!(says("[ab\n", "[ab", false), None);
        assert_eq!(says("name\\\n", "name", false), None);
        assert_eq!(says("name\\\n", "name\\", false), None);
        assert_eq!(says("[[:nope:]]x\n", "ax", false), None);
        assert_eq!(says("!\n", "a", false), None);
        assert_eq!(says("/\n", "a", true), None);
    }
}
