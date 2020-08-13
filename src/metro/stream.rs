use std::collections::LinkedList;
use std::iter::IntoIterator;
use std::sync::mpsc::channel;
use std::sync::mpsc::{Receiver, RecvError};
use std::thread;
use std::thread::JoinHandle;

type Processor<T, U> = dyn (Fn(T) -> U) + Send + Sync + 'static;
trait Processable = Send + Sync + 'static;

struct Node<T: Processable> {
    receiver: Receiver<T>,
}

unsafe impl<T: Processable> Send for Node<T> {}
unsafe impl<T: Processable> Sync for Node<T> {}

fn from<I, T: Processable>(i: I) -> (Node<T>, JoinHandle<()>)
where
    I: IntoIterator<Item = T> + Send + 'static,
{
    let (new_sender, new_receiver) = channel();

    (
        Node {
            receiver: new_receiver,
        },
        thread::spawn(move || {
            for val in i.into_iter() {
                new_sender.send(val).unwrap()
            }
        }),
    )
}

fn map<T: Processable, U: Processable>(
    n: Node<T>,
    f: &'static Processor<T, U>,
) -> (Node<U>, JoinHandle<()>) {
    let (new_sender, new_receiver) = channel();

    (
        Node {
            receiver: new_receiver,
        },
        thread::spawn(move || {
            while let Ok(val) = n.receiver.recv() {
                new_sender.send(f(val)).unwrap();
            }
        }),
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
}

impl<T: Processable> Stream<T> {
    fn from<I>(i: I) -> Stream<T>
    where
        I: IntoIterator<Item = T> + Send + 'static,
    {
        let (source_node, source_process) = from(i);

        let mut processes = LinkedList::new();

        processes.push_back(source_process);
        Stream {
            node: source_node,
            processes,
        }
    }

    fn map<U: Processable>(mut self, f: &'static Processor<T, U>) -> Stream<U> {
        let (new_node, new_process) = map(self.node, f);
        self.processes.push_back(new_process);
        Stream {
            node: new_node,
            processes: self.processes,
        }
    }

    fn collect(mut self) -> LinkedList<T> {
        let out = collect(self.node);

        while let Some(_) = self.processes.front() {
            let p = self.processes.pop_front().unwrap();
            p.join().unwrap();
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

        let (source_node, source_handle) = from(v);
        let (inc_node, inc_handle) = map(source_node, &|i| i + 1);
        let out = collect(inc_node);

        let _ = source_handle.join();
        let _ = inc_handle.join();

        assert_eq!(out, to_list(vec![2, 3, 4, 5]));
    }

    #[test]
    fn stream() {
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
