extern crate futures;
extern crate tokio;

use std::collections::LinkedList;
use std::iter::IntoIterator;
use std::sync::mpsc::channel;
use std::sync::mpsc::{Receiver, RecvError};
use tokio::runtime;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

type Processor<T, U> = dyn (Fn(T) -> U) + Send + Sync + 'static;
trait Processable = Send + 'static;

struct Node<T: Processable> {
    receiver: Receiver<T>,
}

unsafe impl<T: Processable> Send for Node<T> {}
unsafe impl<T: Processable> Sync for Node<T> {}

fn from<I, T: Processable>(i: I, rt: Runtime) -> (Node<T>, JoinHandle<()>, Runtime)
where
    I: IntoIterator<Item = T> + Send + 'static,
{
    let (new_sender, new_receiver) = channel();

    (
        Node {
            receiver: new_receiver,
        },
        rt.spawn(async move {
            for val in i.into_iter() {
                new_sender.send(val).unwrap()
            }
        }),
        rt,
    )
}

fn filter<T: Processable>(
    n: Node<T>,
    pred: &'static Processor<T, Option<T>>,
    rt: Runtime,
) -> (Node<T>, JoinHandle<()>, Runtime) {
    let (new_sender, new_receiver) = channel();

    (
        Node {
            receiver: new_receiver,
        },
        rt.spawn(async move {
            while let Ok(val) = n.receiver.recv() {
                if let Some(passing) = pred(val) {
                    new_sender.send(passing).unwrap();
                }
            }
        }),
        rt,
    )
}

fn map<T: Processable, U: Processable>(
    n: Node<T>,
    f: &'static Processor<T, U>,
    rt: Runtime,
) -> (Node<U>, JoinHandle<()>, Runtime) {
    let (new_sender, new_receiver) = channel();
    (
        Node {
            receiver: new_receiver,
        },
        rt.spawn(async move {
            while let Ok(val) = n.receiver.recv() {
                new_sender.send(f(val)).unwrap();
            }
        }),
        rt,
    )
}

fn collect<T: Processable>(n: Node<T>) -> LinkedList<T> {
    let mut l = LinkedList::new();
    while match n.receiver.recv() {
        Ok(val) => {
            l.push_back(val);
            true
        }
        Err(RecvError) => false,
    } {}
    l
}

struct Stream<T: Processable> {
    node: Node<T>,
    processes: LinkedList<JoinHandle<()>>,
    rt: Runtime,
}

impl<T: Processable> Stream<T> {
    fn from<I>(i: I) -> Stream<T>
    where
        I: IntoIterator<Item = T> + Send + 'static,
    {
        let rt = runtime::Builder::new()
            .threaded_scheduler()
            .build()
            .unwrap();

        let (source_node, source_process, rt) = from(i, rt);

        let mut processes = LinkedList::new();

        processes.push_back(source_process);
        Stream {
            node: source_node,
            processes,
            rt,
        }
    }

    fn filter(mut self, pred: &'static Processor<T, Option<T>>) -> Stream<T> {
        let (new_node, new_process, rt) = filter(self.node, pred, self.rt);
        self.processes.push_back(new_process);
        Stream {
            node: new_node,
            processes: self.processes,
            rt,
        }
    }

    fn map<U: Processable>(mut self, f: &'static Processor<T, U>) -> Stream<U> {
        let (new_node, new_process, rt) = map(self.node, f, self.rt);
        self.processes.push_back(new_process);
        Stream {
            node: new_node,
            processes: self.processes,
            rt,
        }
    }

    fn collect(mut self) -> LinkedList<T> {
        let out = collect(self.node);

        while self.processes.front().is_some() {
            let p = self.processes.pop_front().unwrap();
            futures::executor::block_on(p).unwrap();
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_list<T>(v: Vec<T>) -> LinkedList<T>
    where
        T: Copy,
    {
        let mut out = LinkedList::new();
        for val in v.iter() {
            let vc = val.clone();
            out.push_back(vc);
        }
        out
    }

    #[test]
    fn bare_fns() {
        let v = vec![1, 2, 3, 4];
        let rt = runtime::Builder::new()
            .threaded_scheduler()
            .build()
            .unwrap();

        let (source_node, _source_handle, rt) = from(v, rt);
        let (inc_node, _inc_handle, _) = map(source_node, &|i| i + 1, rt);
        let out = collect(inc_node);

        assert_eq!(out, to_list(vec![2, 3, 4, 5]));
    }

    #[test]
    fn filter_test() {
        let v = vec![1, 2, 3, 4];

        let out = Stream::from(v)
            .filter(&|i| {
                if i % 2 == 0 {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(out, to_list(vec![2, 4]))
    }

    #[test]
    fn map_test() {
        let v = vec![1, 2, 3, 4];

        let out = Stream::from(v)
            .map(&|i| i + 1)
            .map(&|i| i.to_string())
            .collect();

        let mut l = LinkedList::new();
        l.push_back("2".to_string());
        l.push_back("3".to_string());
        l.push_back("4".to_string());
        l.push_back("5".to_string());

        assert_eq!(out, l);
    }
}
