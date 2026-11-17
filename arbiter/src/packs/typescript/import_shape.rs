use tree_sitter::Tree;

// One import statement read to its parts; the byte span allows rewriting.
pub struct ImportShape {
    pub start: usize,
    pub end: usize,
    pub row: usize,
    pub type_only: bool,
    pub module: String,
    pub default: Option<String>,
    pub namespace: Option<String>,
    pub named: Vec<String>,
}

pub fn import_shapes(tree: &Tree, source: &str) -> Vec<ImportShape> {
    let mut shapes = Vec::new();
    let root = tree.root_node();
    let mut cursor = root.walk();
    for node in root.named_children(&mut cursor) {
        if node.kind() != "import_statement" {
            continue;
        }
        let mut shape = ImportShape {
            start: node.start_byte(),
            end: node.end_byte(),
            row: node.start_position().row + 1,
            type_only: source[node.start_byte()..node.end_byte()].starts_with("import type "),
            module: String::new(),
            default: None,
            namespace: None,
            named: Vec::new(),
        };
        let mut children = node.walk();
        for child in node.named_children(&mut children) {
            match child.kind() {
                "import_clause" => {
                    let mut parts = child.walk();
                    for part in child.named_children(&mut parts) {
                        let written = source[part.start_byte()..part.end_byte()].to_string();
                        match part.kind() {
                            "identifier" => shape.default = Some(written),
                            "namespace_import" => shape.namespace = Some(written),
                            "named_imports" => {
                                let mut specs = part.walk();
                                for spec in part.named_children(&mut specs) {
                                    if spec.kind() == "import_specifier" {
                                        shape.named.push(
                                            source[spec.start_byte()..spec.end_byte()].to_string(),
                                        );
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "string" => {
                    let mut parts = child.walk();
                    for part in child.named_children(&mut parts) {
                        if part.kind() == "string_fragment" {
                            shape.module = source[part.start_byte()..part.end_byte()].to_string();
                        }
                    }
                }
                _ => {}
            }
        }
        shapes.push(shape);
    }
    shapes
}
