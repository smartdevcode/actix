use futures::Future;
use futures::unsync::oneshot::{channel, Receiver};

use actor::{Actor, MessageHandler};
use context::{Context, ContextProtocol};
use message::{Envelope, CallResult, MessageResult};
use queue::unsync;
pub use sync_address::SyncAddress;


#[doc(hidden)]
pub trait ActorAddress<A, T> where A: Actor {

    fn get(ctx: &mut Context<A>) -> T;
}

pub trait Subscriber<M: 'static> {

    /// Buffered send
    fn send(&self, msg: M);

    /// Unbuffered send
    fn unbuffered_send(&self, msg: M) -> Result<(), M>;
}

pub trait AsyncSubscriber<M> {

    type Future: Future;

    /// Send message, wait response asynchronously
    fn call(&self, msg: M) -> Self::Future;

    /// Send message, wait response asynchronously
    fn unbuffered_call(&self, msg: M) -> Result<Self::Future, M>;

}

pub(crate) trait MessageProxy {

    type Actor: Actor;

    /// handle message within new actor and context
    fn handle(&mut self, act: &mut Self::Actor, ctx: &mut Context<Self::Actor>);
}

pub(crate) struct Proxy<A>(pub(crate) Box<MessageProxy<Actor=A>>);

impl<A> Proxy<A> where A: Actor {
    pub(crate) fn new<M: 'static + MessageProxy<Actor=A>>(msg: M) -> Self {
        Proxy(Box::new(msg))
    }
}


unsafe impl<T> Send for Proxy<T> {}

/// Address of the actor `A`.
/// Actor has to run in the same thread as owner of the address.
pub struct Address<A> where A: Actor {
    tx: unsync::UnboundedSender<ContextProtocol<A>>
}

impl<A> Clone for Address<A> where A: Actor {
    fn clone(&self) -> Self {
        Address{tx: self.tx.clone() }
    }
}

impl<A> Address<A> where A: Actor {

    pub(crate) fn new(sender: unsync::UnboundedSender<ContextProtocol<A>>) -> Address<A> {
        Address{tx: sender}
    }

    /// Send message `M` to actor `A`.
    pub fn send<M: 'static>(&self, msg: M) where A: MessageHandler<M>
    {
        let _ = self.tx.unbounded_send(
            ContextProtocol::Envelope(
                Proxy::new(Envelope::new(Some(msg), None))));
    }

    /// Send message to actor `A` and asyncronously wait for response.
    pub fn call<B: Actor, M>(&self, msg: M) -> MessageResult<A, B, M>
        where A: MessageHandler<M>,
              M: 'static
    {
        let (tx, rx) = channel();
        let _ = self.tx.unbounded_send(
            ContextProtocol::Envelope(
                Proxy::new(Envelope::new(Some(msg), Some(tx)))));

        MessageResult::new(rx)
    }

    /// Send message to actor `A` and asyncronously wait for response.
    pub fn call_fut<M>(&self, msg: M) -> Receiver<Result<A::Item, A::Error>>
        where A: MessageHandler<M>,
              M: 'static
    {
        let (tx, rx) = channel();
        let _ = self.tx.unbounded_send(
            ContextProtocol::Envelope(
                Proxy::new(Envelope::new(Some(msg), Some(tx)))));

        rx
    }

    /// Upgrade address to SyncAddress.
    pub fn upgrade(&self) -> Receiver<SyncAddress<A>> {
        let (tx, rx) = channel();
        let _ = self.tx.unbounded_send(
            ContextProtocol::SyncAddress(tx));

        rx
    }

    /// Get `Subscriber` for specific message type
    pub fn subscriber<M: 'static>(&self) -> Box<Subscriber<M>>
        where A: MessageHandler<M>
    {
        Box::new(self.clone())
    }
}

impl<A, M: 'static> Subscriber<M> for Address<A>
    where A: Actor + MessageHandler<M>
{

    fn send(&self, msg: M) {
        self.send(msg)
    }

    fn unbuffered_send(&self, msg: M) -> Result<(), M> {
        self.send(msg);
        Ok(())
    }
}

impl<A, M: 'static> AsyncSubscriber<M> for Address<A>
    where A: Actor + MessageHandler<M>
{
    type Future = CallResult<A::Item, A::Error>;

    fn call(&self, msg: M) -> Self::Future
    {
        let (tx, rx) = channel();
        let _ = self.tx.unbounded_send(
            ContextProtocol::Envelope(
                Proxy::new(Envelope::new(Some(msg), Some(tx)))));

        CallResult::new(rx)
    }

    fn unbuffered_call(&self, msg: M) -> Result<Self::Future, M>
    {
        let (tx, rx) = channel();
        let _ = self.tx.unbounded_send(
            ContextProtocol::Envelope(
                Proxy::new(Envelope::new(Some(msg), Some(tx)))));

        Ok(CallResult::new(rx))
    }
}

impl<A> ActorAddress<A, Address<A>> for A where A: Actor {

    fn get(ctx: &mut Context<A>) -> Address<A> {
        ctx.address_cell().unsync_address()
    }
}

impl<A> ActorAddress<A, (Address<A>, SyncAddress<A>)> for A where A: Actor {

    fn get(ctx: &mut Context<A>) -> (Address<A>, SyncAddress<A>) {
        (ctx.address_cell().unsync_address(), ctx.address_cell().sync_address())
    }
}

impl<A> ActorAddress<A, ()> for A where A: Actor {

    fn get(_: &mut Context<A>) -> () {
        ()
    }
}
