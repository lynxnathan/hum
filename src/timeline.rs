use std::time::{Duration, Instant};

use tokio::sync::mpsc::Sender;

use crate::events::DaemonEvent;

/// Run a timeline ticker that sends `DaemonEvent::Tick(pos)` at ~50ms intervals.
///
/// `start_pos` is the initial playback position in seconds. The ticker advances
/// monotonically from that position using wall-clock elapsed time.
///
/// Uses `MissedTickBehavior::Skip` to prevent burst-of-ticks flooding the event
/// loop if reconciliation stalls.
///
/// Exits cleanly when the receiver is dropped (event loop shut down).
pub async fn run_ticker(tx: Sender<DaemonEvent>, start_pos: f64) {
    let start = Instant::now();
    let mut interval = tokio::time::interval(Duration::from_millis(50));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        let pos = start_pos + start.elapsed().as_secs_f64();
        if tx.send(DaemonEvent::Tick(pos)).await.is_err() {
            break; // receiver dropped -- event loop shut down, exit cleanly
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn ticker_sends_ticks() {
        let (tx, mut rx) = mpsc::channel(64);
        let handle = tokio::spawn(run_ticker(tx, 0.0));

        // Receive a few ticks
        let tick1 = rx.recv().await.unwrap();
        let tick2 = rx.recv().await.unwrap();

        match tick1 {
            DaemonEvent::Tick(pos) => assert!(pos >= 0.0),
            _ => panic!("expected Tick"),
        }
        match tick2 {
            DaemonEvent::Tick(pos) => assert!(pos >= 0.0),
            _ => panic!("expected Tick"),
        }

        handle.abort();
    }

    #[tokio::test]
    async fn ticker_advances_monotonically() {
        let (tx, mut rx) = mpsc::channel(64);
        let handle = tokio::spawn(run_ticker(tx, 5.0));

        let mut prev = 0.0;
        for _ in 0..5 {
            if let Some(DaemonEvent::Tick(pos)) = rx.recv().await {
                assert!(pos >= prev, "pos {pos} should be >= prev {prev}");
                assert!(pos >= 5.0, "pos {pos} should be >= start_pos 5.0");
                prev = pos;
            }
        }

        handle.abort();
    }

    #[tokio::test]
    async fn ticker_stops_when_receiver_dropped() {
        let (tx, rx) = mpsc::channel(64);
        let handle = tokio::spawn(run_ticker(tx, 0.0));

        // Drop receiver -- ticker should exit
        drop(rx);

        // Ticker task should complete (not hang)
        let result = tokio::time::timeout(Duration::from_secs(1), handle).await;
        assert!(result.is_ok(), "ticker should exit when receiver is dropped");
    }
}
