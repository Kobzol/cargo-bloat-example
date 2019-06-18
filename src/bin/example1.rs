use std::collections::HashMap;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use std::sync::Mutex;

use futures::future;
use futures::future::Future;
use futures::future::IntoFuture;
use futures::future::ok;
use futures::stream::Stream;
use futures::sync::mpsc::unbounded;
use tokio_zmq::{Dealer, prelude::*};
use tokio_zmq::Multipart;
use zmq::Message;

struct RequestCtx {
    id: u32,
    sender: Option<futures::sync::oneshot::Sender<u32>>
}

fn main() {
    env_logger::init();

    let ctx = Arc::new(zmq::Context::new());
    let rep = Dealer::builder(ctx.clone())
        .connect("tcp://127.0.0.1:5555")
        .build()
        .wait()
        .unwrap();

    let (sink, stream) = rep.sink_stream(8192).split();
    let (sender, receiver) = unbounded::<RequestCtx>();
    let map = Arc::new(Mutex::new(HashMap::<u32, RequestCtx>::new()));
    let counter = Arc::new(AtomicUsize::new(0));

    let channel_map = map.clone();
    // this process reads messages from a channel and sends them to the DEALER
    let channel_process = receiver
        .map(move |chan_val| {
            let id = chan_val.id;
            {
                eprintln!("Sending {}", id);
                let mut m = channel_map.lock().unwrap();
                eprintln!("SendingLock {}", id);
                m.insert(id.clone(), chan_val);
            }

            let data = id.to_string();
            Multipart::from(vec!(Message::from_slice(&data.as_bytes())))
        }).map_err(|_| {
        panic!();
        tokio_zmq::Error::Sink
    }).forward(sink);

    let reader_map = map.clone();

    // this process reads from the DEALER and completes the waiting futures
    let runner = stream
        .map(move |mut multipart| {
            let response = multipart.pop_front().unwrap();
            let id: u32 = response.as_str().unwrap().parse().unwrap();

            eprintln!("Receive {}", id);
            let mut m = reader_map.lock().unwrap();
            eprintln!("ReceiveLock {}", id);
            m.remove(&id).unwrap()
        })
        .map_err(|_| panic!())
        .for_each(|mut req| {
            let sender = req.sender.take().unwrap();
            eprintln!("BeforeSend: {}", req.id);
            sender.send(req.id).unwrap();
            eprintln!("AfterSend: {}", req.id);
            ok(())
        });

    tokio::run(future::lazy(move || {
        tokio::spawn(channel_process.map(|_| ()).map_err(|_| panic!()));
        tokio::spawn(runner.map(|_| { () }).map_err(|_| panic!()));

        for _ in 0..4 {
            let counter = counter.clone();
            let sender = sender.clone();

            // this process repeatedly puts messages into a channel and waits until they are completed
            let process = futures::stream::repeat::<u32, ()>(5)
                .and_then(move |_| {
                    let id = counter.fetch_add(1, Ordering::SeqCst) as u32;
                    let (send, recv) = futures::sync::oneshot::channel();
                    let request = RequestCtx { id, sender: Some(send) };
                    let id = request.id;

                    sender.unbounded_send(request)
                        .into_future()
                        .map_err(|_| panic!())
                        .and_then(move |_| {
                            eprintln!("Waiting {}", id);
                            recv.map_err(|_| panic!())
                        })
                        .map(move |_| {
                            eprintln!("WaitingFinished {}", id);
                        })
                }).for_each(|_| ok(()));
            tokio::spawn(process);
        }

        Ok(())
    }));
}
