// Copyright 2020-2021 the Deno authors. All rights reserved. MIT license.
use super::{Context, LintRule, ProgramRef, DUMMY_NODE};
use swc_ecmascript::ast::TsKeywordType;
use swc_ecmascript::visit::Node;
use swc_ecmascript::visit::Visit;

pub struct NoExplicitAny;

const CODE: &str = "no-explicit-any";
const MESSAGE: &str = "`any` type is not allowed";
const HINT: &str = "Use a specific type other than `any`";

impl LintRule for NoExplicitAny {
  fn new() -> Box<Self> {
    Box::new(NoExplicitAny)
  }

  fn tags(&self) -> &'static [&'static str] {
    &["recommended"]
  }

  fn code(&self) -> &'static str {
    CODE
  }

  fn lint_program<'view>(
    &self,
    context: &mut Context<'view>,
    program: ProgramRef<'view>,
  ) {
    let mut visitor = NoExplicitAnyVisitor::new(context);
    match program {
      ProgramRef::Module(ref m) => visitor.visit_module(m, &DUMMY_NODE),
      ProgramRef::Script(ref s) => visitor.visit_script(s, &DUMMY_NODE),
    }
  }

  fn docs(&self) -> &'static str {
    r#"Disallows use of the `any` type 

Use of the `any` type disables the type check system around that variable,
defeating the purpose of Typescript which is to provide type safe code.
Additionally, the use of `any` hinders code readability, since it is not
immediately clear what type of value is being referenced.  It is better to be
explicit about all types.  For a more type-safe alternative to `any`, use 
`unknown` if you are unable to choose a more specific type.

### Invalid:
```typescript
const someNumber: any = "two";
function foo(): any { return undefined; }
```

### Valid:
```typescript
const someNumber: string = "two";
function foo(): undefined { return undefined; }
```
"#
  }
}

struct NoExplicitAnyVisitor<'c, 'view> {
  context: &'c mut Context<'view>,
}

impl<'c, 'view> NoExplicitAnyVisitor<'c, 'view> {
  fn new(context: &'c mut Context<'view>) -> Self {
    Self { context }
  }
}

impl<'c, 'view> Visit for NoExplicitAnyVisitor<'c, 'view> {
  fn visit_ts_keyword_type(
    &mut self,
    ts_keyword_type: &TsKeywordType,
    _parent: &dyn Node,
  ) {
    use swc_ecmascript::ast::TsKeywordTypeKind::*;

    if ts_keyword_type.kind == TsAnyKeyword {
      self.context.add_diagnostic_with_hint(
        ts_keyword_type.span,
        CODE,
        MESSAGE,
        HINT,
      );
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn no_explicit_any_valid() {
    assert_lint_ok! {
      NoExplicitAny,
      r#"
class Foo {
  static _extensions: {
    // deno-lint-ignore no-explicit-any
    [key: string]: (module: Module, filename: string) => any;
  } = Object.create(null);
}"#,
      r#"
type RequireWrapper = (
  // deno-lint-ignore no-explicit-any
  exports: any,
  // deno-lint-ignore no-explicit-any
  require: any,
  module: Module,
  __filename: string,
  __dirname: string
) => void;"#,
    };
  }

  #[test]
  fn no_explicit_any_invalid() {
    assert_lint_err! {
      NoExplicitAny,
      "function foo(): any { return undefined; }": [{ col: 16, message: MESSAGE, hint: HINT }],
      "function bar(): Promise<any> { return undefined; }": [{ col: 24, message: MESSAGE, hint: HINT }],
      "const a: any = {};": [{ col: 9, message: MESSAGE, hint: HINT }],
      r#"
class Foo {
  static _extensions: {
    [key: string]: (module: Module, filename: string) => any;
  } = Object.create(null);
}"#: [{ line: 4, col: 57, message: MESSAGE, hint: HINT }],
      r#"
type RequireWrapper = (
  exports: any,
  require: any,
  module: Module,
  __filename: string,
  __dirname: string
) => void;"#: [{ line: 3, col: 11, message: MESSAGE, hint: HINT }, { line: 4, col: 11, message: MESSAGE, hint: HINT }],
    }
  }
}
