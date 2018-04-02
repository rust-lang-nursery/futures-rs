use futures::stable::block_on_stable;
use futures::prelude::*;
use std::boxed::PinBox;

#[async]
fn foo() -> Result<i32, i32> {
    Ok(1)
}

#[async(pinned)]
fn bar(x: &i32) -> Result<i32, i32> {
    Ok(*x)
}

#[async]
fn bar2(x: &i32) -> Result<i32, i32> {
    Ok(*x)
}

#[async(pinned)]
fn baz(x: i32) -> Result<i32, i32> {
    await!(bar(&x))
}

#[async(pinned_send)]
fn baz2(x: i32) -> Result<i32, i32> {
    await!(bar2(&x))
}

#[async]
fn qux(
    x: PinBox<Future<Item = i32, Error = i32>>,
    y: PinBox<Future<Item = i32, Error = i32> + Send>,
) -> Result<i32, i32> {
    Ok(await!(x)? + await!(y)?)
}

#[async_stream(item = u64)]
fn _stream1() -> Result<(), i32> {
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
    assert_eq!(block_on_stable(qux(baz(11), baz2(22))), Ok(33));
    assert_eq!(block_on_stable(uses_async_for()), Ok(vec![0, 1]));
}
