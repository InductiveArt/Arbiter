use arbiter::verbs::ahead;
use arbiter::verbs::stamped;
use arbiter::verbs::written_by;
use arbiter::verbs::VERBS;
use arbiter::verbs::VERSION;

#[test]
fn the_skill_speaks_each_verb_with_its_own_line() {
    let skill = include_str!("../share/skills/arbiter-quality/SKILL.md");
    for (call, line) in VERBS {
        let verb = call.split_whitespace().next().unwrap_or(call);
        assert!(
            skill.contains(&format!("arbiter {verb}")),
            "the skill never names {verb}"
        );
        assert!(
            skill.contains(line),
            "the skill drifts from the verb {verb}: {line}"
        );
    }
}

#[test]
fn every_rule_sits_in_exactly_one_group() {
    use arbiter::core::pack::law;
    use arbiter::core::pack::CORE;
    use arbiter::core::pack::GROUPS;
    use arbiter::core::pack::RULES;
    let mut counts = std::collections::HashMap::new();
    for (_, members) in GROUPS {
        for member in *members {
            assert!(law(member).is_some(), "{member} names no rule");
            *counts.entry(*member).or_insert(0) += 1;
        }
    }
    for (rule, _) in RULES.iter().chain(CORE) {
        assert_eq!(
            counts.get(rule),
            Some(&1),
            "{rule} sits in no group, or in two"
        );
    }
}

#[test]
fn a_stamp_names_the_arbiter_that_wrote_the_file() {
    let page = stamped("SKILL.md", "---\nname: x\n---\nthe law\n");
    let hook = stamped("arbiter-stop.sh", "#!/bin/bash\nexit 0\n");
    assert!(page
        .trim_end()
        .ends_with(&format!("<!-- arbiter {VERSION} -->")));
    assert!(hook.trim_end().ends_with(&format!("# arbiter {VERSION}")));
    assert_eq!(written_by(&page), written_by(&hook));
    assert!(written_by(&page).is_some());
    assert_eq!(written_by("no stamp at all\n"), None);
}

#[test]
fn a_file_from_a_newer_arbiter_is_never_written_backwards() {
    assert!(ahead("the law\n<!-- arbiter 9999.0.0 -->\n"));
    assert!(!ahead("the law\n<!-- arbiter 0.0.1 -->\n"));
    assert!(!ahead(&stamped("SKILL.md", "the law\n")));
    assert!(!ahead("no stamp at all\n"));
}
