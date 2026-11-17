use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

use crate::core::repository::is_test;
use crate::core::repository::Repo;
use crate::domain::finding::Finding;
use crate::packs::python::reading::Asking;
use crate::packs::python::reading::Reading;

// A class standing where a protocol is asked names the protocol: a shape
// satisfied silently is an edge the graph cannot see.
pub fn instances(readings: &[Reading], repo: &Repo) -> Vec<Finding> {
    let doors: HashSet<&String> = readings
        .iter()
        .flat_map(|reading| &reading.doors.protocols)
        .collect();
    let places: HashMap<&str, (&PathBuf, usize, &Vec<String>)> = readings
        .iter()
        .flat_map(|reading| {
            reading.doors.classes.iter().map(move |class| {
                (
                    class.name.as_str(),
                    (&reading.file, class.line, &class.bases),
                )
            })
        })
        .collect();
    let reaches = |class: &str, asked: &str| -> bool {
        let mut stack = vec![class];
        let mut seen = HashSet::new();
        while let Some(name) = stack.pop() {
            if !seen.insert(name.to_string()) {
                continue;
            }
            let Some((_, _, bases)) = places.get(name) else {
                continue;
            };
            for base in bases.iter() {
                if base == asked {
                    return true;
                }
                stack.push(base);
            }
        }
        false
    };
    let mut unnamed = BTreeSet::new();
    let fixture = |file: &PathBuf| {
        let unit = repo.unit_of(file);
        is_test(&repo.units[unit].path.clone()) || is_test(file)
    };
    let mut findings = Vec::new();
    let mut undeclared_door =
        |types: &Vec<String>, constructor: &String, findings: &mut Vec<Finding>| {
            for asked in types {
                if !doors.contains(asked) || asked == constructor {
                    continue;
                }
                let Some((place_file, place_line, _)) = places.get(constructor.as_str()) else {
                    continue;
                };
                if reaches(constructor, asked) {
                    continue;
                }
                if unnamed.insert((constructor.clone(), asked.clone())) {
                    findings.push(Finding::integrity(
                        place_file,
                        *place_line,
                        "undeclared-protocol",
                        format!(
                            "{constructor} stands where {asked} is asked and names no such \
                         base; a protocol satisfied by shape is still named"
                        ),
                    ));
                }
            }
        };
    for reading in readings {
        if fixture(&reading.file) {
            continue;
        }
        for assign in &reading.doors.assigns {
            undeclared_door(&assign.types, &assign.constructor, &mut findings);
        }
    }
    let askings: Vec<&Asking> = readings
        .iter()
        .flat_map(|reading| &reading.doors.askings)
        .collect();
    for reading in readings {
        if fixture(&reading.file) {
            continue;
        }
        for callarg in &reading.doors.callargs {
            for asking in &askings {
                if asking.function != callarg.callee {
                    continue;
                }
                let fits = match &callarg.keyword {
                    Some(keyword) => asking.keyword.as_ref() == Some(keyword),
                    None => asking.place == callarg.place,
                };
                if fits {
                    undeclared_door(&asking.types, &callarg.constructor, &mut findings);
                }
            }
        }
    }
    findings
}
