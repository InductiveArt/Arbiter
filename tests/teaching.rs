use arbiter::verbs::VERBS;

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
