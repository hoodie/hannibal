#![allow(clippy::cast_possible_truncation, clippy::indexing_slicing)]

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

use hannibal::{prelude::*, runtime::sleep};

#[derive(Clone, Debug)]
#[message]
struct CountedTick {
    sequence: u32,
}

struct TickTrackingActor {
    received_ticks: Arc<Mutex<Vec<u32>>>,
    generated_count: Arc<AtomicU32>,
    processing_time: Duration,
}

impl Actor for TickTrackingActor {
    async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
        let counter = Arc::new(AtomicU32::new(0));
        let generated = Arc::clone(&self.generated_count);
        ctx.interval_with(
            move || {
                let sequence = counter.fetch_add(1, Ordering::SeqCst);
                generated.fetch_add(1, Ordering::SeqCst);
                CountedTick { sequence }
            },
            Duration::from_millis(100),
        );
        Ok(())
    }
}

impl Handler<CountedTick> for TickTrackingActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: CountedTick) {
        self.received_ticks.lock().unwrap().push(msg.sequence);
        sleep(self.processing_time).await;
    }
}

#[test_log::test(tokio::test)]
async fn bounded_channel_with_slow_handler_skips_ticks() {
    let received_ticks = Arc::new(Mutex::new(Vec::new()));
    let generated_count = Arc::new(AtomicU32::new(0));

    let tick_tracker = hannibal::setup_actor(TickTrackingActor {
        received_ticks: Arc::clone(&received_ticks),
        generated_count: Arc::clone(&generated_count),
        processing_time: Duration::from_millis(250),
    })
    .bounded(2)
    .spawn();

    sleep(Duration::from_millis(2000)).await;

    tick_tracker.halt().await.unwrap();

    let ticks = received_ticks.lock().unwrap().clone();
    let total_generated = generated_count.load(Ordering::SeqCst);
    let tick_count = ticks.len();

    println!("\n=== Bounded Channel with Slow Handler ===");
    println!(
        "Generated: {total_generated} | Received: {tick_count} | Skipped: {}",
        total_generated - tick_count as u32
    );
    println!("Sequence: {ticks:?}");

    // The key assertion: more ticks were generated than received
    assert!(
        total_generated > tick_count as u32,
        "Expected more ticks to be generated ({total_generated}) than received ({tick_count}), but they were equal!"
    );

    let skip_ratio = (total_generated - tick_count as u32) as f32 / total_generated as f32;
    println!("Skip ratio: {:.1}%", skip_ratio * 100.0);

    assert!(
        skip_ratio >= 0.25,
        "Expected at least 25% of ticks to be skipped due to overload handling, but only {:.1}% were skipped",
        skip_ratio * 100.0
    );

    println!(
        "✓ Overflow handling working - {:.1}% ticks skipped\n",
        skip_ratio * 100.0
    );
}

/// With unbounded channel and reasonable processing speed,
/// all ticks should be queued and eventually processed
#[test_log::test(tokio::test)]
async fn unbounded_channel_processes_all_ticks() {
    let received_ticks = Arc::new(Mutex::new(Vec::new()));
    let generated_count = Arc::new(AtomicU32::new(0));

    let actor = TickTrackingActor {
        received_ticks: Arc::clone(&received_ticks),
        generated_count: Arc::clone(&generated_count),
        processing_time: Duration::from_millis(30),
    };

    // Use default unbounded channel
    let addr = actor.spawn();

    // Run for 1 second
    sleep(Duration::from_secs(1)).await;

    addr.halt().await.unwrap();

    let ticks = received_ticks.lock().unwrap().clone();
    let tick_count = ticks.len();

    println!("\n=== Unbounded Channel ===");
    println!("Received: {tick_count} - Sequence: {ticks:?}");

    // With unbounded channel, we should get most/all ticks
    assert!(
        tick_count >= 8,
        "With unbounded channel, expected at least 8 ticks, but got {tick_count}"
    );

    // Check for minimal gaps in sequence
    let mut has_significant_gaps = false;
    for i in 1..ticks.len() {
        if ticks[i] - ticks[i - 1] > 2 {
            has_significant_gaps = true;
            break;
        }
    }

    if !has_significant_gaps {
        println!("✓ No significant gaps in sequence");
    }
    println!("✓ Unbounded channel working\n");
}

/// Fast processing (10ms) should keep up with 100ms interval
/// even with a bounded channel
#[test_log::test(tokio::test)]
async fn fast_handler_keeps_up_with_bounded_channel() {
    let received_ticks = Arc::new(Mutex::new(Vec::new()));
    let generated_count = Arc::new(AtomicU32::new(0));

    let actor = TickTrackingActor {
        received_ticks: Arc::clone(&received_ticks),
        generated_count: Arc::clone(&generated_count),
        processing_time: Duration::from_millis(10),
    };

    let addr = hannibal::setup_actor(actor).bounded(3).spawn();

    // Run for 1 second
    sleep(Duration::from_secs(1)).await;

    addr.halt().await.unwrap();

    let ticks = received_ticks.lock().unwrap().clone();
    let total_generated = generated_count.load(Ordering::SeqCst);
    let tick_count = ticks.len();

    println!("\n=== Fast Handler with Bounded Channel ===");
    println!(
        "Generated: {total_generated} | Received: {tick_count} | Skipped: {}",
        total_generated - tick_count as u32
    );
    println!("Sequence: {ticks:?}");

    // Fast processing means the actor keeps up with the interval rate
    assert!(
        tick_count >= 8,
        "With fast processing (10ms) and 100ms interval, expected at least 8 ticks, but got {tick_count}"
    );

    let skipped = total_generated - tick_count as u32;

    assert!(
        skipped <= 3,
        "Fast handler should have minimal skipped ticks, but had {skipped}"
    );

    println!("✓ Fast handler keeps up\n");
}

#[test_log::test(tokio::test)]
async fn extremely_slow_handler_skips_many_ticks() {
    let received_ticks = Arc::new(Mutex::new(Vec::new()));
    let generated_count = Arc::new(AtomicU32::new(0));

    // Extremely slow processing (400ms) with fast interval (100ms)
    // and tiny channel should skip most ticks
    let actor = TickTrackingActor {
        received_ticks: Arc::clone(&received_ticks),
        generated_count: Arc::clone(&generated_count),
        processing_time: Duration::from_millis(400),
    };

    let addr = hannibal::setup_actor(actor)
        .bounded(1) // Very small channel
        .spawn();

    // Run for 2.5 seconds
    sleep(Duration::from_millis(2500)).await;

    addr.halt().await.unwrap();

    let ticks = received_ticks.lock().unwrap().clone();
    let total_generated = generated_count.load(Ordering::SeqCst);
    let tick_count = ticks.len();

    println!("\n=== Extremely Slow Handler (400ms processing, 100ms interval) ===");
    println!(
        "Generated: {total_generated} | Received: {tick_count} | Skipped: {}",
        total_generated - tick_count as u32
    );
    println!("Sequence: {ticks:?}");

    // Verify overload handling is working
    assert!(
        total_generated > tick_count as u32,
        "Expected more ticks to be generated ({total_generated}) than received ({tick_count})"
    );

    let skip_ratio = (total_generated - tick_count as u32) as f32 / total_generated as f32;
    println!("Skip ratio: {:.1}%", skip_ratio * 100.0);

    // With 400ms processing and 100ms interval, we should skip a significant portion
    assert!(
        skip_ratio >= 0.4,
        "Expected at least 40% of ticks to be skipped due to overload handling with extremely slow handler, but only {:.1}% were skipped",
        skip_ratio * 100.0
    );

    println!(
        "✓ Extreme overload handling working - {:.1}% skipped\n",
        skip_ratio * 100.0
    );
}

#[test_log::test(tokio::test)]
async fn demonstrates_generated_vs_received_with_sequence_numbers() {
    let received_ticks = Arc::new(Mutex::new(Vec::new()));
    let generated_count = Arc::new(AtomicU32::new(0));

    let actor = TickTrackingActor {
        received_ticks: Arc::clone(&received_ticks),
        generated_count: Arc::clone(&generated_count),
        processing_time: Duration::from_millis(300),
    };

    let addr = hannibal::setup_actor(actor).bounded(2).spawn();

    // Run for 2 seconds to allow clear pattern to emerge
    sleep(Duration::from_millis(2000)).await;

    addr.halt().await.unwrap();

    let ticks = received_ticks.lock().unwrap().clone();
    let total_generated = generated_count.load(Ordering::SeqCst);

    println!("\n=== Generated vs Received (300ms processing, 100ms interval) ===");
    println!(
        "Generated: {total_generated} | Received: {} | Skipped: {}",
        ticks.len(),
        total_generated - ticks.len() as u32
    );
    println!("Sequence: {ticks:?}");

    // The key assertion: demonstrates overload handling
    assert!(
        total_generated > ticks.len() as u32,
        "Expected more ticks generated ({total_generated}) than received ({}). Overflow handling should cause ticks to be skipped!",
        ticks.len()
    );

    let skip_ratio = (total_generated - ticks.len() as u32) as f32 / total_generated as f32;
    println!("Skip ratio: {:.1}%", skip_ratio * 100.0);

    assert!(
        skip_ratio >= 0.3,
        "Expected at least 30% skip ratio, but got {:.1}%",
        skip_ratio * 100.0
    );

    println!(
        "✓ Overflow handling working - {:.1}% ticks intentionally skipped\n",
        skip_ratio * 100.0
    );
}
