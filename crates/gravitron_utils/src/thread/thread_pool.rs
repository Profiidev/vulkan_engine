use std::{sync::mpsc, thread};

#[cfg(feature = "debug")]
use log::trace;

use crate::thread::Mutator;

enum Message {
  NewJob(Job),
  Terminate,
}

pub struct ThreadPool {
  workers: Vec<Worker>,
  sender: mpsc::Sender<Message>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool {
  pub fn new(size: usize) -> Self {
    assert!(size > 0);
    #[cfg(feature = "debug")]
    trace!("Creating ThreadPool with {} Workers", size);

    let (sender, receiver) = mpsc::channel();
    let receiver = Mutator::new(receiver);

    let mut workers = Vec::with_capacity(size);

    for i in 0..size {
      workers.push(Worker::new(i, Mutator::inner_clone(&receiver)));
    }

    ThreadPool { workers, sender }
  }

  pub fn execute<F>(&self, f: F)
  where
    F: FnOnce() + Send + 'static,
  {
    let job = Box::new(f);

    self.sender.send(Message::NewJob(job)).unwrap();
  }
}

impl Drop for ThreadPool {
  fn drop(&mut self) {
    #[cfg(feature = "debug")]
    trace!("Sending terminate message to workers");
    for _ in &mut self.workers {
      self.sender.send(Message::Terminate).unwrap();
    }

    while let Some(worker) = self.workers.pop() {
      worker.thread.join().unwrap()
    }
  }
}

struct Worker {
  thread: thread::JoinHandle<()>,
}

impl Worker {
  #[allow(unused_variables)]
  fn new(id: usize, receiver: Mutator<mpsc::Receiver<Message>>) -> Self {
    let thread = thread::spawn(move || loop {
      let message = receiver.get().recv().unwrap();

      match message {
        Message::NewJob(job) => {
          #[cfg(feature = "debug")]
          trace!("Worker {} got job", id);
          job();
        }
        Message::Terminate => {
          #[cfg(feature = "debug")]
          trace!("Terminating Worker {}", id);
          break;
        }
      }
    });

    Worker { thread }
  }
}
