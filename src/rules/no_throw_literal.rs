// Copyright 2020-2021 the Deno authors. All rights reserved. MIT license.
use super::{Context, LintRule, ProgramRef, DUMMY_NODE};
use swc_ecmascript::ast::{Expr, ThrowStmt};
use swc_ecmascript::visit::noop_visit_type;
use swc_ecmascript::visit::Node;
use swc_ecmascript::visit::Visit;

pub struct NoThrowLiteral;

impl LintRule for NoThrowLiteral {
  fn new() -> Box<Self> {
    Box::new(NoThrowLiteral)
  }

  fn code(&self) -> &'static str {
    "no-throw-literal"
  }

  fn lint_program<'view>(
    &self,
    context: &mut Context<'view>,
    program: ProgramRef<'view>,
  ) {
    let mut visitor = NoThrowLiteralVisitor::new(context);
    match program {
      ProgramRef::Module(ref m) => visitor.visit_module(m, &DUMMY_NODE),
      ProgramRef::Script(ref s) => visitor.visit_script(s, &DUMMY_NODE),
    }
  }
}

struct NoThrowLiteralVisitor<'c, 'view> {
  context: &'c mut Context<'view>,
}

impl<'c, 'view> NoThrowLiteralVisitor<'c, 'view> {
  fn new(context: &'c mut Context<'view>) -> Self {
    Self { context }
  }
}

impl<'c, 'view> Visit for NoThrowLiteralVisitor<'c, 'view> {
  noop_visit_type!();

  fn visit_throw_stmt(&mut self, throw_stmt: &ThrowStmt, _parent: &dyn Node) {
    match &*throw_stmt.arg {
      Expr::Lit(_) => self.context.add_diagnostic(
        throw_stmt.span,
        "no-throw-literal",
        "expected an error object to be thrown",
      ),
      Expr::Ident(ident) if ident.sym == *"undefined" => {
        self.context.add_diagnostic(
          throw_stmt.span,
          "no-throw-literal",
          "do not throw undefined",
        )
      }
      _ => {}
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_util::*;

  #[test]
  fn no_throw_literal_valid() {
    assert_lint_ok! {
      NoThrowLiteral,
      "throw e",
    };
  }

  #[test]
  fn no_throw_literal_invalid() {
    assert_lint_err::<NoThrowLiteral>("throw 'kumiko'", 0);
    assert_lint_err::<NoThrowLiteral>("throw true", 0);
    assert_lint_err::<NoThrowLiteral>("throw 1096", 0);
    assert_lint_err::<NoThrowLiteral>("throw null", 0);
    assert_lint_err::<NoThrowLiteral>("throw undefined", 0);
  }
}
