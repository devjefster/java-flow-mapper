use std::path::Path;

use super::{ParsedFile, parse_file};
use crate::model::{BodyElement, BranchKind, ClassKind, LoopExecution, LoopKind, MethodInfo};

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
    assert_eq!(condition_call_names, vec!["equals"]);
    assert_eq!(
        call_input_names(&branch.condition_calls[0]),
        vec!["getActive"]
    );
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
    assert_eq!(condition_call_names, vec!["anyMatch"]);
    assert_eq!(call_input_names(&branch.condition_calls[0]), vec!["stream"]);
    assert!(!condition_call_names.contains(&"check"));
}

#[test]
fn attaches_argument_expression_calls_to_owning_call() {
    let methods = parse_methods(
        r#"
            class Demo {
                void create(CreateUserRequest request) {
                    validateEmailUniqueness(request.getEmail());
                    new User(
                        request.getName().trim(),
                        Normalizers.normalizeEmail(request.getEmail()),
                        request.getAge(),
                        true
                    );
                }
            }
            "#,
    );
    let method = method(&methods, "create");

    assert_eq!(method.body.len(), 2);
    let validate = call(&method.body[0]);
    assert_eq!(validate.method_name, "validateEmailUniqueness");
    assert_eq!(call_input_names(&method.body[0]), vec!["getEmail"]);

    let constructor = call(&method.body[1]);
    assert_eq!(constructor.method_name, "<init>");
    assert_eq!(
        call_input_names(&method.body[1]),
        vec!["trim", "normalizeEmail", "getAge"]
    );
    assert_eq!(call_input_names(&constructor.inputs[0]), vec!["getName"]);
    assert_eq!(call_input_names(&constructor.inputs[1]), vec!["getEmail"]);
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
    assert_eq!(loop_node.execution, LoopExecution::ZeroOrMore);
    assert_eq!(loop_node.source, "ready()");
    assert_eq!(call_name(&loop_node.condition_calls[0]), "ready");
    assert_eq!(loop_node.arms[0].label, "body");
    assert_eq!(call_name(&loop_node.arms[0].body[0]), "work");
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
    assert_eq!(loop_node.execution, LoopExecution::OneOrMore);
    assert_eq!(loop_node.source, "again()");
    assert_eq!(loop_node.arms[0].label, "body");
    assert_eq!(call_name(&loop_node.arms[0].body[0]), "work");
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
    let init_call_names = loop_node
        .init_calls
        .iter()
        .map(call_name)
        .collect::<Vec<_>>();
    let condition_call_names = loop_node
        .condition_calls
        .iter()
        .map(call_name)
        .collect::<Vec<_>>();
    assert_eq!(loop_node.kind, LoopKind::For);
    assert_eq!(loop_node.execution, LoopExecution::ZeroOrMore);
    assert!(init_call_names.contains(&"start"));
    assert!(condition_call_names.contains(&"limit"));
    assert_eq!(loop_node.arms[0].label, "body");
    assert_eq!(call_name(&loop_node.arms[0].body[0]), "work");
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
    assert_eq!(loop_node.execution, LoopExecution::ZeroOrMore);
    assert_eq!(loop_node.source, "User user : users");
    assert_eq!(loop_node.locals[0].name, "user");
    assert_eq!(loop_node.locals[0].ty.raw, "User");
    assert_eq!(loop_node.arms[0].label, "body");
    assert_eq!(call_name(&loop_node.arms[0].body[0]), "getEmail");
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
    let BodyElement::Branch(branch) = &loop_node.arms[0].body[0] else {
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
    assert_eq!(call_name(&loop_node.arms[0].body[0]), "work");
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

#[test]
fn parses_try_catch_finally_branches() {
    let methods = parse_methods(
        r#"
            class Demo {
                void guarded() {
                    try {
                        work();
                    } catch (IllegalArgumentException ex) {
                        recover(ex);
                    } finally {
                        cleanup();
                    }
                }
            }
            "#,
    );
    let method = method(&methods, "guarded");

    assert_eq!(method.body.len(), 1);
    let BodyElement::Branch(branch) = &method.body[0] else {
        panic!("expected try/catch branch");
    };
    assert_eq!(branch.kind, BranchKind::TryCatch);
    assert_eq!(branch.arms.len(), 3);
    assert_eq!(branch.arms[0].label, "try");
    assert_eq!(call_name(&branch.arms[0].body[0]), "work");
    assert_eq!(branch.arms[1].label, "catch IllegalArgumentException ex");
    assert_eq!(call_name(&branch.arms[1].body[0]), "recover");
    assert_eq!(branch.arms[2].label, "finally");
    assert_eq!(call_name(&branch.arms[2].body[0]), "cleanup");
}

#[test]
fn indexes_enum_declarations_with_methods() {
    let parsed = parse_source(
        r#"
            package com.example.demo.exception;

            enum ErrorCode {
                USER_NOT_FOUND("User not found");

                private final String defaultMessage;

                ErrorCode(String defaultMessage) {
                    this.defaultMessage = defaultMessage;
                }

                public String getDefaultMessage() {
                    return defaultMessage;
                }
            }
            "#,
    );
    let class = parsed
        .classes
        .iter()
        .find(|class| class.simple_name == "ErrorCode")
        .unwrap();

    assert_eq!(class.kind, ClassKind::Enum);
    assert_eq!(class.fqn.0, "com.example.demo.exception.ErrorCode");
    assert!(
        class
            .methods
            .iter()
            .any(|method| method.name == "getDefaultMessage")
    );
}

#[test]
fn parses_valid_parameter_annotations() {
    let methods = parse_methods(
        r#"
            import jakarta.validation.Valid;
            import org.springframework.web.bind.annotation.RequestBody;

            class DemoController {
                void create(@Valid @RequestBody CreateUserRequest request) {
                }
            }
            "#,
    );
    let method = method(&methods, "create");
    let param = &method.params[0];

    assert_eq!(param.name, "request");
    assert_eq!(param.ty, "CreateUserRequest");
    assert_eq!(param.annotations, vec!["@Valid", "@RequestBody"]);
}

#[test]
fn parses_builtin_field_validation_constraints() {
    let parsed = parse_source(
        r#"
            package com.example.demo.dto;

            import jakarta.validation.constraints.*;

            public class CreateUserRequest {
                @NotBlank(message = ValidationMessages.NAME_REQUIRED)
                @Size(min = 3, max = 120)
                private String name;

                @NotNull
                @Min(value = 18)
                @Max(value = 120)
                private Integer age;
            }
            "#,
    );
    let class = parsed
        .classes
        .iter()
        .find(|class| class.simple_name == "CreateUserRequest")
        .unwrap();
    let name = class
        .fields
        .iter()
        .find(|field| field.name == "name")
        .unwrap();
    let age = class
        .fields
        .iter()
        .find(|field| field.name == "age")
        .unwrap();

    assert_eq!(
        name.validation
            .iter()
            .map(|constraint| constraint.annotation.as_str())
            .collect::<Vec<_>>(),
        vec!["NotBlank", "Size"]
    );
    assert_eq!(
        age.validation
            .iter()
            .map(|constraint| constraint.annotation.as_str())
            .collect::<Vec<_>>(),
        vec!["NotNull", "Min", "Max"]
    );
}

#[test]
fn indexes_custom_constraint_annotation_declarations() {
    let parsed = parse_source(
        r#"
            package com.example.demo.validation;

            import jakarta.validation.Constraint;

            @Constraint(validatedBy = CompanyEmailValidator.class)
            public @interface CompanyEmail {
            }
            "#,
    );
    let annotation = parsed
        .classes
        .iter()
        .find(|class| class.simple_name == "CompanyEmail")
        .unwrap();

    assert_eq!(annotation.kind, ClassKind::Annotation);
    assert_eq!(
        annotation.annotations,
        vec!["@Constraint(validatedBy = CompanyEmailValidator.class)"]
    );
}

fn parse_methods(source: &str) -> Vec<MethodInfo> {
    parse_source(source)
        .classes
        .into_iter()
        .next()
        .unwrap()
        .methods
}

fn parse_source(source: &str) -> ParsedFile {
    let mut parser = tree_sitter::Parser::new();
    let language: tree_sitter::Language = tree_sitter_java::LANGUAGE.into();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(source, None).unwrap();
    parse_file(Path::new("Demo.java"), source, tree.root_node())
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

fn call(element: &BodyElement) -> &crate::model::CallSite {
    let BodyElement::Call(call) = element else {
        panic!("expected call");
    };
    call
}

fn call_input_names(element: &BodyElement) -> Vec<&str> {
    call(element).inputs.iter().map(call_name).collect()
}
