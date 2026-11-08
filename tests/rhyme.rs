use arbiter::judge::rhyme::rhymes;
use arbiter::packs::graphed;

const RHYME: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/rhyme");

#[test]
fn bones_group_the_shapes_and_flesh_splits_perfect_from_slant() {
    let found = graphed(RHYME).expect("the fixture folder exists");
    let shown: Vec<String> = rhymes(&found)
        .iter()
        .map(|group| {
            format!(
                "{} {}",
                if group.perfect { "perfect" } else { "slant" },
                group
                    .places
                    .iter()
                    .map(|place| format!(
                        "{}:{} {}",
                        place.file.to_string_lossy(),
                        place.line,
                        place.title
                    ))
                    .collect::<Vec<String>>()
                    .join(", ")
            )
        })
        .collect();
    assert_eq!(
        shown,
        [
            "slant chorus.ts:1 chorus, verse.ts:1 verse",
            "perfect chorus.ts:12 anthem, verse.ts:12 anthem",
        ]
    );
}
