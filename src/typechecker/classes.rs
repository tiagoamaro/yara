use super::*;

/// Builds `self.classes` in two passes over every `Stmt::ClassDef` in the
/// program. Pass one just registers each class's *name* with an empty,
/// placeholder `ClassInfo` — this exists purely so that pass two can
/// freely resolve a field/param/return type annotation that names *any*
/// class, including one declared later in the file or the class's own
/// name (self-referential fields, e.g. a `Node` with a `next: Node`
/// field). Pass two then does the real work for each class: resolves
/// every const/field annotation into a `Type` (consts and instance vars
/// are merged into one `fields` map, since both are read unqualified
/// inside methods via implicit `self` and the checker doesn't need to
/// tell them apart afterward), and resolves every method's param/return
/// types into a `FunctionSig` — but does not yet check any method
/// *bodies*; that happens later in `check_classes`, once every class's
/// signatures are known.
pub(super) fn collect_classes(
    checker: &mut TypeChecker,
    program: &[Stmt],
) -> Result<(), TypeError> {
    // Pre-register every class name (with an empty placeholder) first, so
    // field/param/return type annotations can name any class regardless
    // of declaration order (including a class referencing itself).
    for stmt in program {
        if let Stmt::ClassDef { name, .. } = stmt {
            checker.classes.entry(name.clone()).or_insert(ClassInfo {
                fields: HashMap::new(),
                methods: HashMap::new(),
            });
        }
    }

    for stmt in program {
        let Stmt::ClassDef {
            name,
            consts,
            fields,
            methods,
            ..
        } = stmt
        else {
            continue;
        };

        let mut field_types = HashMap::new();
        for c in consts {
            let Stmt::ConstDecl {
                name: cname,
                type_ann,
                line,
                column,
                ..
            } = c
            else {
                unreachable!("parser only ever puts ConstDecl in ClassDef.consts")
            };
            let Some(ann) = type_ann else {
                return Err(TypeError {
                    message: "class constants require an explicit type annotation".to_string(),
                    line: *line,
                    column: *column,
                });
            };
            let ty = checker.resolve_type(&ann.name, ann.line, ann.column)?;
            field_types.insert(cname.clone(), ty);
        }
        for f in fields {
            let ty = checker.resolve_type(&f.type_ann.name, f.line, f.column)?;
            field_types.insert(f.name.clone(), ty);
        }

        let mut method_sigs = HashMap::new();
        for m in methods {
            let Stmt::FunctionDef {
                name: mname,
                params,
                return_type,
                line,
                column,
                ..
            } = m
            else {
                unreachable!("parser only ever puts FunctionDef in ClassDef.methods")
            };
            let mut param_types = Vec::new();
            for p in params {
                param_types.push(checker.resolve_type(&p.type_ann.name, p.line, p.column)?);
            }
            let ret = match return_type {
                Some(t) => Some(checker.resolve_type(&t.name, *line, *column)?),
                None => None,
            };
            method_sigs.insert(
                mname.clone(),
                FunctionSig {
                    param_types,
                    return_type: ret,
                },
            );
        }

        checker.classes.insert(
            name.clone(),
            ClassInfo {
                fields: field_types,
                methods: method_sigs,
            },
        );
    }
    Ok(())
}

/// Type-checks every class method body (including `initializer`), with
/// the class's fields/consts pre-declared in the method's scope so bare
/// names resolve to them (implicit `self`, Ruby-ivar style).
pub(super) fn check_classes(checker: &mut TypeChecker, program: &[Stmt]) -> Result<(), TypeError> {
    for stmt in program {
        let Stmt::ClassDef {
            name,
            fields,
            methods,
            ..
        } = stmt
        else {
            continue;
        };
        check_fields_assigned_in_initializer(checker, name, fields, methods)?;
        let field_types = checker.classes[name].fields.clone();
        for m in methods {
            let Stmt::FunctionDef {
                params,
                body,
                return_type,
                ..
            } = m
            else {
                unreachable!("parser only ever puts FunctionDef in ClassDef.methods")
            };
            checker.push_scope();
            for (fname, fty) in &field_types {
                checker.declare_var(fname, fty.clone());
            }
            for p in params {
                let ty = checker.resolve_type(&p.type_ann.name, p.line, p.column)?;
                checker.declare_var(&p.name, ty);
            }
            let declared_return = match return_type {
                Some(t) => Some(checker.resolve_type(&t.name, t.line, t.column)?),
                None => None,
            };
            let actual_return = super::stmts::check_body_return_type(checker, body)?;
            if let (Some(declared), Some(actual)) = (&declared_return, &actual_return) {
                if declared != actual {
                    checker.pop_scope();
                    return Err(TypeError {
                        message: format!(
                            "method `{name}#{}` declared to return `{declared}`, but returns `{actual}`",
                            method_name(m)
                        ),
                        line: m.line(),
                        column: m.column(),
                    });
                }
            }
            checker.pop_scope();
        }
    }
    Ok(())
}

/// Requires every instance-variable declaration (a `FieldDecl` — always
/// valueless by construction) to be assigned somewhere in the class's
/// `initializer`, closing the soundness gap where reading a never-assigned
/// field type-checked as its declared type but evaluated to `Nil`.
///
/// The check is deliberately flow-insensitive: an assignment anywhere in
/// the initializer body (including inside an `if` branch or loop) counts,
/// so a conditionally-assigned field still passes. That over-approximation
/// is accepted — the goal is catching the common "declared but never set
/// anywhere" mistake, not full definite-assignment analysis. Inside
/// methods a bare `count = number` assignment parses as `Stmt::VarDecl`
/// (implicit self), so collecting `VarDecl` names is what detects field
/// assignment.
fn check_fields_assigned_in_initializer(
    _checker: &TypeChecker,
    class_name: &str,
    fields: &[crate::ast::FieldDecl],
    methods: &[Stmt],
) -> Result<(), TypeError> {
    if fields.is_empty() {
        return Ok(());
    }
    let initializer = methods.iter().find_map(|m| match m {
        Stmt::FunctionDef { name, body, .. } if name == "initializer" => Some(body),
        _ => None,
    });
    let mut assigned = std::collections::HashSet::new();
    if let Some(body) = initializer {
        collect_assigned_names(body, &mut assigned);
    }
    for f in fields {
        if !assigned.contains(f.name.as_str()) {
            return Err(TypeError {
                message: format!(
                    "field `{}` of class `{class_name}` is never assigned in `initializer` \
                     (it would be `Nil` at runtime, not `{}`)",
                    f.name, f.type_ann.name
                ),
                line: f.line,
                column: f.column,
            });
        }
    }
    Ok(())
}

/// Shared by `Expr::FieldAccess` and `Stmt::FieldAssign`: resolves
/// `object.field`'s type, erroring if `object` isn't a class instance or
/// the class has no such field.
pub(super) fn check_field_access(
    checker: &mut TypeChecker,
    object: &Expr,
    field: &str,
    line: usize,
    column: usize,
) -> Result<Type, TypeError> {
    let object_ty = super::exprs::check_expr(checker, object)?;
    let Type::Instance(class_name) = &object_ty else {
        return Err(TypeError {
            message: format!("cannot access field `{field}` on `{object_ty}`"),
            line,
            column,
        });
    };
    checker.classes[class_name]
        .fields
        .get(field)
        .cloned()
        .ok_or_else(|| TypeError {
            message: format!("class `{class_name}` has no field `{field}`"),
            line,
            column,
        })
}

/// Handles both `ClassName.new(args)` (when `object` is a bare `Ident`
/// naming a class rather than a bound variable) and ordinary
/// `instance.method(args)` calls.
pub(super) fn check_method_call(
    checker: &mut TypeChecker,
    object: &Expr,
    method: &str,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Type, TypeError> {
    if let Expr::Ident { name, .. } = object {
        if checker.lookup_var(name).is_none() && checker.classes.contains_key(name) {
            return check_construction(checker, name, method, args, line, column);
        }
    }

    let object_ty = super::exprs::check_expr(checker, object)?;
    let Type::Instance(class_name) = &object_ty else {
        return Err(TypeError {
            message: format!("cannot call method `{method}` on `{object_ty}`"),
            line,
            column,
        });
    };
    let sig = checker.classes[class_name]
        .methods
        .get(method)
        .cloned()
        .ok_or_else(|| TypeError {
            message: format!("class `{class_name}` has no method `{method}`"),
            line,
            column,
        })?;
    super::calls::check_call_args(
        checker,
        &format!("{class_name}#{method}"),
        &sig,
        args,
        line,
        column,
    )?;
    Ok(sig.return_type.unwrap_or(Type::Nil))
}

/// Type-checks `ClassName.new(args)`, dispatched from `check_method_call`
/// when `method_call`'s object is a bare `Ident` naming a known class
/// (rather than a variable bound to an instance). Only `new` is a valid
/// "static method" name — anything else is an error. If the class
/// declared an `initializer` method, arguments are checked against its
/// signature via `check_call_args`; if it didn't, `.new` must be called
/// with zero arguments (a helpful error otherwise). Always yields
/// `Type::Instance(class_name)` on success, since construction always
/// produces an instance of the class being constructed.
fn check_construction(
    checker: &mut TypeChecker,
    class_name: &str,
    method: &str,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Type, TypeError> {
    if method != "new" {
        return Err(TypeError {
            message: format!("class `{class_name}` has no static method `{method}`"),
            line,
            column,
        });
    }
    match checker.classes[class_name]
        .methods
        .get("initializer")
        .cloned()
    {
        Some(sig) => {
            super::calls::check_call_args(
                checker,
                &format!("{class_name}.new"),
                &sig,
                args,
                line,
                column,
            )?;
        }
        None if !args.is_empty() => {
            return Err(TypeError {
                message: format!(
                    "class `{class_name}` has no initializer, so `.new` takes no arguments"
                ),
                line,
                column,
            });
        }
        None => {}
    }
    Ok(Type::Instance(class_name.to_string()))
}

/// Collects every name assigned anywhere in `body` into `assigned`,
/// recursing through `if`/`while`/`for` bodies (flow-insensitively — a
/// conditional assignment still counts; see
/// `check_fields_assigned_in_initializer`). Both a bare `name = value`
/// (`Stmt::VarDecl`, how implicit-self field assignment parses inside
/// methods) and an explicit `object.field = value` (`Stmt::FieldAssign`)
/// register the assigned name.
fn collect_assigned_names(body: &[Stmt], assigned: &mut std::collections::HashSet<String>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl { name, .. } => {
                assigned.insert(name.clone());
            }
            Stmt::FieldAssign { field, .. } => {
                assigned.insert(field.clone());
            }
            Stmt::If {
                then_body,
                elsif_branches,
                else_body,
                ..
            } => {
                collect_assigned_names(then_body, assigned);
                for (_, branch) in elsif_branches {
                    collect_assigned_names(branch, assigned);
                }
                if let Some(body) = else_body {
                    collect_assigned_names(body, assigned);
                }
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                collect_assigned_names(body, assigned);
            }
            _ => {}
        }
    }
}

/// Extracts a method's name from its `Stmt::FunctionDef` node, for building
/// the `"ClassName#method_name"` label used in return-type-mismatch error
/// messages in `check_classes`. Returns `"?"` for any other statement kind,
/// which should never actually happen since `ClassDef.methods` only ever
/// contains `FunctionDef`s (see the `unreachable!` guards in `collect_classes`
/// and `check_classes`).
fn method_name(method: &Stmt) -> &str {
    match method {
        Stmt::FunctionDef { name, .. } => name,
        _ => "?",
    }
}
