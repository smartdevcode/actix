use std::cell::Cell;
use std::marker::PhantomData;

use futures::{Async, Future, Poll};
use futures::sync::oneshot::{channel, Canceled, Receiver, Sender};

use fut::ActorFuture;
use actor::{Actor, MessageHandler, MessageResponse};
use address::{Subscriber, AsyncSubscriber, MessageProxy, Proxy, ActorAddress};
use context::Context;
use queue::sync;
use message::MessageFuture;


/// Address of the actor `A`. Actor can run in differend thread.
pub struct SyncAddress<A> where A: Actor {
    tx: sync::UnboundedSender<Proxy<A>>,
    closed: Cell<bool>,
}

impl<A> Clone for SyncAddress<A> where A: Actor {
    fn clone(&self) -> Self {
        SyncAddress{tx: self.tx.clone(), closed: self.closed.clone()}
    }
}

impl<A> ActorAddress<A, SyncAddress<A>> for A where A: Actor {

    fn get(ctx: &mut Context<A>) -> SyncAddress<A> {
        ctx.address_cell().sync_address()
    }
}

impl<A> SyncAddress<A> where A: Actor {

    pub(crate) fn new(sender: sync::UnboundedSender<Proxy<A>>) -> SyncAddress<A> {
        SyncAddress{tx: sender, closed: Cell::new(false)}
    }

    /// Indicates if address is closed on other side.
    pub fn is_closed(&self) -> bool {
        self.closed.get()
    }

    /// Send message `M` to actor `A`. Message cold be sent to actor running in
    /// different thread.
    pub fn send<M: 'static + Send>(&self, msg: M)
        where A: MessageHandler<M> + MessageResponse<M>,
              A::Item: Send,
              A::Error: Send,
    {
        if self.tx.unbounded_send(
            Proxy::new(SyncEnvelope::new(Some(msg), None))).is_err()
        {
            self.closed.set(true)
        }
    }

    /// Send message to actor `A` and asyncronously wait for response.
    pub fn call<B: Actor, M: 'static + Send>(&self, msg: M) -> MessageResult<A, B, M>
        where A: MessageHandler<M>,
              A::Item: Send,
              A::Error: Send,
    {
        let (tx, rx) = channel();
        if self.tx.unbounded_send(
            Proxy::new(SyncEnvelope::new(Some(msg), Some(tx)))).is_err()
        {
            self.closed.set(true)
        }

        MessageResult::new(rx)
    }

    /// Send message to actor `A` and asyncronously wait for response.
    pub fn call_fut<M>(&self, msg: M) -> Receiver<Result<A::Item, A::Error>>
        where A: MessageHandler<M>,
              M: 'static
    {
        let (tx, rx) = channel();
        if self.tx.unbounded_send(
            Proxy::new(SyncEnvelope::new(Some(msg), Some(tx)))).is_err()
        {
            self.closed.set(true)
        }

        rx
    }

    /// Get `Subscriber` for specific message type
    pub fn subscriber<M: 'static + Send>(&self) -> Box<Subscriber<M> + Send>
        where A: MessageHandler<M>,
              A::Item: Send,
              A::Error: Send,
    {
        Box::new(self.clone())
    }

    pub fn async_subscriber<M>(&self)
                               -> Box<AsyncSubscriber<M, Future=CallResult<A::Item, A::Error>>>
        where A: MessageHandler<M>,
              A::Item: Send,
              A::Error: Send,
              M: 'static + Send,
    {
        Box::new(self.clone())
    }
}

impl<A, M> Subscriber<M> for SyncAddress<A>
    where M: 'static + Send,
          A::Item: Send,
          A::Error: Send,
          A: Actor + MessageHandler<M>
{
    fn send(&self, msg: M) {
        self.send(msg)
    }

    fn unbuffered_send(&self, msg: M) -> Result<(), M> {
        self.send(msg);
        Ok(())
    }
}

impl<A, M> AsyncSubscriber<M> for SyncAddress<A>
    where M: 'static + Send,
          A: Actor + MessageHandler<M>,
          A::Item: Send,
          A::Error: Send,
{
    type Future = CallResult<A::Item, A::Error>;

    fn call(&self, msg: M) -> Self::Future
    {
        let (tx, rx) = channel();
        if self.tx.unbounded_send(
            Proxy::new(SyncEnvelope::new(Some(msg), Some(tx)))).is_err()
        {
            self.closed.set(true)
        }

        CallResult::new(rx)
    }

    fn unbuffered_call(&self, msg: M) -> Result<Self::Future, M>
    {
        Ok(AsyncSubscriber::call(self, msg))
    }
}

struct SyncEnvelope<A, M> where A: Actor + MessageHandler<M>
{
    msg: Option<M>,
    act: PhantomData<A>,
    tx: Option<Sender<Result<A::Item, A::Error>>>,
}

impl<A, M> SyncEnvelope<A, M> where A: Actor + MessageHandler<M>
{
    fn new(msg: Option<M>,
           tx: Option<Sender<Result<A::Item, A::Error>>>) -> SyncEnvelope<A, M>
    {
        SyncEnvelope{msg: msg, tx: tx, act: PhantomData}
    }
}

impl<A, M> MessageProxy for SyncEnvelope<A, M>
    where M: 'static, A: Actor + MessageHandler<M>,
{
    type Actor = A;

    fn handle(&mut self, act: &mut Self::Actor, ctx: &mut Context<A>)
    {
        if let Some(msg) = self.msg.take() {
            let fut = <Self::Actor as MessageHandler<M>>::handle(act, msg, ctx);
            let f: EnvelopFuture<Self::Actor, _> = EnvelopFuture {msg: PhantomData,
                                                                  fut: fut,
                                                                  tx: self.tx.take()};
            ctx.spawn(f);
        }
    }
}

struct EnvelopFuture<A, M> where A: Actor + MessageHandler<M>
{
    msg: PhantomData<M>,
    fut: MessageFuture<A, M>,
    tx: Option<Sender<Result<A::Item, A::Error>>>,
}

impl<A, M> ActorFuture for EnvelopFuture<A, M> where A: Actor + MessageHandler<M>
{
    type Item = ();
    type Error = ();
    type Actor = A;

    fn poll(&mut self, act: &mut A, ctx: &mut Context<A>) -> Poll<Self::Item, Self::Error>
    {
        match self.fut.poll(act, ctx) {
            Ok(Async::Ready(val)) => {
                if let Some(tx) = self.tx.take() {
                    let _ = tx.send(Ok(val));
                }
                Ok(Async::Ready(()))
            },
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(err) => {
                if let Some(tx) = self.tx.take() {
                    let _ = tx.send(Err(err));
                }
                Err(())
            }
        }
    }
}

#[must_use = "future do nothing unless polled"]
pub struct MessageResult<A, B, M>
    where A: MessageHandler<M>,
          B: Actor,
{
    rx: Receiver<Result<A::Item, A::Error>>,
    act: PhantomData<B>,
}

impl<A, B, M> MessageResult<A, B, M>
    where B: Actor,
          A: MessageHandler<M>
{
    pub(crate) fn new(rx: Receiver<Result<A::Item, A::Error>>) -> MessageResult<A, B, M> {
        MessageResult{rx: rx, act: PhantomData}
    }
}

impl<A, B, M> ActorFuture for MessageResult<A, B, M>
    where B: Actor,
          A: MessageHandler<M>
{
    type Item = Result<A::Item, A::Error>;
    type Error = Canceled;
    type Actor = A;

    fn poll(&mut self, _: &mut A, _: &mut Context<A>) -> Poll<Self::Item, Self::Error>
    {
        self.rx.poll()
    }
}

#[must_use = "future do nothing unless polled"]
pub struct CallResult<I, E>
{
    rx: Receiver<Result<I, E>>,
}

impl<I, E> CallResult<I, E>
{
    fn new(rx: Receiver<Result<I, E>>) -> CallResult<I, E> {
        CallResult{rx: rx}
    }
}

impl<I, E> Future for CallResult<I, E>
{
    type Item = Result<I, E>;
    type Error = Canceled;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.rx.poll()
    }
}
