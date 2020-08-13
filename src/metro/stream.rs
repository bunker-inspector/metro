use std::any::Any;
use std::collections::LinkedList;
use std::iter::IntoIterator;
use std::sync::mpsc::channel;
use std::sync::mpsc::{Receiver, RecvError};
use std::thread;
use std::thread::JoinHandle;

struct Source<I, T>
where
    I: IntoIterator<Item = T> + Send + 'static,
    T: Send + Sync + 'static,
{
    source: I,
}

impl<I, T> Source<I, T>
where
    I: IntoIterator<Item = T> + Send + 'static,
    T: Send + Sync + 'static,
{
    pub fn from(i: I) -> (Node<T>, JoinHandle<()>) {
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
}

struct Node<T>
where
    T: Send + Sync + 'static,
{
    receiver: Receiver<T>,
}

unsafe impl<T> Send for Node<T> where T: Send + Sync + 'static {}

unsafe impl<T> Sync for Node<T> where T: Send + Sync + 'static {}

fn map<T, U>(
    n: Node<T>,
    f: Box<dyn Fn(T) -> U + Send + Sync + 'static>,
) -> (Node<U>, JoinHandle<()>)
where
    T: Send + Sync + 'static,
    U: Send + Sync + 'static,
{
    let (new_sender, new_receiver) = channel();

    (
        Node {
            receiver: new_receiver,
        },
        thread::spawn(move || {
            while match n.receiver.recv() {
                Ok(val) => {
                    new_sender.send(f(val)).unwrap();
                    true
                }
                Err(RecvError) => false,
            } {}
        }),
    )
}

fn sink<T>(n: Node<T>) -> LinkedList<T>
where
    T: Send + Sync + 'static,
{
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

struct Stream<T>
where
    T: Sync + Send + 'static {
    node: Node<T>,
    processes: LinkedList<JoinHandle<()>>
}

impl<T: Any> Stream<T>
where
    T:Sync + Send + 'static {
    fn from<I>(i: I) -> Stream<T>
        where I: IntoIterator<Item = T> + Send + 'static
    {
        let (source_node, source_process) = Source::from(i);

        let mut processes = LinkedList::new();

        processes.push_back(source_process);
        Stream{node: source_node, processes}
    }

    fn map(mut self, f: Box<dyn Fn(T) -> T + Send + Sync + 'static>) -> Self {
        let (new_node, new_process) = map(self.node, f);
        self.node = new_node;
        self.processes.push_back(new_process);
        self
    }

    fn sink(mut self) -> LinkedList<T> {
        let out = sink(self.node);

        while let Some(_) = self.processes.front() {
            let p = self.processes.pop_front().unwrap();
            p.join();
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
    fn foo() {
        let v = vec![1, 2, 3, 4];

        let (source_node, source_handle) = Source::from(v);
        let (inc_node, inc_handle) = map(source_node, Box::new(|i| i + 1));
        let out = sink(inc_node);

        let _ = source_handle.join();
        let _ = inc_handle.join();

        assert_eq!(out, to_list(vec![2, 3, 4, 5]));
    }

    #[test]
    fn bar() {
        let v = vec![1, 2, 3, 4];

        let out = Stream::from(v)
            .map(Box::new(|i| i + 1))
            .map(Box::new(|i| i * 2))
            .sink();

        assert_eq!(out, to_list(vec![4, 6, 8, 10]));
    }
}
