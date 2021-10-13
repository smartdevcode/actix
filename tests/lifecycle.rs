#![cfg_attr(feature="cargo-clippy", allow(let_unit_value))]

extern crate actix;
extern crate futures;
extern crate tokio_core;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use futures::{future, Future};
use futures::unsync::oneshot::{channel, Sender};
use tokio_core::reactor::Timeout;
use actix::prelude::*;

struct MyActor{
    started: Arc<AtomicBool>,
    stopping: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
    temp: Option<Sender<()>>,
    restore_after_stop: bool,
}

impl Actor for MyActor {
    fn started(&mut self, _: &mut Context<MyActor>) {
        self.started.store(true, Ordering::Relaxed);
    }
    fn stopping(&mut self, ctx: &mut Context<MyActor>) {
        self.stopping.store(true, Ordering::Relaxed);

        if self.restore_after_stop {
            let (tx, rx) = channel();
            self.temp = Some(tx);
            rx.actfuture().then(|_, _: &mut MyActor, _: &mut _| {
                fut::result(Ok(()))
            }).spawn(ctx);
        }
    }
    fn stopped(&mut self, _: &mut Context<MyActor>) {
        self.stopped.store(true, Ordering::Relaxed);
    }
}

#[test]
fn test_active_address() {
    let sys = System::new("test".to_owned());

    let started = Arc::new(AtomicBool::new(false));
    let stopping = Arc::new(AtomicBool::new(false));
    let stopped = Arc::new(AtomicBool::new(false));

    let _addr: Address<_> = MyActor{
        started: Arc::clone(&started),
        stopping: Arc::clone(&stopping),
        stopped: Arc::clone(&stopped),
        temp: None, restore_after_stop: false,
    }.start();

    Arbiter::handle().spawn(
        Timeout::new(Duration::new(0, 100), Arbiter::handle()).unwrap()
            .then(|_| {
                Arbiter::system().send(actix::SystemExit(0));
                future::result(Ok(()))
            })
    );

    sys.run();
    assert!(started.load(Ordering::Relaxed), "Not started");
    assert!(!stopping.load(Ordering::Relaxed), "Stopping");
    assert!(!stopped.load(Ordering::Relaxed), "Stopped");
}

#[test]
fn test_active_sync_address() {
    let sys = System::new("test".to_owned());

    let started = Arc::new(AtomicBool::new(false));
    let stopping = Arc::new(AtomicBool::new(false));
    let stopped = Arc::new(AtomicBool::new(false));

    let _addr: SyncAddress<_> = MyActor{
        started: Arc::clone(&started),
        stopping: Arc::clone(&stopping),
        stopped: Arc::clone(&stopped),
        temp: None, restore_after_stop: false,
    }.start();

    Arbiter::handle().spawn(
        Timeout::new(Duration::new(0, 100), Arbiter::handle()).unwrap()
            .then(|_| {
                Arbiter::system().send(actix::SystemExit(0));
                future::result(Ok(()))
            })
    );

    sys.run();
    assert!(started.load(Ordering::Relaxed), "Not started");
    assert!(!stopping.load(Ordering::Relaxed), "Stopping");
    assert!(!stopped.load(Ordering::Relaxed), "Stopped");
}

#[test]
fn test_stop_after_drop_address() {
    let sys = System::new("test".to_owned());

    let started = Arc::new(AtomicBool::new(false));
    let stopping = Arc::new(AtomicBool::new(false));
    let stopped = Arc::new(AtomicBool::new(false));

    let addr: Address<_> = MyActor{
        started: Arc::clone(&started),
        stopping: Arc::clone(&stopping),
        stopped: Arc::clone(&stopped),
        temp: None, restore_after_stop: false,
    }.start();

    let started2 = Arc::clone(&started);
    let stopping2 = Arc::clone(&stopping);
    let stopped2 = Arc::clone(&stopped);

    Arbiter::handle().spawn_fn(move || {
        assert!(started2.load(Ordering::Relaxed), "Not started");
        assert!(!stopping2.load(Ordering::Relaxed), "Stopping");
        assert!(!stopped2.load(Ordering::Relaxed), "Stopped");

        Timeout::new(Duration::new(0, 100), Arbiter::handle()).unwrap()
            .then(move |_| {
                drop(addr);
                Arbiter::system().send(actix::SystemExit(0));
                future::result(Ok(()))
            })
    });

    sys.run();
    assert!(started.load(Ordering::Relaxed), "Not started");
    assert!(stopping.load(Ordering::Relaxed), "Not stopping");
    assert!(stopped.load(Ordering::Relaxed), "Not stopped");
}

#[test]
fn test_stop_after_drop_sync_address() {
    let sys = System::new("test".to_owned());

    let started = Arc::new(AtomicBool::new(false));
    let stopping = Arc::new(AtomicBool::new(false));
    let stopped = Arc::new(AtomicBool::new(false));

    let addr: SyncAddress<_> = MyActor{
        started: Arc::clone(&started),
        stopping: Arc::clone(&stopping),
        stopped: Arc::clone(&stopped),
        temp: None, restore_after_stop: false,
    }.start();

    let started2 = Arc::clone(&started);
    let stopping2 = Arc::clone(&stopping);
    let stopped2 = Arc::clone(&stopped);

    Arbiter::handle().spawn_fn(move || {
        assert!(started2.load(Ordering::Relaxed), "Not started");
        assert!(!stopping2.load(Ordering::Relaxed), "Stopping");
        assert!(!stopped2.load(Ordering::Relaxed), "Stopped");

        Timeout::new(Duration::new(0, 100), Arbiter::handle()).unwrap()
            .then(move |_| {
                drop(addr);
                Arbiter::system().send(actix::SystemExit(0));
                future::result(Ok(()))
            })
    });

    sys.run();
    assert!(started.load(Ordering::Relaxed), "Not started");
    assert!(stopping.load(Ordering::Relaxed), "Not stopping");
    assert!(stopped.load(Ordering::Relaxed), "Not stopped");
}

#[test]
fn test_stop() {
    let sys = System::new("test".to_owned());

    let started = Arc::new(AtomicBool::new(false));
    let stopping = Arc::new(AtomicBool::new(false));
    let stopped = Arc::new(AtomicBool::new(false));

    let _: () = MyActor{
        started: Arc::clone(&started),
        stopping: Arc::clone(&stopping),
        stopped: Arc::clone(&stopped),
        temp: None, restore_after_stop: false,
    }.start();

    Arbiter::handle().spawn(
        Timeout::new(Duration::new(0, 100), Arbiter::handle()).unwrap()
            .then(|_| {
                Arbiter::system().send(actix::SystemExit(0));
                future::result(Ok(()))
            })
    );

    sys.run();
    assert!(started.load(Ordering::Relaxed), "Not started");
    assert!(stopping.load(Ordering::Relaxed), "Not stopping");
    assert!(stopped.load(Ordering::Relaxed), "Not stopped");
}

#[test]
fn test_stop_restore_after_stopping() {
    let sys = System::new("test".to_owned());

    let started = Arc::new(AtomicBool::new(false));
    let stopping = Arc::new(AtomicBool::new(false));
    let stopped = Arc::new(AtomicBool::new(false));

    let _: () = MyActor{
        started: Arc::clone(&started),
        stopping: Arc::clone(&stopping),
        stopped: Arc::clone(&stopped),
        temp: None, restore_after_stop: true,
    }.start();

    Arbiter::handle().spawn(
        Timeout::new(Duration::new(0, 100), Arbiter::handle()).unwrap()
            .then(|_| {
                Arbiter::system().send(actix::SystemExit(0));
                future::result(Ok(()))
            })
    );

    sys.run();
    assert!(started.load(Ordering::Relaxed), "Not started");
    assert!(stopping.load(Ordering::Relaxed), "Not stopping");
    assert!(!stopped.load(Ordering::Relaxed), "Stopped");
}
