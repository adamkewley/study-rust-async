use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker, RawWaker, RawWakerVTable};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::mem;


trait ArcWake: Send + Sync {
    fn wake(self: Arc<Self>) {
        Self::wake_by_ref(&self)
    }
    fn wake_by_ref(arc_self: &Arc<Self>);
}

fn waker_vtable<W: ArcWake>() -> &'static RawWakerVTable {
    &RawWakerVTable::new(
        clone_arc_raw::<W>,
        wake_arc_raw::<W>,
        wake_by_ref_arc_raw::<W>,
        drop_arc_raw::<W>,
    )
}

fn waker<W>(wake: Arc<W>) -> Waker
where
    W: ArcWake,
{
    let ptr = Arc::into_raw(wake) as *const ();

    unsafe {
        Waker::from_raw(RawWaker::new(ptr, waker_vtable::<W>()))
    }
}

unsafe fn increase_refcount<T: ArcWake>(data: *const ()) {
    // Retain Arc, but don't touch refcount by wrapping in ManuallyDrop
    let arc = mem::ManuallyDrop::new(Arc::<T>::from_raw(data as *const T));
    // Now increase refcount, but don't drop new refcount either
    let _arc_clone: mem::ManuallyDrop<_> = arc.clone();
}

// used by `waker_ref`
unsafe fn clone_arc_raw<T: ArcWake>(data: *const ()) -> RawWaker {
    increase_refcount::<T>(data);
    RawWaker::new(data, waker_vtable::<T>())
}

unsafe fn wake_arc_raw<T: ArcWake>(data: *const ()) {
    let arc: Arc<T> = Arc::from_raw(data as *const T);
    ArcWake::wake(arc);
}

// used by `waker_ref`
unsafe fn wake_by_ref_arc_raw<T: ArcWake>(data: *const ()) {
    // Retain Arc, but don't touch refcount by wrapping in ManuallyDrop
    let arc = mem::ManuallyDrop::new(Arc::<T>::from_raw(data as *const T));
    ArcWake::wake_by_ref(&arc);
}

unsafe fn drop_arc_raw<T: ArcWake>(data: *const ()) {
    drop(Arc::<T>::from_raw(data as *const T))
}


struct ThreadNotify {
    thread: thread::Thread,
}

impl ArcWake for ThreadNotify {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.thread.unpark();
    }
}

thread_local! {
    static CURRENT_THREAD_NOTIFY: Arc<ThreadNotify> = Arc::new(ThreadNotify {
        thread: thread::current(),
    });
}


struct TimerFuture {
    waker: Arc<Mutex<Option<Waker>>>,
    triggered: Arc<AtomicBool>,
}

impl Future for TimerFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.triggered.load(Ordering::Relaxed) {
            Poll::Ready(())
        } else {
            *self.waker.lock().unwrap() = Option::Some(ctx.waker().clone());
            Poll::Pending
        }
    }
}

impl TimerFuture {
    fn new() -> TimerFuture {
        let waker: Arc<Mutex<Option<Waker>>> = Arc::new(Mutex::new(Option::None));
        let triggered = Arc::new(AtomicBool::new(false));

        let waker_subthread = waker.clone();
        let triggered_subthread = triggered.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(5));
            triggered_subthread.store(true, Ordering::Relaxed);
            if let Some(w) = waker_subthread.lock().unwrap().take() {
                w.wake();
            }
        });

        TimerFuture {
            waker: waker,
            triggered: triggered,
        }
    }
}

fn block_on<F: Future>(mut f: F) -> F::Output {
    let mut p = unsafe { Pin::new_unchecked(&mut f) };
    
    CURRENT_THREAD_NOTIFY.with(|n| {
        let waker = waker(n.clone());
        let mut ctx = Context::from_waker(&waker);
        loop {
            if let Poll::Ready(res) = Future::poll(p.as_mut(), &mut ctx) {
                return res;
            }
            thread::park();
        }
    })
}

async fn stop_several_times() -> () {
    for i in 0..10 {
        println!("waiting {}", i);
        TimerFuture::new().await;
    }
}

fn main() {
    block_on(stop_several_times());
    println!("I waited all day!");
}
