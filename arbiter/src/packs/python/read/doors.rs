use tree_sitter::Node;

use crate::packs::python::reading::Asking;
use crate::packs::python::reading::Assign;
use crate::packs::python::reading::CallArg;
use crate::packs::python::reading::Class;
use crate::packs::python::reading::Doors;

// Collects the door shapes: protocol classes, each class's bases, annotated
// parameters, and the constructors standing at annotated sites.
pub fn doors(node: Node, source: &str, found: &mut Doors) {
    let text = |at: Node| source[at.byte_range()].to_string();
    match node.kind() {
        "class_definition" => {
            let Some(name) = node.child_by_field_name("name") else {
                return;
            };
            let mut bases = Vec::new();
            if let Some(supers) = node.child_by_field_name("superclasses") {
                for index in 0..supers.named_child_count() {
                    if let Some(base) = supers.named_child(index) {
                        let whole = text(base);
                        let last = whole.rsplit('.').next().unwrap_or(&whole);
                        bases.push(last.split('[').next().unwrap_or(last).to_string());
                    }
                }
            }
            if bases.iter().any(|base| base == "Protocol") {
                found.protocols.push(text(name));
            }
            found.classes.push(Class {
                name: text(name),
                bases,
                line: node.start_position().row + 1,
            });
        }
        "function_definition" => {
            let Some(name) = node.child_by_field_name("name") else {
                return;
            };
            if let Some(parameters) = node.child_by_field_name("parameters") {
                let mut place = 0;
                for index in 0..parameters.named_child_count() {
                    let Some(parameter) = parameters.named_child(index) else {
                        continue;
                    };
                    let kind = parameter.kind();
                    if kind == "typed_parameter" || kind == "typed_default_parameter" {
                        let keyword = parameter
                            .named_child(0)
                            .filter(|child| child.kind() == "identifier")
                            .map(text);
                        if let Some(annotation) = parameter.child_by_field_name("type") {
                            let names = type_names(annotation, source);
                            if !names.is_empty() {
                                found.askings.push(Asking {
                                    function: text(name),
                                    place,
                                    keyword,
                                    types: names,
                                });
                            }
                        }
                    }
                    place += 1;
                }
            }
        }
        "assignment" => {
            if let (Some(annotation), Some(right)) = (
                node.child_by_field_name("type"),
                node.child_by_field_name("right"),
            ) {
                if let Some(constructor) = constructed(right, source) {
                    found.assigns.push(Assign {
                        types: type_names(annotation, source),
                        constructor,
                    });
                }
            }
        }
        "call" => {
            if let (Some(function), Some(arguments)) = (
                node.child_by_field_name("function"),
                node.child_by_field_name("arguments"),
            ) {
                if function.kind() == "identifier" {
                    let callee = text(function);
                    let mut place = 0;
                    for index in 0..arguments.named_child_count() {
                        let Some(argument) = arguments.named_child(index) else {
                            continue;
                        };
                        if argument.kind() == "keyword_argument" {
                            if let (Some(key), Some(value)) = (
                                argument.child_by_field_name("name"),
                                argument.child_by_field_name("value"),
                            ) {
                                if let Some(constructor) = constructed(value, source) {
                                    found.callargs.push(CallArg {
                                        callee: callee.clone(),
                                        place: usize::MAX,
                                        keyword: Some(text(key)),
                                        constructor,
                                    });
                                }
                            }
                            continue;
                        }
                        if let Some(constructor) = constructed(argument, source) {
                            found.callargs.push(CallArg {
                                callee: callee.clone(),
                                place,
                                keyword: None,
                                constructor,
                            });
                        }
                        place += 1;
                    }
                }
            }
        }
        _ => {}
    }
    for index in 0..node.named_child_count() {
        if let Some(child) = node.named_child(index) {
            doors(child, source, found);
        }
    }
}

// A direct constructor: a call whose function is a bare capitalized name.
fn constructed(node: Node, source: &str) -> Option<String> {
    if node.kind() != "call" {
        return None;
    }
    let function = node.child_by_field_name("function")?;
    if function.kind() != "identifier" {
        return None;
    }
    let name = source[function.byte_range()].to_string();
    name.chars()
        .next()
        .is_some_and(char::is_uppercase)
        .then_some(name)
}

// Every identifier a type annotation names, so a union or an Optional still
// speaks each of its doors.
fn type_names(node: Node, source: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut stack = vec![node];
    while let Some(at) = stack.pop() {
        if at.kind() == "identifier" {
            names.push(source[at.byte_range()].to_string());
        }
        for index in 0..at.named_child_count() {
            if let Some(child) = at.named_child(index) {
                stack.push(child);
            }
        }
    }
    names
}
