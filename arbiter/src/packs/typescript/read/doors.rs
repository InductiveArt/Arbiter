use crate::packs::typescript::reading::Asking;
use crate::packs::typescript::reading::Assign;
use crate::packs::typescript::reading::CallArg;
use crate::packs::typescript::reading::Class;
use crate::packs::typescript::reading::Doors;

// Collects the door shapes: interfaces, each class's named heritage,
// annotated parameters, and the constructions standing at annotated sites.
pub fn doors(node: tree_sitter::Node, source: &str, found: &mut Doors) {
    let text = |at: tree_sitter::Node| source[at.byte_range()].to_string();
    match node.kind() {
        "interface_declaration" => {
            if let Some(name) = node.child_by_field_name("name") {
                found.interfaces.push(text(name));
            }
        }
        "class_declaration" => {
            if let Some(name) = node.child_by_field_name("name") {
                let mut bases = Vec::new();
                let mut stack = vec![node];
                while let Some(at) = stack.pop() {
                    match at.kind() {
                        "extends_clause" | "implements_clause" => {
                            for index in 0..at.named_child_count() {
                                if let Some(base) = at.named_child(index) {
                                    let whole = text(base);
                                    let first = whole.split('<').next().unwrap_or(&whole);
                                    bases.push(
                                        first.rsplit('.').next().unwrap_or(first).to_string(),
                                    );
                                }
                            }
                        }
                        "class_body" => {}
                        _ => {
                            for index in 0..at.named_child_count() {
                                if let Some(child) = at.named_child(index) {
                                    if child.kind() != "class_body" {
                                        stack.push(child);
                                    }
                                }
                            }
                        }
                    }
                }
                found.classes.push(Class {
                    name: text(name),
                    bases,
                    line: node.start_position().row + 1,
                });
            }
        }
        "function_declaration" => {
            if let (Some(name), Some(parameters)) = (
                node.child_by_field_name("name"),
                node.child_by_field_name("parameters"),
            ) {
                let mut place = 0;
                for index in 0..parameters.named_child_count() {
                    let Some(parameter) = parameters.named_child(index) else {
                        continue;
                    };
                    if let Some(annotation) = parameter.child_by_field_name("type") {
                        let names = annotation_names(annotation, source);
                        if !names.is_empty() {
                            found.askings.push(Asking {
                                function: text(name),
                                place,
                                types: names,
                            });
                        }
                    }
                    place += 1;
                }
            }
        }
        "variable_declarator" => {
            if let (Some(annotation), Some(value)) = (
                node.child_by_field_name("type"),
                node.child_by_field_name("value"),
            ) {
                if let Some(construction) = constructed_ts(value, source) {
                    found.assigns.push(Assign {
                        types: annotation_names(annotation, source),
                        construction,
                    });
                }
            }
        }
        "call_expression" => {
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
                        if let Some(construction) = constructed_ts(argument, source) {
                            found.callargs.push(CallArg {
                                callee: callee.clone(),
                                place,
                                construction,
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

// A direct construction: a new expression over a bare name.
fn constructed_ts(node: tree_sitter::Node, source: &str) -> Option<String> {
    if node.kind() != "new_expression" {
        return None;
    }
    let constructor = node.child_by_field_name("constructor")?;
    if constructor.kind() != "identifier" {
        return None;
    }
    Some(source[constructor.byte_range()].to_string())
}

// Every type identifier an annotation names, so a union or an array element
// still speaks each of its doors.
fn annotation_names(node: tree_sitter::Node, source: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut stack = vec![node];
    while let Some(at) = stack.pop() {
        if at.kind() == "type_identifier" {
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
