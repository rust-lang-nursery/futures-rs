use futures::stable::block_on_stable;
use futures::executor::{block_on, ThreadPool};
use futures::prelude::*;

#[async]
fn foo() -> Result<i32, i32> {
    Ok(1)
}

#[async]
fn bar(x: &i32) -> Result<i32, i32> {
    Ok(*x)
}

#[async]
fn baz(x: i32) -> Result<i32, i32> {
    await!(bar(&x))
}

#[async(boxed)]
fn boxed(x: i32) -> Result<i32, i32> {
    Ok(x)
}

#[async(boxed)]
fn boxed_borrow(x: &i32) -> Result<i32, i32> {
    Ok(*x)
}

#[async(boxed, send)]
fn boxed_send(x: i32) -> Result<i32, i32> {
    Ok(x)
}

#[async(boxed, send)]
fn boxed_send_borrow(x: &i32) -> Result<i32, i32> {
    Ok(*x)
}

#[async(boxed, send)]
fn spawnable() -> Result<(), Never> {
    Ok(())
}

#[async_stream(item = u64)]
fn _stream1() -> Result<(), i32> {
    fn integer() -> u64 { 1 }
    let x = &integer();
    stream_yield!(0);
    stream_yield!(*x);
    Ok(())
}

#[async_stream(boxed, item = u64)]
fn _stream_boxed() -> Result<(), i32> {
    fn integer() -> u64 { 1 }
    let x = &integer();
    stream_yield!(0);
    stream_yield!(*x);
    Ok(())
}

#[async]
pub fn uses_async_for() -> Result<Vec<u64>, i32> {
    let mut v = vec![];
    #[async]
    for i in _stream1() {
        v.push(i);
    }
    Ok(v)
}

#[test]
fn main() {
    assert_eq!(block_on_stable(foo()), Ok(1));
    assert_eq!(block_on_stable(bar(&1)), Ok(1));
    assert_eq!(block_on_stable(baz(17)), Ok(17));
    assert_eq!(block_on(boxed(17)), Ok(17));
    assert_eq!(block_on(boxed_send(17)), Ok(17));
    assert_eq!(block_on(boxed_borrow(&17)), Ok(17));
    assert_eq!(block_on(boxed_send_borrow(&17)), Ok(17));
    assert_eq!(block_on_stable(uses_async_for()), Ok(vec![0, 1]));
}

#[test]
fn run_pinned_future_in_thread_pool() {
    let mut pool = ThreadPool::new().unwrap();
    pool.spawn_pinned(spawnable()).unwrap();
}
