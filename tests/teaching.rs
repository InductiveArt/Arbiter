use arbiter::commands::ahead;
use arbiter::commands::stamped;
use arbiter::commands::written_by;
use arbiter::commands::COMMANDS;
use arbiter::commands::VERSION;

#[test]
fn the_skill_speaks_each_command_with_its_own_line() {
    let skill = include_str!("../share/skills/arbiter-quality/SKILL.md");
    for (call, line) in COMMANDS {
        let command = call.split_whitespace().next().unwrap_or(call);
        assert!(
            skill.contains(&format!("arbiter {command}")),
            "the skill never names {command}"
        );
        assert!(
            skill.contains(line),
            "the skill drifts from the command {command}: {line}"
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
fn a_gated_law_drills_but_is_not_findable() {
    use arbiter::core::pack::drillable;
    use arbiter::core::pack::findable;
    use arbiter::core::pack::CORE;
    use arbiter::core::pack::GATED;
    use arbiter::core::pack::RULES;
    for gated in GATED {
        assert!(drillable(gated), "{gated} names no rule");
        assert!(!findable(gated));
    }
    for (rule, _) in RULES.iter().chain(CORE) {
        if !GATED.contains(rule) {
            assert!(findable(rule), "{rule} should be findable");
        }
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
