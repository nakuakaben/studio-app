use super::*;
use std::sync::mpsc::{self, Receiver};
#[derive(Clone)]
pub struct Request {
    pub id: String,
    pub rules: DetectionRules,
}
pub struct Snapshot {
    pub generation: u64,
    pub entries: Vec<(Request, Report)>,
}
pub fn spawn(
    generation: u64,
    requests: Vec<Request>,
    repaint: impl Fn() + Send + 'static,
) -> Receiver<Snapshot> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let entries = requests
            .into_iter()
            .map(|r| {
                let report = detect(&r.rules);
                (r, report)
            })
            .collect();
        let _ = tx.send(Snapshot {
            generation,
            entries,
        });
        repaint();
    });
    rx
}
