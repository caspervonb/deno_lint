// Copyright 2020-2021 the Deno authors. All rights reserved. MIT license.
use super::{Context, LintRule, ProgramRef, DUMMY_NODE};
use swc_ecmascript::visit::noop_visit_type;
use swc_ecmascript::visit::Node;
use swc_ecmascript::visit::Visit;

pub struct ExplicitFunctionReturnType;

impl LintRule for ExplicitFunctionReturnType {
  fn new() -> Box<Self> {
    Box::new(ExplicitFunctionReturnType)
  }

  fn code(&self) -> &'static str {
    "explicit-function-return-type"
  }

  fn lint_program<'view>(
    &self,
    context: &mut Context<'view>,
    program: ProgramRef<'view>,
  ) {
    let mut visitor = ExplicitFunctionReturnTypeVisitor::new(context);
    match program {
      ProgramRef::Module(ref m) => visitor.visit_module(m, &DUMMY_NODE),
      ProgramRef::Script(ref s) => visitor.visit_script(s, &DUMMY_NODE),
    }
  }

  fn docs(&self) -> &'static str {
    r#"Requires all functions to have explicit return types.

Explicit return types have a number of advantages including easier to understand
code and better type safety.  It is clear from the signature what the return 
type of the function (if any) will be.

### Invalid:
```typescript
function someCalc() { return 2*2; }
function anotherCalc() { return; }
```
    
### Valid:
```typescript
function someCalc(): number { return 2*2; }
function anotherCalc(): void { return; }
```
"#
  }
}

struct ExplicitFunctionReturnTypeVisitor<'c, 'view> {
  context: &'c mut Context<'view>,
}

impl<'c, 'view> ExplicitFunctionReturnTypeVisitor<'c, 'view> {
  fn new(context: &'c mut Context<'view>) -> Self {
    Self { context }
  }
}

impl<'c, 'view> Visit for ExplicitFunctionReturnTypeVisitor<'c, 'view> {
  noop_visit_type!();

  fn visit_function(
    &mut self,
    function: &swc_ecmascript::ast::Function,
    _parent: &dyn Node,
  ) {
    if function.return_type.is_none() {
      self.context.add_diagnostic_with_hint(
        function.span,
        "explicit-function-return-type",
        "Missing return type on function",
        "Add a return type to the function signature",
      );
    }
    for stmt in &function.body {
      self.visit_block_stmt(stmt, _parent);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_util::*;

  #[test]
  fn explicit_function_return_type_valid() {
    assert_lint_ok! {
      ExplicitFunctionReturnType,
      "function fooTyped(): void { }",
      "const bar = (a: string) => { }",
      "const barTyped = (a: string): Promise<void> => { }",
    };
  }

  #[test]
  fn explicit_function_return_type_invalid() {
    assert_lint_err::<ExplicitFunctionReturnType>("function foo() { }", 0);
    assert_lint_err_on_line_n::<ExplicitFunctionReturnType>(
      r#"
function a() {
  function b() {}
}
      "#,
      vec![(2, 0), (3, 2)],
    );
  }
}
