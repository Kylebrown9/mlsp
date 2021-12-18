/// A test where 100 random actors send messages to each other
/// containing `MlspPackage` with numeric contents that add to each other.
/// This test is not in any way exhaustive, it is just a first attempt
/// at a fuzzing / canary test to catch obvious issues.
use mlsp::{Mlsp, MlspPackage};
use rand::Rng;

use std::thread::sleep;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use std::{borrow::Borrow, num::Wrapping, sync::mpsc};

#[test]
fn random_actors() {
    let mut senders = Vec::new();
    let mut actors = Vec::new();

    // Create actors
    println!("Phase: Creating Actors");
    for i in 0..50 {
        let (sender, receiver) = mpsc::channel();
        senders.push(sender);
        actors.push(DummyActor::new(i, receiver));
    }

    // Give each actor all other actors mailboxes
    println!("Phase: Initializing Mailbox Lists");
    for actor in actors.iter_mut() {
        actor.set_senders(senders.clone());
    }

    // Start all actors
    println!("Phase: Starting All Actors");
    let join_handles: Vec<JoinHandle<_>> = actors
        .into_iter()
        .map(|actor| thread::spawn(|| actor.run()))
        .collect();

    // Send Share messages
    println!("Phase: Sending Initial Messages");
    for sender in senders.iter() {
        let _ = sender.send(Message::Share(
            Mlsp::new(Wrapping(rand::random())).package(),
        ));
    }

    sleep(Duration::from_secs(5));

    // Send Kill messages
    println!("Phase: Sending Kill Messages");
    for sender in senders.iter() {
        let _ = sender.send(Message::Kill);
    }

    // Wait for all threads to terminate
    for handle in join_handles.into_iter() {
        let _ = handle.join();
    }
}

enum Message {
    Kill,
    Share(MlspPackage<Wrapping<u32>>),
}

struct DummyActor {
    id: u32,
    receiver: mpsc::Receiver<Message>,
    senders: Vec<mpsc::Sender<Message>>,
    sum: Wrapping<u32>,
}

impl DummyActor {
    fn new(id: u32, receiver: mpsc::Receiver<Message>) -> Self {
        DummyActor {
            id,
            receiver,
            senders: vec![],
            sum: Wrapping(1),
        }
    }

    fn set_senders(&mut self, senders: Vec<mpsc::Sender<Message>>) {
        self.senders = senders;
    }

    fn run(mut self) {
        println!("Actor {}: Init", self.id);
        let mut rng = rand::thread_rng();
        let mut pointers: Vec<Mlsp<Wrapping<u32>>> = Vec::new();

        loop {
            // drop references when you have too many pointers
            if pointers.len() > 100 {
                pointers = Vec::new();
            }

            if let Ok(val) = self.receiver.recv() {
                match val {
                    Message::Kill => {
                        println!("Actor {}: Received Kill message", self.id);
                        break;
                    }
                    Message::Share(package) => {
                        let new_mlsp = package.unpackage();
                        let contents: &Wrapping<u32> = new_mlsp.borrow();
                        let contents: Wrapping<u32> = *contents;
                        println!(
                            "Actor {}: Received a message with contents: {}",
                            self.id, contents
                        );

                        // Add to list of pointers
                        pointers.push(new_mlsp.clone());

                        // Add value to the sum
                        self.sum += contents;

                        let value = if rng.gen_range(0..100) < 80 {
                            new_mlsp.package()
                        } else {
                            Mlsp::new(self.sum).package()
                        };
                        for _ in 0..2 {
                            let choice = rng.gen_range(0..self.senders.len());
                            let _ = self.senders[choice].send(Message::Share(value.clone()));
                            println!("Actor {}: Sent message to {}", self.id, choice);
                        }
                    }
                }
            } else {
                println!("Actor {}: No input", self.id);
            }
        }
    }
}
