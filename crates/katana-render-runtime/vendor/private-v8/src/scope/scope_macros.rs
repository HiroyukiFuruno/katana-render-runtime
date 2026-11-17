/* Note: the macros below _do not_ use std::pin::pin! because */
/* it leads to worse compiler errors when the scope doesn't live long enough. */
/* Instead, we do the same thing as std::pin::pin! but without the additional temporary scope. */

#[cfg(test)]
#[allow(clippy::macro_metavars_in_unsafe)]
macro_rules! callback_scope {
  (unsafe $scope: ident, $param: expr $(,)?) => {
    #[allow(clippy::macro_metavars_in_unsafe)]
    let mut $scope = {
      /* force the caller to put a separate unsafe block around the param expr */
      let param = $param;
      unsafe { $crate::private_v8::CallbackScope::new(param) }
    };
    /* SAFETY: we are shadowing the original binding, so it can't be accessed */
    /* ever again */
    let mut $scope = {
      let scope_pinned = unsafe { std::pin::Pin::new_unchecked(&mut $scope) };
      scope_pinned.init()
    };
    let $scope = &mut $scope;
  };
  (unsafe let $scope: ident, $param: expr $(,)?) => {
    $crate::private_v8::callback_scope!(unsafe $scope, $param);
  }
}

#[cfg(test)]
pub(crate) use callback_scope;

/// Creates a pinned `HandleScope` for an initialized isolate and binds
/// `&mut PinScope` to `$scope`.
#[allow(unused_macros)]
macro_rules! scope {
  ($scope: ident, $param: expr $(,)?) => {
    let mut $scope = $crate::private_v8::HandleScope::new($param);
    /* SAFETY: we are shadowing the original binding, so it can't be accessed */
    /* ever again */
    let mut $scope = {
      let scope_pinned = unsafe { std::pin::Pin::new_unchecked(&mut $scope) };
      scope_pinned.init()
    };
    let $scope = &mut $scope;
  };
  (let $scope: ident, $param: expr $(,)?) => {
    $crate::private_v8::scope!($scope, $param);
  };
}

#[allow(unused_imports)]
pub(crate) use scope;

#[allow(unused_macros)]
macro_rules! tc_scope {
  ($scope: ident, $param: expr $(,)?) => {
    let mut $scope = $crate::private_v8::TryCatch::new($param);
    /* SAFETY: we are shadowing the original binding, so it can't be accessed */
    /* ever again */
    let mut $scope = {
      let scope_pinned = unsafe { std::pin::Pin::new_unchecked(&mut $scope) };
      scope_pinned.init()
    };
    let $scope = &mut $scope;
  };
  (let $scope: ident, $param: expr $(,)?) => {
    $crate::private_v8::tc_scope!($scope, $param);
  };
}

#[cfg(test)]
macro_rules! disallow_javascript_execution_scope {
  ($scope: ident, $param: expr, $on_failure: expr $(,)?) => {
    let mut $scope = $crate::private_v8::DisallowJavascriptExecutionScope::new(
      $param,
      $on_failure,
    );
    /* SAFETY: we are shadowing the original binding, so it can't be accessed */
    /* ever again */
    let mut $scope = {
      let scope_pinned = unsafe { std::pin::Pin::new_unchecked(&mut $scope) };
      scope_pinned.init()
    };
    let $scope = &mut $scope;
  };
  (let $scope: ident, $param: expr, $on_failure: expr $(,)?) => {
    $crate::private_v8::disallow_javascript_execution_scope!(
      $scope,
      $param,
      $on_failure
    );
  };
}

#[cfg(test)]
pub(crate) use disallow_javascript_execution_scope;

#[cfg(test)]
macro_rules! allow_javascript_execution_scope {
  ($scope: ident, $param: expr $(,)?) => {
    let mut $scope =
      $crate::private_v8::AllowJavascriptExecutionScope::new($param);
    let mut $scope = {
      let scope_pinned = unsafe { std::pin::Pin::new_unchecked(&mut $scope) };
      scope_pinned.init()
    };
    let $scope = &mut $scope;
  };
  (let $scope: ident, $param: expr $(,)?) => {
    $crate::private_v8::allow_javascript_execution_scope!($scope, $param);
  };
}

#[cfg(test)]
pub(crate) use allow_javascript_execution_scope;

#[cfg(test)]
macro_rules! escapable_handle_scope {
  ($scope: ident, $param: expr $(,)?) => {
    let mut $scope = $crate::private_v8::EscapableHandleScope::new($param);
    let mut $scope = {
      let scope_pinned = unsafe { std::pin::Pin::new_unchecked(&mut $scope) };
      scope_pinned.init()
    };
    let $scope = &mut $scope;
  };
  (let $scope: ident, $param: expr $(,)?) => {
    $crate::private_v8::escapable_handle_scope!($scope, $param);
  };
}

#[cfg(test)]
pub(crate) use escapable_handle_scope;

pub(crate) use tc_scope;
