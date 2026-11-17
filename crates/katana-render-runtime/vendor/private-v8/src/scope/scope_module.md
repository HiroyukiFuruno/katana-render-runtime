 This module contains the rust wrappers for V8's scope types.

 There are two main types of scopes, with the other types being derived from them:
 - `HandleScope` - a scope to create and access `Local` handles
 - `TryCatch` - a scope to catch exceptions thrown from javascript

 There are a few important properties that make v8 scopes challenging to model in rust.
 - `HandleScope`s can (and almost certainly will be) nested, but handles are only
   bound to the innermost `HandleScope`
   - Importantly, this means that the Handle lifetimes are determined by the innermost `HandleScope`
 - Both `HandleScope` and `TryCatch`  cannot be moved, because V8 holds direct pointers to them
 - The C++ API relies heavily on inheritance, which is a bit awkward to model in rust

 # Example

 ```rust
 use v8::{Context, ContextScope, HandleScope, Isolate, Object};
 # let platform = v8::Platform::new(0, false).make_shared();
 # v8::V8::initialize_platform(platform);
 # v8::V8::initialize();

 let isolate = &mut Isolate::new(Default::default());
 let scope = HandleScope::new(isolate);
 let scope = std::pin::pin!(scope);
 let mut scope = scope.init();
 let context = Context::new(&scope, Default::default());

 let context_scope = ContextScope::new(&mut scope, context);
 let object = Object::new(&context_scope);
 ```

 ## Explanation
 The first thing you'll notice is that creating a `HandleScope` requires a few different steps. You'll see this pattern
 across all scope types that are address-sensitive (all except for `ContextScope`):

 1. Allocate the storage for the scope. At this point, the scope is not yet address-sensitive, and so it can be safely moved.
 2. Pin the storage to the stack. This is necessary because once we initialize the scope, it must not be moved.
 3. Initialize the scope. This is where the scope is actually initialized, and our `Pin` ensures that the scope cannot be moved.

 This is a bit verbose, so you can collapse it into two lines,
 ```rust
 # use v8::{HandleScope, Isolate};
 # let platform = v8::Platform::new(0, false).make_shared();
 # v8::V8::initialize_platform(platform);
 # v8::V8::initialize();
 # let isolate = &mut Isolate::new(Default::default());
 let scope = std::pin::pin!(HandleScope::new(isolate));
 let mut scope = scope.init();
 ```

 or use the provided macros:
 ```rust
 # use v8::Isolate;
 # let platform = v8::Platform::new(0, false).make_shared();
 # v8::V8::initialize_platform(platform);
 # v8::V8::initialize();
 # let isolate = &mut Isolate::new(Default::default());
 // note that this expands into statements, introducing a new variable `scope` into the current
 // block. Using it as an expression (`let scope = v8::scope!(let scope, isolate);`) will not work
 v8::scope!(let scope, isolate);
 ```

 # Scopes as function args
 In a function that takes a scope, you'll typically want to take a `PinScope`, like
 ```rust
 fn foo<'s, 'i>(scope: &mut v8::PinScope<'s, 'i>) {
   let local = v8::Number::new(scope, 42.0);
 }
 ```

 `PinScope` is just a shorthand for `PinnedRef<'s, HandleScope<'i>>`, which you can use if you really prefer.

 The lifetimes can sometimes be elided, but if you are taking or returning a `Local`, you'll need to specify at least the first one.
 ```
 fn foo<'s>(scope: &mut v8::PinScope<'s, '_>, arg: v8::Local<'s, v8::Number>) -> v8::Local<'s, v8::Number> {
   v8::Number::new(scope, arg.value() + 42.0)
 }
 ```

 # Deref/DerefMut

 Scopes implement `Deref` and `DerefMut` to allow for sort of "inheritance" of behavior. This is useful because
 it allows most methods (as mentioned above) to just take a `PinScope`, and other scopes will deref to `PinScope`.

 That lets you seamlessly pass, for instance, a `ContextScope` to a function that takes a `PinScope`.
 Note that pinned scopes do not implement `Deref` or `DerefMut`, themselves, rather `PinnedRef` does.

 The deref implementations are:

 PinnedRef<'_, HandleScope<'_, ()>> -> Isolate
 PinnedRef<'_, HandleScope<'_>> -> PinnedRef<'_, HandleScope<'_, ()>>
 PinnedRef<'_, ContextScope<'_, '_>> -> PinnedRef<'_, HandleScope<'_>>
 PinnedRef<'_, CallbackScope<'_, '_>> -> PinnedRef<'_, HandleScope<'_, ()>>



 # Internals

 The initialization process uses the typestate pattern. The storage for the scope is a `ScopeStorage` struct, which is
 then transititions to a `Pin<&mut ScopeStorage<T>>`, and then `init` transitions to a `PinnedRef<'s, T>`.

 The `ScopeStorage` struct tracks initialization state, and is responsible for calling the destructor when the storage is dropped
 (iff the scope was initialized).

 The `PinnedRef` type, returned from `init`, is a transparent wrapper around a `Pin<&mut T>`. The reason it is a newtype is so
 that it can have specialized Deref/DerefMut implementations for the different scope types. `Pin` has a blanket implementation
 that doesn't have the behavior we want.

 ## Lifetimes

 The trickiest part of the scopes here are the lifetimes. In general, the lifetimes exist for a few reasons:
 - ensure that a scope can't outlive the thing it's made from (e.g. an isolate, or another scope)
 - ensure that a scope higher up the stack can't be used until the scope below it has dropped
 - ensure that the `Handle`s bound to the scope do not outlive the scope

 These lifetimes do not need to be exact, and in some cases I'm sure they are shorter than they could be,
 as long as everything lives long enough. In other words, the lifetimes just need to be a safe approximation.


 ### HandleScope
 `HandleScope` itself has only one lifetime, `'i` which is the lifetime of the thing that the scope was created from
 (e.g. an isolate).

 The lifetime for handles bound to the scope is really the lifetime of the `HandleScope` itself. In our case,
 since we've pinned it to the stack, that is the lifetime of the pinned reference. So in
 `PinnedRef<'s, HandleScope<'i>>`, `'s` is the lifetime of the pinned reference, and therefore
 the handles, and 'i is the lifetime of the isolate.

 ### ContextScope
 ContextScope is really just a wrapper around another scope, with a `Context` added to it.
 It wraps a scope, and so it is not actually address-sensitive, and can be moved around freely.

 ContextScope has two lifetimes, `'b` and `'s`. `'b` is the lifetime of the borrow of the scope
 it's wrapping, and `'s` is the lifetime of the scope.

 Effectively you have `&'b PinnedRef<'s, T>`.

 The lifetime for handles bound to the scope is the lifetime of the scope that it was created from.
 So in `ContextScope<'b, 's>`, `'b` is the lifetime of the borrow of the inner scope, and `'s` is the lifetime of the inner scope (and therefore the handles).
