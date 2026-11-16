use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

use crate::core::pack::Repo;
use crate::domain::finding::Finding;
use crate::packs::typescript::reading::Asking;
use crate::packs::typescript::reading::Reading;

// A class standing where an interface is asked names the interface: a shape
// satisfied silently is an edge the graph cannot see.
pub fn instances(readings: &[Reading], repo: &Repo) -> Vec<Finding> {
    let doors: HashSet<&String> = readings
        .iter()
        .flat_map(|reading| &reading.doors.interfaces)
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
        let mut walked = HashSet::new();
        while let Some(name) = stack.pop() {
            if !walked.insert(name.to_string()) {
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
    let fixture = |file: &PathBuf| {
        let unit = repo.unit_of(file);
        repo.is_test(unit, &repo.units[unit].path.clone()) || repo.is_test(unit, file)
    };
    let mut unnamed = BTreeSet::new();
    let mut findings = Vec::new();
    let mut undeclared_door =
        |types: &Vec<String>, construction: &String, findings: &mut Vec<Finding>| {
            for asked in types {
                if !doors.contains(asked) || asked == construction {
                    continue;
                }
                let Some((place_file, place_line, _)) = places.get(construction.as_str()) else {
                    continue;
                };
                if reaches(construction, asked) {
                    continue;
                }
                if unnamed.insert((construction.clone(), asked.clone())) {
                    findings.push(Finding::integrity(
                        place_file,
                        *place_line,
                        "undeclared-protocol",
                        format!(
                            "{construction} stands where {asked} is asked and names no \
                         such base; a protocol satisfied by shape is still named"
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
            undeclared_door(&assign.types, &assign.construction, &mut findings);
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
                if asking.function == callarg.callee && asking.place == callarg.place {
                    undeclared_door(&asking.types, &callarg.construction, &mut findings);
                }
            }
        }
    }
    findings
}
