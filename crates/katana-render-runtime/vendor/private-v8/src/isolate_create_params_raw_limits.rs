#[repr(C)]
#[derive(Debug)]
pub(crate) struct ResourceConstraints {
  pub(crate) code_range_size_: usize,
  pub(crate) max_old_generation_size_: usize,
  pub(crate) max_young_generation_size_: usize,
  pub(crate) initial_old_generation_size_: usize,
  pub(crate) initial_young_generation_size_: usize,
  pub(crate) physical_memory_size_: u64,
  pub(crate) stack_limit_: *mut u32,
}
