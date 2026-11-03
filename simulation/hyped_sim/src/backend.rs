use std::{
    cell::RefCell,
    collections::HashMap,
    net::TcpStream,
    rc::Rc,
    sync::mpsc::{self, Sender},
    thread,
};

use qtbridge::{QmlObject, qobject};

use crate::{
    monitor::{self, Job, MonitorEvent},
    uart::{Subscription, UartEvent},
};

const INVALID_ID: i32 = -1;

pub struct Backend {
    next_id: i32,
    monitor_jobs: Sender<Job>,
    subscriptions: HashMap<i32, Subscription>,
}

impl Backend {
    /// Must be called on the Qt thread, after the QApp has been created.
    pub fn new(monitor: TcpStream) -> Rc<RefCell<Self>> {
        let (monitor_jobs, jobs) = mpsc::channel();
        let backend = Rc::new(RefCell::new(Self {
            next_id: 0,
            monitor_jobs,
            subscriptions: HashMap::new(),
        }));
        Self::attach_qobject(&backend);

        let handle = backend.borrow().get_thread_handle();
        thread::spawn(move || {
            monitor::run(monitor, jobs, |event| match event {
                MonitorEvent::Output(text) => handle.invoke_mut(move |b| b.monitor_output(text)),
                MonitorEvent::Finished { id, succeeded } => {
                    handle.invoke_mut(move |b| b.monitor_job_finished(id, succeeded))
                }
            });
        });

        backend
    }

    fn take_id(&mut self) -> i32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn submit(&mut self, command: String) -> i32 {
        let id = self.take_id();
        // The worker only stops once this sender is gone
        let _ = self.monitor_jobs.send(Job { id, command });
        id
    }
}

// The signals below are emitted while the backend is mutably borrowed, so their QML handlers
// can't call back into any of the slots.
#[qobject(NoQmlElement)]
impl Backend {
    #[qsignal]
    fn uart_connected(&mut self, id: i32, connected: bool);

    #[qsignal]
    fn uart_text(&mut self, id: i32, text: String);

    #[qsignal]
    fn monitor_output(&mut self, text: String);

    #[qsignal]
    fn monitor_job_finished(&mut self, id: i32, succeeded: bool);

    /// Returns the id that identifies this subscription in the uart signals
    #[qslot]
    fn subscribe_uart(&mut self, port: i32) -> i32 {
        let Ok(port) = u16::try_from(port) else {
            return INVALID_ID;
        };
        let id = self.take_id();
        let handle = self.get_thread_handle();
        let subscription = Subscription::new(port, move |event| match event {
            UartEvent::Connected(connected) => {
                handle.invoke_mut(move |b| b.uart_connected(id, connected))
            }
            UartEvent::Text(text) => handle.invoke_mut(move |b| b.uart_text(id, text)),
        });
        self.subscriptions.insert(id, subscription);
        id
    }

    #[qslot]
    fn unsubscribe_uart(&mut self, id: i32) {
        self.subscriptions.remove(&id);
    }

    /// Returns the id that identifies this load in monitor_job_finished
    #[qslot]
    fn load_script(&mut self, path: String) -> i32 {
        self.submit(monitor::include_command(&path))
    }

    #[qslot]
    fn clear_emulation(&mut self) {
        self.submit("Clear".to_owned());
    }
}
