use std::path::Path;

use super::parse_file;
use crate::model::{BodyElement, BranchKind, LoopKind, MethodInfo};

#[test]
fn parses_if_without_else_and_marks_throwing_then_arm() {
    let methods = parse_methods(
        r#"
            class Demo {
                void guard(User user) {
                    if (user == null) {
                        throw new IllegalArgumentException("missing");
                    }
                    save(user);
                }
            }
            "#,
    );
    let method = method(&methods, "guard");

    assert_eq!(method.body.len(), 2);
    let BodyElement::Branch(branch) = &method.body[0] else {
        panic!("expected first body element to be a branch");
    };
    assert_eq!(branch.condition_src, "user == null");
    assert!(branch.then_terminates);
    assert!(branch.else_arm.is_none());
    assert_eq!(
        call_name(&branch.then_arm[0]),
        "IllegalArgumentException#<init>"
    );
    assert_eq!(call_name(&method.body[1]), "save");
}

#[test]
fn parses_if_else_with_both_arms_populated() {
    let methods = parse_methods(
        r#"
            class Demo {
                void choose() {
                    if (ready()) {
                        yes();
                    } else {
                        no();
                    }
                }
            }
            "#,
    );
    let method = method(&methods, "choose");

    let BodyElement::Branch(branch) = &method.body[0] else {
        panic!("expected branch");
    };
    assert_eq!(call_name(&branch.condition_calls[0]), "ready");
    assert_eq!(call_name(&branch.then_arm[0]), "yes");
    assert_eq!(call_name(&branch.else_arm.as_ref().unwrap()[0]), "no");
}

#[test]
fn preserves_nested_if_structure_inside_arm() {
    let methods = parse_methods(
        r#"
            class Demo {
                void nested() {
                    if (outer()) {
                        if (inner()) {
                            work();
                        }
                    }
                }
            }
            "#,
    );
    let method = method(&methods, "nested");

    let BodyElement::Branch(outer) = &method.body[0] else {
        panic!("expected outer branch");
    };
    let BodyElement::Branch(inner) = &outer.then_arm[0] else {
        panic!("expected nested branch");
    };
    assert_eq!(call_name(&outer.condition_calls[0]), "outer");
    assert_eq!(call_name(&inner.condition_calls[0]), "inner");
    assert_eq!(call_name(&inner.then_arm[0]), "work");
}

#[test]
fn separates_calls_inside_condition_from_then_arm() {
    let methods = parse_methods(
        r#"
            class Demo {
                void conditionCalls(User user) {
                    if (Boolean.TRUE.equals(user.getActive())) {
                        delete(user);
                    }
                }
            }
            "#,
    );
    let method = method(&methods, "conditionCalls");

    let BodyElement::Branch(branch) = &method.body[0] else {
        panic!("expected branch");
    };
    let condition_call_names = branch
        .condition_calls
        .iter()
        .map(call_name)
        .collect::<Vec<_>>();
    assert_eq!(
        branch.condition_src,
        "Boolean.TRUE.equals(user.getActive())"
    );
    assert_eq!(condition_call_names, vec!["getActive", "equals"]);
    assert_eq!(call_name(&branch.then_arm[0]), "delete");
}

#[test]
fn skips_lambda_body_inside_condition() {
    let methods = parse_methods(
        r#"
            import java.util.List;

            class Demo {
                void lambdaCondition(List<User> users) {
                    if (users.stream().anyMatch(user -> check(user))) {
                        found();
                    }
                }
            }
            "#,
    );
    let method = method(&methods, "lambdaCondition");

    let BodyElement::Branch(branch) = &method.body[0] else {
        panic!("expected branch");
    };
    let condition_call_names = branch
        .condition_calls
        .iter()
        .map(call_name)
        .collect::<Vec<_>>();
    assert!(condition_call_names.contains(&"stream"));
    assert!(condition_call_names.contains(&"anyMatch"));
    assert!(!condition_call_names.contains(&"check"));
}

#[test]
fn parses_while_loop_with_condition_and_body() {
    let methods = parse_methods(
        r#"
            class Demo {
                void poll() {
                    while (ready()) {
                        work();
                    }
                }
            }
            "#,
    );
    let method = method(&methods, "poll");

    let BodyElement::Loop(loop_node) = &method.body[0] else {
        panic!("expected loop");
    };
    assert_eq!(loop_node.kind, LoopKind::While);
    assert_eq!(loop_node.source, "ready()");
    assert_eq!(call_name(&loop_node.condition_calls[0]), "ready");
    assert_eq!(call_name(&loop_node.body[0]), "work");
}

#[test]
fn parses_do_while_loop_with_body_before_condition() {
    let methods = parse_methods(
        r#"
            class Demo {
                void retry() {
                    do {
                        work();
                    } while (again());
                }
            }
            "#,
    );
    let method = method(&methods, "retry");

    let BodyElement::Loop(loop_node) = &method.body[0] else {
        panic!("expected loop");
    };
    assert_eq!(loop_node.kind, LoopKind::DoWhile);
    assert_eq!(loop_node.source, "again()");
    assert_eq!(call_name(&loop_node.body[0]), "work");
    assert_eq!(call_name(&loop_node.condition_calls[0]), "again");
}

#[test]
fn parses_classic_for_loop_header_body_and_update_calls() {
    let methods = parse_methods(
        r#"
            class Demo {
                void count() {
                    for (int i = start(); i < limit(); i = next(i)) {
                        work();
                    }
                }
            }
            "#,
    );
    let method = method(&methods, "count");

    let BodyElement::Loop(loop_node) = &method.body[0] else {
        panic!("expected loop");
    };
    let condition_call_names = loop_node
        .condition_calls
        .iter()
        .map(call_name)
        .collect::<Vec<_>>();
    assert_eq!(loop_node.kind, LoopKind::For);
    assert!(condition_call_names.contains(&"start"));
    assert!(condition_call_names.contains(&"limit"));
    assert_eq!(call_name(&loop_node.body[0]), "work");
    assert_eq!(call_name(&loop_node.update_calls[0]), "next");
}

#[test]
fn parses_enhanced_for_loop_with_loop_local() {
    let methods = parse_methods(
        r#"
            import java.util.List;

            class Demo {
                void each(List<User> users) {
                    for (User user : users) {
                        user.getEmail();
                    }
                }
            }
            "#,
    );
    let method = method(&methods, "each");

    let BodyElement::Loop(loop_node) = &method.body[0] else {
        panic!("expected loop");
    };
    assert_eq!(loop_node.kind, LoopKind::EnhancedFor);
    assert_eq!(loop_node.source, "User user : users");
    assert_eq!(loop_node.locals[0].name, "user");
    assert_eq!(loop_node.locals[0].ty.raw, "User");
    assert_eq!(call_name(&loop_node.body[0]), "getEmail");
}

#[test]
fn preserves_nested_branch_inside_loop() {
    let methods = parse_methods(
        r#"
            class Demo {
                void guarded() {
                    while (ready()) {
                        if (allowed()) {
                            work();
                        }
                    }
                }
            }
            "#,
    );
    let method = method(&methods, "guarded");

    let BodyElement::Loop(loop_node) = &method.body[0] else {
        panic!("expected loop");
    };
    let BodyElement::Branch(branch) = &loop_node.body[0] else {
        panic!("expected branch inside loop");
    };
    assert_eq!(call_name(&branch.condition_calls[0]), "allowed");
    assert_eq!(call_name(&branch.then_arm[0]), "work");
}

#[test]
fn preserves_loop_inside_branch_arm() {
    let methods = parse_methods(
        r#"
            class Demo {
                void branchLoop() {
                    if (enabled()) {
                        while (ready()) {
                            work();
                        }
                    }
                }
            }
            "#,
    );
    let method = method(&methods, "branchLoop");

    let BodyElement::Branch(branch) = &method.body[0] else {
        panic!("expected branch");
    };
    let BodyElement::Loop(loop_node) = &branch.then_arm[0] else {
        panic!("expected loop inside branch");
    };
    assert_eq!(loop_node.kind, LoopKind::While);
    assert_eq!(call_name(&loop_node.body[0]), "work");
}

#[test]
fn parses_switch_statement_branches() {
    let methods = parse_methods(
        r#"
            class Demo {
                void route(Status status) {
                    switch (status.kind()) {
                        case ACTIVE:
                            activate();
                            break;
                        case DISABLED:
                            throw new IllegalStateException("disabled");
                        default:
                            ignore();
                    }
                }
            }
            "#,
    );
    let method = method(&methods, "route");

    assert_eq!(method.body.len(), 1);
    let BodyElement::Branch(branch) = &method.body[0] else {
        panic!("expected switch branch");
    };
    assert_eq!(branch.kind, BranchKind::Switch);
    assert_eq!(branch.condition_src, "status.kind()");
    assert_eq!(call_name(&branch.condition_calls[0]), "kind");
    assert_eq!(branch.arms.len(), 3);
    assert_eq!(branch.arms[0].label, "ACTIVE");
    assert_eq!(call_name(&branch.arms[0].body[0]), "activate");
    assert!(!branch.arms[0].terminates);
    assert_eq!(branch.arms[1].label, "DISABLED");
    assert_eq!(call_name(&branch.arms[1].body[0]), "<init>");
    assert!(branch.arms[1].terminates);
    assert_eq!(branch.arms[2].label, "default");
    assert_eq!(call_name(&branch.arms[2].body[0]), "ignore");
}

#[test]
#[ignore]
fn parses_switch_expression_branches() {
    unimplemented!("switch_expression parsing is planned for PR #6");
}

#[test]
fn parses_ternary_expression_branches() {
    let methods = parse_methods(
        r#"
            class Demo {
                String choose() {
                    return enabled() ? yes() : no();
                }
            }
            "#,
    );
    let method = method(&methods, "choose");

    assert_eq!(method.body.len(), 1);
    let BodyElement::Branch(branch) = &method.body[0] else {
        panic!("expected ternary branch");
    };
    assert_eq!(branch.kind, BranchKind::Ternary);
    assert_eq!(branch.condition_src, "enabled()");
    assert_eq!(call_name(&branch.condition_calls[0]), "enabled");
    assert_eq!(branch.arms.len(), 2);
    assert_eq!(branch.arms[0].label, "then");
    assert_eq!(call_name(&branch.arms[0].body[0]), "yes");
    assert_eq!(branch.arms[1].label, "else");
    assert_eq!(call_name(&branch.arms[1].body[0]), "no");
}

fn parse_methods(source: &str) -> Vec<MethodInfo> {
    let mut parser = tree_sitter::Parser::new();
    let language: tree_sitter::Language = tree_sitter_java::LANGUAGE.into();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(source, None).unwrap();
    let parsed = parse_file(Path::new("Demo.java"), source, tree.root_node());
    parsed.classes.into_iter().next().unwrap().methods
}

fn method<'a>(methods: &'a [MethodInfo], name: &str) -> &'a MethodInfo {
    methods.iter().find(|method| method.name == name).unwrap()
}

fn call_name(element: &BodyElement) -> &str {
    match element {
        BodyElement::Call(call) if call.method_name == "<init>" => match &call.receiver {
            crate::model::ReceiverKind::Constructor(ty)
                if ty.ends_with("IllegalArgumentException") =>
            {
                "IllegalArgumentException#<init>"
            }
            _ => "<init>",
        },
        BodyElement::Call(call) => &call.method_name,
        BodyElement::Branch(_) => "branch",
        BodyElement::Loop(_) => "loop",
    }
}
