use futures::{Async, Poll};

use fut::ActorFuture;
use context::Context;


/// Future for the `map` combinator, changing the type of a future.
///
/// This is created by the `Future::map` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Map<A, F> where A: ActorFuture {
    future: A,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> Map<A, F>
    where A: ActorFuture,
{
    Map {
        future: future,
        f: Some(f),
    }
}

impl<U, A, F> ActorFuture for Map<A, F>
    where A: ActorFuture,
          F: FnOnce(A::Item, &mut A::Actor, &mut Context<A::Actor>) -> U,
{
    type Item = U;
    type Error = A::Error;
    type Actor = A::Actor;

    fn poll(&mut self, act: &mut Self::Actor, ctx: &mut Context<Self::Actor>) -> Poll<U, A::Error> {
        let e = match self.future.poll(act, ctx) {
            Ok(Async::NotReady) => return Ok(Async::NotReady),
            Ok(Async::Ready(e)) => Ok(e),
            Err(e) => Err(e),
        };
        match e {
            Ok(item) =>
                Ok(Async::Ready(
                    self.f.take().expect("cannot poll Map twice")(item, act, ctx))),
            Err(err) => Err(err)
        }
    }
}
