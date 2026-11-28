use super::*;

#[unsafe(no_mangle)]
unsafe extern "C" fn rusty_v8_RustObj_trace(
  obj: *const RustObj,
  visitor: *mut Visitor,
) {
  unsafe {
    let r = get_rust_obj(obj);
    r.trace(&mut *visitor);
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn rusty_v8_RustObj_get_name(
  obj: *const RustObj,
) -> *const c_char {
  let r = unsafe { get_rust_obj(obj) };
  r.get_name().as_ptr()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn rusty_v8_RustObj_drop(obj: *mut RustObj) {
  unsafe {
    let r = get_rust_obj_mut(obj);
    std::ptr::drop_in_place(r);
  }
}
