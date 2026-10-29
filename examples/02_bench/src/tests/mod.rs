//! The bench's tests beyond each module's own.

// The counter guards read this thread's allocations.
#[global_allocator]
static ALLOC: tungsten::testing::CountingAlloc = tungsten::testing::CountingAlloc;

mod guards;
