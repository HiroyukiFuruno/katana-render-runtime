/* Discriminants follow the V8 Function::New arguments. */
#[repr(C)]
pub(super) enum ConstructorBehavior {
  Allow = 1,
}

#[repr(C)]
pub(super) enum SideEffectType {
  HasSideEffect = 0,
}
