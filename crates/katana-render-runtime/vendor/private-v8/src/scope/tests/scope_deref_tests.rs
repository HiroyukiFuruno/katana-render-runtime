use super::*;

#[test]
fn escaped_handle_survives_its_inner_scope() {
  use crate::private_v8::{String, initialize_v8};

  initialize_v8();
  let mut isolate = Isolate::new(Default::default());
  scope!(let outer, &mut isolate);
  let escaped = {
    escapable_handle_scope!(let inner, outer);
    let value = String::new(inner, "escaped").unwrap();
    inner.escape(value)
  };

  assert_eq!(escaped.to_rust_string_lossy(outer), "escaped");
}

macro_rules! assert_deref_chain {
  ($value:expr;) => {};
  ($value:expr; $expected:ty $(, $rest:ty)*) => {{
    let dereferenced = $value.deref_mut();
    AssertTypeOf(dereferenced).is::<$expected>();
    assert_deref_chain!(dereferenced; $($rest),*);
  }};
}

macro_rules! assert_context_child_chains {
  ($context_scope:expr, $context:expr) => {{
    {
      tc_scope!(let l3_tc, &mut **$context_scope);
      AssertTypeOf(l3_tc).is::<PinnedRef<TryCatch<HandleScope>>>();
      assert_deref_chain!(l3_tc; PinnedRef<HandleScope>, PinnedRef<HandleScope<()>>, Isolate);
    }
    {
      disallow_javascript_execution_scope!(let l3_djses, $context_scope, OnFailure::CrashOnFailure);
      AssertTypeOf(l3_djses).is::<PinnedRef<DisallowJavascriptExecutionScope<HandleScope>>>();
      assert_deref_chain!(l3_djses; PinnedRef<HandleScope>, PinnedRef<HandleScope<()>>, Isolate);
      {
        allow_javascript_execution_scope!(let l4_ajses, l3_djses);
        AssertTypeOf(l4_ajses).is::<PinnedRef<AllowJavascriptExecutionScope<DisallowJavascriptExecutionScope<HandleScope>>>>();
        assert_deref_chain!(l4_ajses; PinnedRef<DisallowJavascriptExecutionScope<HandleScope>>, PinnedRef<HandleScope>, PinnedRef<HandleScope<()>>, Isolate);
      }
    }
    {
      escapable_handle_scope!(let l3_ehs, $context_scope);
      AssertTypeOf(l3_ehs).is::<PinnedRef<EscapableHandleScope>>();
      {
        let l4_cxs = &mut ContextScope::new(l3_ehs, $context);
        AssertTypeOf(l4_cxs).is::<ContextScope<EscapableHandleScope>>();
        assert_deref_chain!(l4_cxs; PinnedRef<EscapableHandleScope>, PinnedRef<HandleScope>, PinnedRef<HandleScope<()>>, Isolate);
      }
      {
        tc_scope!(let l4_tc, l3_ehs);
        AssertTypeOf(l4_tc).is::<PinnedRef<TryCatch<EscapableHandleScope>>>();
        assert_deref_chain!(l4_tc; PinnedRef<EscapableHandleScope>, PinnedRef<HandleScope>, PinnedRef<HandleScope<()>>, Isolate);
      }
      {
        disallow_javascript_execution_scope!(let l4_djses, l3_ehs, OnFailure::CrashOnFailure);
        AssertTypeOf(l4_djses).is::<PinnedRef<DisallowJavascriptExecutionScope<EscapableHandleScope>>>();
        assert_deref_chain!(l4_djses; PinnedRef<EscapableHandleScope>, PinnedRef<HandleScope>, PinnedRef<HandleScope<()>>, Isolate);
        {
          allow_javascript_execution_scope!(let l5_ajses, l4_djses);
          AssertTypeOf(l5_ajses).is::<PinnedRef<AllowJavascriptExecutionScope<DisallowJavascriptExecutionScope<EscapableHandleScope>>>>();
          assert_deref_chain!(l5_ajses; PinnedRef<DisallowJavascriptExecutionScope<EscapableHandleScope>>, PinnedRef<EscapableHandleScope>, PinnedRef<HandleScope>, PinnedRef<HandleScope<()>>, Isolate);
        }
      }
    }
  }};
}

#[test]
fn context_scope_deref_types() {
  crate::private_v8::initialize_v8();
  let isolate = &mut Isolate::new(Default::default());
  AssertTypeOf(isolate).is::<OwnedIsolate>();
  let l1_hs = pin!(HandleScope::new(isolate));
  let l1_hs = &mut l1_hs.init();
  AssertTypeOf(l1_hs).is::<PinnedRef<HandleScope<()>>>();
  let context = Context::new(l1_hs, ContextOptions::default());

  let l2_cxs = &mut ContextScope::new(l1_hs, context);
  AssertTypeOf(l2_cxs).is::<ContextScope<HandleScope>>();
  assert_deref_chain!(l2_cxs; PinnedRef<HandleScope>, PinnedRef<HandleScope<()>>, Isolate);
  assert_context_child_chains!(l2_cxs, context);
}

#[test]
fn root_try_catch_scope_deref_types() {
  crate::private_v8::initialize_v8();
  let isolate = &mut Isolate::new(Default::default());
  let l1_hs = pin!(HandleScope::new(isolate));
  let l1_hs = &mut l1_hs.init();

  tc_scope!(let l2_tc, l1_hs);
  AssertTypeOf(l2_tc).is::<PinnedRef<TryCatch<HandleScope<()>>>>();
  assert_deref_chain!(l2_tc; PinnedRef<HandleScope<()>>, Isolate);

  {
    disallow_javascript_execution_scope!(let l3_djses, l2_tc, OnFailure::CrashOnFailure);
    AssertTypeOf(l3_djses).is::<PinnedRef<DisallowJavascriptExecutionScope<TryCatch<HandleScope<()>>>>>();
    assert_deref_chain!(l3_djses; PinnedRef<TryCatch<HandleScope<()>>>, PinnedRef<HandleScope<()>>, Isolate);
    {
      allow_javascript_execution_scope!(let l4_ajses, l3_djses);
      AssertTypeOf(l4_ajses).is::<PinnedRef<AllowJavascriptExecutionScope<DisallowJavascriptExecutionScope<TryCatch<HandleScope<()>>>>>>();
      assert_deref_chain!(l4_ajses; PinnedRef<DisallowJavascriptExecutionScope<TryCatch<HandleScope<()>>>>, PinnedRef<TryCatch<HandleScope<()>>>, PinnedRef<HandleScope<()>>, Isolate);
    }
  }
}

#[test]
fn root_escapable_scope_deref_types() {
  crate::private_v8::initialize_v8();
  let isolate = &mut Isolate::new(Default::default());
  let l1_hs = pin!(HandleScope::new(isolate));
  let l1_hs = &mut l1_hs.init();

  escapable_handle_scope!(let l2_ehs, l1_hs);
  AssertTypeOf(l2_ehs).is::<PinnedRef<EscapableHandleScope<()>>>();
  tc_scope!(let l3_tc, l2_ehs);
  AssertTypeOf(l3_tc).is::<PinnedRef<TryCatch<EscapableHandleScope<()>>>>();
  assert_deref_chain!(l3_tc; PinnedRef<EscapableHandleScope<()>>, PinnedRef<HandleScope<()>>, Isolate);
}

#[test]
fn callback_scope_deref_types() {
  crate::private_v8::initialize_v8();
  let isolate = &mut Isolate::new(Default::default());
  let l1_hs = pin!(HandleScope::new(isolate));
  let l1_hs = &mut l1_hs.init();
  let context = Context::new(l1_hs, ContextOptions::default());

  /* WHY: CallbackScopeはHandleScopeと入場済みContextを前提とする。 */
  let _ = ContextScope::new(l1_hs, context);
  {
    crate::private_v8::callback_scope!(unsafe let l2_cbs, context);
    AssertTypeOf(l2_cbs).is::<PinnedRef<CallbackScope>>();
    assert_deref_chain!(l2_cbs; PinnedRef<HandleScope>, PinnedRef<HandleScope<()>>, Isolate);
  }
  {
    let isolate: &mut Isolate = l1_hs.as_mut();
    callback_scope!(unsafe l2_cbs, isolate);
    AssertTypeOf(l2_cbs).is::<PinnedRef<CallbackScope<()>>>();
    assert_deref_chain!(l2_cbs; PinnedRef<HandleScope<()>>, Isolate);
  }
}
