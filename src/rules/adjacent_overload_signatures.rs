// Copyright 2020-2021 the Deno authors. All rights reserved. MIT license.
use super::{Context, LintRule, ProgramRef, DUMMY_NODE};
use crate::swc_util::StringRepr;
use std::collections::HashSet;
use swc_common::Span;
use swc_common::Spanned;
use swc_ecmascript::ast::{
  Class, ClassMember, ClassMethod, Decl, ExportDecl, Expr, FnDecl, Ident, Lit,
  Module, ModuleDecl, ModuleItem, Script, Stmt, Str, TsInterfaceBody,
  TsMethodSignature, TsModuleBlock, TsTypeElement, TsTypeLit,
};
use swc_ecmascript::visit::VisitAllWith;
use swc_ecmascript::visit::{Node, VisitAll};

pub struct AdjacentOverloadSignatures;

impl LintRule for AdjacentOverloadSignatures {
  fn new() -> Box<Self> {
    Box::new(AdjacentOverloadSignatures)
  }

  fn tags(&self) -> &'static [&'static str] {
    &["recommended"]
  }

  fn code(&self) -> &'static str {
    "adjacent-overload-signatures"
  }

  fn lint_program<'view>(
    &self,
    context: &mut Context<'view>,
    program: ProgramRef<'view>,
  ) {
    let mut visitor = AdjacentOverloadSignaturesVisitor::new(context);
    match program {
      ProgramRef::Module(ref m) => m.visit_all_with(&DUMMY_NODE, &mut visitor),
      ProgramRef::Script(ref s) => s.visit_all_with(&DUMMY_NODE, &mut visitor),
    }
  }

  fn docs(&self) -> &'static str {
    r#"Requires overload signatures to be adjacent to each other.

Overloaded signatures which are not next to each other can lead to code which is hard to read and maintain.

### Invalid:
(bar is declared in-between foo overloads)
```typescript
type FooType = {
  foo(s: string): void;
  foo(n: number): void;
  bar(): void;
  foo(sn: string | number): void;
};
```
```typescript
interface FooInterface {
  foo(s: string): void;
  foo(n: number): void;
  bar(): void;
  foo(sn: string | number): void;
}
```
```typescript
class FooClass {
  foo(s: string): void;
  foo(n: number): void;
  bar(): void {}
  foo(sn: string | number): void {}
}
```
```typescript
export function foo(s: string): void;
export function foo(n: number): void;
export function bar(): void {}
export function foo(sn: string | number): void {}
```
### Valid:
(bar is declared after foo)
```typescript
type FooType = {
  foo(s: string): void;
  foo(n: number): void;
  foo(sn: string | number): void;
  bar(): void;
};
```
```typescript
interface FooInterface {
  foo(s: string): void;
  foo(n: number): void;
  foo(sn: string | number): void;
  bar(): void;
}
```
```typescript
class FooClass {
  foo(s: string): void;
  foo(n: number): void;
  foo(sn: string | number): void {}
  bar(): void {}
}
```
```typescript
export function foo(s: string): void;
export function foo(n: number): void;
export function foo(sn: string | number): void {}
export function bar(): void {}
```"#
  }
}

struct AdjacentOverloadSignaturesVisitor<'c, 'view> {
  context: &'c mut Context<'view>,
}

impl<'c, 'view> AdjacentOverloadSignaturesVisitor<'c, 'view> {
  fn new(context: &'c mut Context<'view>) -> Self {
    Self { context }
  }

  fn add_diagnostic(&mut self, span: Span, fn_name: &str) {
    self.context.add_diagnostic_with_hint(
      span,
      "adjacent-overload-signatures",
      format!("All '{}' signatures should be adjacent", fn_name),
      "Make sure all overloaded signatures are grouped together",
    );
  }

  fn check<'a, 'b, T, U>(&'a mut self, items: T)
  where
    T: IntoIterator<Item = &'b U>,
    U: ExtractMethod + Spanned + 'b,
  {
    let mut seen_methods = HashSet::new();
    let mut last_method = None;
    for item in items {
      if let Some(method) = item.get_method() {
        if seen_methods.contains(&method)
          && last_method.as_ref() != Some(&method)
        {
          self.add_diagnostic(item.span(), method.get_name());
        }

        seen_methods.insert(method.clone());
        last_method = Some(method);
      } else {
        last_method = None;
      }
    }
  }
}

fn extract_ident_from_decl(decl: &Decl) -> Option<String> {
  match decl {
    Decl::Fn(FnDecl { ref ident, .. }) => Some(ident.sym.to_string()),
    _ => None,
  }
}

trait ExtractMethod {
  fn get_method(&self) -> Option<Method>;
}

impl ExtractMethod for ExportDecl {
  fn get_method(&self) -> Option<Method> {
    let method_name = extract_ident_from_decl(&self.decl);
    method_name.map(Method::Method)
  }
}

impl ExtractMethod for Stmt {
  fn get_method(&self) -> Option<Method> {
    let method_name = match self {
      Stmt::Decl(ref decl) => extract_ident_from_decl(decl),
      _ => None,
    };
    method_name.map(Method::Method)
  }
}

impl ExtractMethod for ModuleItem {
  fn get_method(&self) -> Option<Method> {
    match self {
      ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export_decl)) => {
        export_decl.get_method()
      }
      ModuleItem::Stmt(stmt) => stmt.get_method(),
      _ => None,
    }
  }
}

impl ExtractMethod for ClassMember {
  fn get_method(&self) -> Option<Method> {
    match self {
      ClassMember::Method(ClassMethod {
        ref key, is_static, ..
      }) => key.string_repr().map(|k| {
        if *is_static {
          Method::Static(k)
        } else {
          Method::Method(k)
        }
      }),
      ClassMember::Constructor(_) => {
        Some(Method::Method("constructor".to_string()))
      }
      _ => None,
    }
  }
}

impl ExtractMethod for TsTypeElement {
  fn get_method(&self) -> Option<Method> {
    match self {
      TsTypeElement::TsMethodSignature(TsMethodSignature {
        ref key, ..
      }) => match &**key {
        Expr::Ident(Ident { ref sym, .. }) => {
          Some(Method::Method(sym.to_string()))
        }
        Expr::Lit(Lit::Str(Str { ref value, .. })) => {
          Some(Method::Method(value.to_string()))
        }
        _ => None,
      },
      TsTypeElement::TsCallSignatureDecl(_) => Some(Method::CallSignature),
      TsTypeElement::TsConstructSignatureDecl(_) => {
        Some(Method::ConstructSignature)
      }
      _ => None,
    }
  }
}

impl<'c, 'view> VisitAll for AdjacentOverloadSignaturesVisitor<'c, 'view> {
  fn visit_script(&mut self, script: &Script, _parent: &dyn Node) {
    self.check(&script.body);
  }

  fn visit_module(&mut self, module: &Module, _parent: &dyn Node) {
    self.check(&module.body);
  }

  fn visit_ts_module_block(
    &mut self,
    ts_module_block: &TsModuleBlock,
    _parent: &dyn Node,
  ) {
    self.check(&ts_module_block.body);
  }

  fn visit_class(&mut self, class: &Class, _parent: &dyn Node) {
    self.check(&class.body);
  }

  fn visit_ts_type_lit(&mut self, ts_type_lit: &TsTypeLit, _parent: &dyn Node) {
    self.check(&ts_type_lit.members);
  }

  fn visit_ts_interface_body(
    &mut self,
    ts_inteface_body: &TsInterfaceBody,
    _parent: &dyn Node,
  ) {
    self.check(&ts_inteface_body.body);
  }
}

#[derive(PartialEq, Eq, Hash, Clone)]
enum Method {
  Method(String),
  Static(String),
  CallSignature,
  ConstructSignature,
}

impl Method {
  fn get_name(&self) -> &str {
    match self {
      Method::Method(ref s) | Method::Static(ref s) => s,
      Method::CallSignature => "call",
      Method::ConstructSignature => "new",
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn adjacent_overload_signatures_valid() {
    assert_lint_ok! {
      AdjacentOverloadSignatures,
      r#"
function error(a: string);
function error(b: number);
function error(ab: string | number) {}
export { error };
      "#,
      r#"
import { connect } from 'react-redux';
export interface ErrorMessageModel {
  message: string;
}
function mapStateToProps() {}
function mapDispatchToProps() {}
export default connect(mapStateToProps, mapDispatchToProps)(ErrorMessage);
      "#,
      r#"
export const foo = 'a',
  bar = 'b';
export interface Foo {}
export class Foo {}
      "#,
      r#"
export interface Foo {}
export const foo = 'a',
  bar = 'b';
export class Foo {}
      "#,
      r#"
const foo = 'a',
  bar = 'b';
interface Foo {}
class Foo {}
      "#,
      r#"
interface Foo {}
const foo = 'a',
  bar = 'b';
class Foo {}
      "#,
      r#"
export class Foo {}
export class Bar {}
export type FooBar = Foo | Bar;
      "#,
      r#"
export interface Foo {}
export class Foo {}
export class Bar {}
export type FooBar = Foo | Bar;
      "#,
      r#"
export function foo(s: string);
export function foo(n: number);
export function foo(sn: string | number) {}
export function bar(): void {}
export function baz(): void {}
      "#,
      r#"
function foo(s: string);
function foo(n: number);
function foo(sn: string | number) {}
function bar(): void {}
function baz(): void {}
      "#,
      r#"
declare function foo(s: string);
declare function foo(n: number);
declare function foo(sn: string | number);
declare function bar(): void;
declare function baz(): void;
      "#,
      r#"
declare module 'Foo' {
  export function foo(s: string): void;
  export function foo(n: number): void;
  export function foo(sn: string | number): void;
  export function bar(): void;
  export function baz(): void;
}
      "#,
      r#"
declare namespace Foo {
  export function foo(s: string): void;
  export function foo(n: number): void;
  export function foo(sn: string | number): void;
  export function bar(): void;
  export function baz(): void;
}
      "#,
      r#"
type Foo = {
  foo(s: string): void;
  foo(n: number): void;
  foo(sn: string | number): void;
  bar(): void;
  baz(): void;
};
      "#,
      r#"
type Foo = {
  foo(s: string): void;
  ['foo'](n: number): void;
  foo(sn: string | number): void;
  bar(): void;
  baz(): void;
};
      "#,
      r#"
interface Foo {
  (s: string): void;
  (n: number): void;
  (sn: string | number): void;
  foo(n: number): void;
  bar(): void;
  baz(): void;
}
      "#,
      r#"
interface Foo {
  (s: string): void;
  (n: number): void;
  (sn: string | number): void;
  foo(n: number): void;
  bar(): void;
  baz(): void;
  call(): void;
}
      "#,
      r#"
interface Foo {
  foo(s: string): void;
  foo(n: number): void;
  foo(sn: string | number): void;
  bar(): void;
  baz(): void;
}
      "#,
      r#"
interface Foo {
  foo(s: string): void;
  ['foo'](n: number): void;
  foo(sn: string | number): void;
  bar(): void;
  baz(): void;
}
      "#,
      r#"
interface Foo {
  foo(): void;
  bar: {
    baz(s: string): void;
    baz(n: number): void;
    baz(sn: string | number): void;
  };
}
      "#,
      r#"
interface Foo {
  new (s: string);
  new (n: number);
  new (sn: string | number);
  foo(): void;
}
      "#,
      r#"
class Foo {
  constructor(s: string);
  constructor(n: number);
  constructor(sn: string | number) {}
  bar(): void {}
  baz(): void {}
}
    "#,
      r#"
class Foo {
  foo(s: string): void;
  foo(n: number): void;
  foo(sn: string | number): void {}
  bar(): void {}
  baz(): void {}
}
    "#,
      r#"
class Foo {
  foo(s: string): void;
  "foo"(n: number): void;
  foo(sn: string | number): void {}
  bar(): void {}
  baz(): void {}
}
    "#,
      r#"
class Foo {
  foo(s: string): void;
  ['foo'](n: number): void;
  foo(sn: string | number): void {}
  bar(): void {}
  baz(): void {}
}
      "#,
      r#"
class Foo {
  foo(s: string): void;
  [`foo`](n: number): void;
  foo(sn: string | number): void {}
  bar(): void {}
  baz(): void {}
}
      "#,
      r#"
class Foo {
  name: string;
  foo(s: string): void;
  foo(n: number): void;
  foo(sn: string | number): void {}
  bar(): void {}
  baz(): void {}
}
      "#,
      r#"
class Foo {
  name: string;
  static foo(s: string): void;
  static foo(n: number): void;
  static foo(sn: string | number): void {}
  bar(): void {}
  baz(): void {}
  foo() {}
}
      "#,
      r#"
class Test {
  static test() {}
  untest() {}
  test() {}
}
      "#,
      r#"export default function <T>(foo: T) {}"#,
      r#"export default function named<T>(foo: T) {}"#,
      r#"
interface Foo {
  [Symbol.toStringTag](): void;
  [Symbol.iterator](): void;
}
      "#,
    };
  }

  #[test]
  fn adjacent_overload_signatures_invalid() {
    assert_lint_err! {
      AdjacentOverloadSignatures,
      r#"
export function foo(s: string);
export function foo(n: number);
export function bar(): void {}
export function baz(): void {}
export function foo(sn: string | number) {}
      "#: [
            {
              line: 6,
              col: 0,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
export function foo(s: string);
export function foo(n: number);
export type bar = number;
export type baz = number | string;
export function foo(sn: string | number) {}
      "#: [
            {
              line: 6,
              col: 0,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
function foo(s: string);
function foo(n: number);
function bar(): void {}
function baz(): void {}
function foo(sn: string | number) {}
      "#: [
            {
              line: 6,
              col: 0,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
function foo(s: string);
function foo(n: number);
type bar = number;
type baz = number | string;
function foo(sn: string | number) {}
      "#: [
            {
              line: 6,
              col: 0,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
function foo(s: string) {}
function foo(n: number) {}
const a = '';
const b = '';
function foo(sn: string | number) {}
      "#: [
            {
              line: 6,
              col: 0,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
function foo(s: string) {}
function foo(n: number) {}
class Bar {}
function foo(sn: string | number) {}
      "#: [
            {
              line: 5,
              col: 0,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
function foo(s: string) {}
function foo(n: number) {}
function foo(sn: string | number) {}
class Bar {
  foo(s: string);
  foo(n: number);
  name: string;
  foo(sn: string | number) {}
}
      "#: [
            {
              line: 9,
              col: 2,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
declare function foo(s: string);
declare function foo(n: number);
declare function bar(): void;
declare function baz(): void;
declare function foo(sn: string | number);
      "#: [
            {
              line: 6,
              col: 0,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
declare function foo(s: string);
declare function foo(n: number);
const a = '';
const b = '';
declare function foo(sn: string | number);
      "#: [
            {
              line: 6,
              col: 0,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
declare module 'Foo' {
  export function foo(s: string): void;
  export function foo(n: number): void;
  export function bar(): void;
  export function baz(): void;
  export function foo(sn: string | number): void;
}
      "#: [
            {
              line: 7,
              col: 2,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
declare module 'Foo' {
  export function foo(s: string): void;
  export function foo(n: number): void;
  export function foo(sn: string | number): void;
  function baz(s: string): void;
  export function bar(): void;
  function baz(n: number): void;
  function baz(sn: string | number): void;
}
      "#: [
            {
              line: 8,
              col: 2,
              message: "All 'baz' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
declare namespace Foo {
  export function foo(s: string): void;
  export function foo(n: number): void;
  export function bar(): void;
  export function baz(): void;
  export function foo(sn: string | number): void;
}
      "#: [
            {
              line: 7,
              col: 2,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
declare namespace Foo {
  export function foo(s: string): void;
  export function foo(n: number): void;
  export function foo(sn: string | number): void;
  function baz(s: string): void;
  export function bar(): void;
  function baz(n: number): void;
  function baz(sn: string | number): void;
}
      "#: [
            {
              line: 8,
              col: 2,
              message: "All 'baz' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
type Foo = {
  foo(s: string): void;
  foo(n: number): void;
  bar(): void;
  baz(): void;
  foo(sn: string | number): void;
};
      "#: [
            {
              line: 7,
              col: 2,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
type Foo = {
  foo(s: string): void;
  ['foo'](n: number): void;
  bar(): void;
  baz(): void;
  foo(sn: string | number): void;
};
      "#: [
            {
              line: 7,
              col: 2,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
type Foo = {
  foo(s: string): void;
  name: string;
  foo(n: number): void;
  foo(sn: string | number): void;
  bar(): void;
  baz(): void;
};
      "#: [
            {
              line: 5,
              col: 2,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
interface Foo {
  (s: string): void;
  foo(n: number): void;
  (n: number): void;
  (sn: string | number): void;
  bar(): void;
  baz(): void;
}
      "#: [
            {
              line: 5,
              col: 2,
              message: "All 'call' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
interface Foo {
  foo(s: string): void;
  foo(n: number): void;
  bar(): void;
  baz(): void;
  foo(sn: string | number): void;
}
      "#: [
            {
              line: 7,
              col: 2,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
interface Foo {
  foo(s: string): void;
  ['foo'](n: number): void;
  bar(): void;
  baz(): void;
  foo(sn: string | number): void;
}
      "#: [
            {
              line: 7,
              col: 2,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
interface Foo {
  foo(s: string): void;
  'foo'(n: number): void;
  bar(): void;
  baz(): void;
  foo(sn: string | number): void;
}
      "#: [
            {
              line: 7,
              col: 2,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
interface Foo {
  foo(s: string): void;
  name: string;
  foo(n: number): void;
  foo(sn: string | number): void;
  bar(): void;
  baz(): void;
}
      "#: [
            {
              line: 5,
              col: 2,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
interface Foo {
  foo(): void;
  bar: {
    baz(s: string): void;
    baz(n: number): void;
    foo(): void;
    baz(sn: string | number): void;
  };
}
      "#: [
            {
              line: 8,
              col: 4,
              message: "All 'baz' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
interface Foo {
  new (s: string);
  new (n: number);
  foo(): void;
  bar(): void;
  new (sn: string | number);
}
      "#: [
            {
              line: 7,
              col: 2,
              message: "All 'new' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
interface Foo {
  new (s: string);
  foo(): void;
  new (n: number);
  bar(): void;
  new (sn: string | number);
}
      "#: [
            {
              line: 5,
              col: 2,
              message: "All 'new' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            },
            {
              line: 7,
              col: 2,
              message: "All 'new' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
class Foo {
  constructor(s: string);
  constructor(n: number);
  bar(): void {}
  baz(): void {}
  constructor(sn: string | number) {}
}
      "#: [
            {
              line: 7,
              col: 2,
              message: "All 'constructor' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
class Foo {
  foo(s: string): void;
  foo(n: number): void;
  bar(): void {}
  baz(): void {}
  foo(sn: string | number): void {}
}
      "#: [
            {
              line: 7,
              col: 2,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
class Foo {
  foo(s: string): void;
  ['foo'](n: number): void;
  bar(): void {}
  baz(): void {}
  foo(sn: string | number): void {}
}
      "#: [
            {
              line: 7,
              col: 2,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
class Foo {
  // prettier-ignore
  "foo"(s: string): void;
  foo(n: number): void;
  bar(): void {}
  baz(): void {}
  foo(sn: string | number): void {}
}
      "#: [
            {
              line: 8,
              col: 2,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
class Foo {
  constructor(s: string);
  name: string;
  constructor(n: number);
  constructor(sn: string | number) {}
  bar(): void {}
  baz(): void {}
}
      "#: [
            {
              line: 5,
              col: 2,
              message: "All 'constructor' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
class Foo {
  foo(s: string): void;
  name: string;
  foo(n: number): void;
  foo(sn: string | number): void {}
  bar(): void {}
  baz(): void {}
}
      "#: [
            {
              line: 5,
              col: 2,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
class Foo {
  static foo(s: string): void;
  name: string;
  static foo(n: number): void;
  static foo(sn: string | number): void {}
  bar(): void {}
  baz(): void {}
}
      "#: [
            {
              line: 5,
              col: 2,
              message: "All 'foo' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
class Foo {
  foo() {
    class Bar {
      bar(): void;
      baz() {}
      bar(s: string): void;
    }
  }
}
      "#: [
            {
              line: 7,
              col: 6,
              message: "All 'bar' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
class Foo {
  foo() {
    class Bar {
      bar(): void;
      baz() {}
      bar(s: string): void;
    }
  }
}
      "#: [
            {
              line: 7,
              col: 6,
              message: "All 'bar' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ],
r#"
type Foo = {
  foo(): void;
  bar: {
    baz(s: string): void;
    baz(n: number): void;
    foo(): void;
    baz(sn: string | number): void;
  };
}
      "#: [
            {
              line: 8,
              col: 4,
              message: "All 'baz' signatures should be adjacent",
              hint: "Make sure all overloaded signatures are grouped together"
            }
          ]
    };
  }
}
