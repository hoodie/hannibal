#![allow(clippy::cast_possible_truncation)]

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use hannibal::{TaskHandle, prelude::*, runtime::sleep};

/// Actor that spawns tasks and tracks their completion
#[derive(Default)]
struct TaskSpawningActor {
    task_completed_flags: Vec<Arc<AtomicBool>>,
}

impl Actor for TaskSpawningActor {}

#[message(response = Arc<AtomicBool>)]
struct SpawnQuickTask {
    duration_ms: u64,
}

impl Handler<SpawnQuickTask> for TaskSpawningActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, msg: SpawnQuickTask) -> Arc<AtomicBool> {
        let completed = Arc::new(AtomicBool::new(false));
        let completed_clone = Arc::clone(&completed);

        ctx.spawn_task(async move {
            sleep(Duration::from_millis(msg.duration_ms)).await;
            completed_clone.store(true, Ordering::SeqCst);
        });

        self.task_completed_flags.push(Arc::clone(&completed));
        completed
    }
}

#[message]
struct TriggerGC;

impl Handler<TriggerGC> for TaskSpawningActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, _: TriggerGC) {
        ctx.gc();
    }
}

/// Tests that `gc()` can be called and removes finished tasks
/// We verify this indirectly by ensuring no panics and that the actor continues to work
#[test_log::test(tokio::test)]
async fn gc_runs_without_errors() {
    let actor = TaskSpawningActor::default();
    let addr = actor.spawn();

    // Spawn several quick tasks
    for _ in 0..5 {
        addr.call(SpawnQuickTask { duration_ms: 50 }).await.unwrap();
    }

    // Wait for tasks to complete
    sleep(Duration::from_millis(100)).await;

    // Trigger GC multiple times - should not panic
    addr.send(TriggerGC).await.unwrap();
    addr.send(TriggerGC).await.unwrap();

    // Actor should still be responsive
    let flag = addr.call(SpawnQuickTask { duration_ms: 10 }).await.unwrap();
    sleep(Duration::from_millis(50)).await;
    assert!(
        flag.load(Ordering::SeqCst),
        "Actor should still work after GC"
    );
}

/// Tests that GC handles mix of finished and running tasks
#[test_log::test(tokio::test)]
async fn gc_with_mixed_task_states() {
    let actor = TaskSpawningActor::default();
    let addr = actor.spawn();

    // Spawn quick tasks that will finish
    let quick_flags: Vec<_> =
        futures::future::join_all((0..3).map(|_| addr.call(SpawnQuickTask { duration_ms: 50 })))
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

    // Spawn slow tasks that will still be running
    let slow_flags: Vec<_> =
        futures::future::join_all((0..2).map(|_| addr.call(SpawnQuickTask { duration_ms: 500 })))
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

    // Wait for quick tasks to finish
    sleep(Duration::from_millis(100)).await;

    for flag in &quick_flags {
        assert!(flag.load(Ordering::SeqCst), "Quick task should be done");
    }
    for flag in &slow_flags {
        assert!(
            !flag.load(Ordering::SeqCst),
            "Slow task should still be running"
        );
    }

    // Trigger GC - should remove finished tasks, keep running ones
    addr.send(TriggerGC).await.unwrap();

    // Wait for slow tasks to finish
    sleep(Duration::from_millis(500)).await;

    for flag in &slow_flags {
        assert!(flag.load(Ordering::SeqCst), "Slow task should now be done");
    }

    // GC again - should clean up the slow tasks
    addr.send(TriggerGC).await.unwrap();

    // Actor should still be fully functional
    let new_flag = addr.call(SpawnQuickTask { duration_ms: 10 }).await.unwrap();
    sleep(Duration::from_millis(50)).await;
    assert!(new_flag.load(Ordering::SeqCst));
}

/// Actor that tracks spawned task handles
#[derive(Default)]
struct TaskTrackingActor;

impl Actor for TaskTrackingActor {}

#[message(response = TaskHandle)]
struct SpawnTrackedTask {
    duration_ms: u64,
}

impl Handler<SpawnTrackedTask> for TaskTrackingActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, msg: SpawnTrackedTask) -> TaskHandle {
        ctx.spawn_task(async move {
            sleep(Duration::from_millis(msg.duration_ms)).await;
        })
    }
}

#[message(response = Option<bool>)]
struct CheckTaskFinished {
    handle: TaskHandle,
}

impl Handler<CheckTaskFinished> for TaskTrackingActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, msg: CheckTaskFinished) -> Option<bool> {
        ctx.is_task_finished(&msg.handle)
    }
}

#[message]
struct StopTrackedTask {
    handle: TaskHandle,
}

impl Handler<StopTrackedTask> for TaskTrackingActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, msg: StopTrackedTask) {
        ctx.stop_task(msg.handle);
    }
}

/// Tests that `is_task_finished` correctly tracks task lifecycle
#[test_log::test(tokio::test)]
async fn is_task_finished_tracks_task_state() {
    let addr = TaskTrackingActor.spawn();

    // Spawn a task that takes 200ms
    let handle: TaskHandle = addr
        .call(SpawnTrackedTask { duration_ms: 200 })
        .await
        .unwrap();

    // Check immediately - should not be finished
    let is_finished = addr.call(CheckTaskFinished { handle }).await.unwrap();
    assert_eq!(is_finished, Some(false), "Task should not be finished yet");

    // Wait for task to complete
    sleep(Duration::from_millis(250)).await;

    // Check again - should be finished
    let is_finished = addr.call(CheckTaskFinished { handle }).await.unwrap();
    assert_eq!(is_finished, Some(true), "Task should be finished now");
}

/// Tests that `stop_task` removes and aborts a running task
#[test_log::test(tokio::test)]
async fn stop_task_aborts_and_removes_task() {
    let addr = TaskTrackingActor.spawn();

    // Spawn a long-running task
    let handle: TaskHandle = addr
        .call(SpawnTrackedTask { duration_ms: 5000 })
        .await
        .unwrap();

    // Verify it's running
    sleep(Duration::from_millis(50)).await;
    let is_finished = addr.call(CheckTaskFinished { handle }).await.unwrap();
    assert_eq!(is_finished, Some(false), "Task should be running");

    // Stop the task
    addr.send(StopTrackedTask { handle }).await.unwrap();

    // Verify task is removed (returns None)
    let is_finished = addr.call(CheckTaskFinished { handle }).await.unwrap();
    assert_eq!(is_finished, None, "Task should be removed after stop");
}

/// Tests multiple GC cycles work correctly
#[test_log::test(tokio::test)]
async fn multiple_gc_cycles() {
    let actor = TaskSpawningActor::default();
    let addr = actor.spawn();

    for cycle in 0..5 {
        // Spawn tasks
        let flags: Vec<_> = futures::future::join_all(
            (0..3).map(|_| addr.call(SpawnQuickTask { duration_ms: 50 })),
        )
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

        // Wait for completion
        sleep(Duration::from_millis(100)).await;

        // Verify all completed
        for (i, flag) in flags.iter().enumerate() {
            assert!(
                flag.load(Ordering::SeqCst),
                "Cycle {}: Task {} should be done",
                cycle,
                i
            );
        }

        // GC
        addr.send(TriggerGC).await.unwrap();
    }

    // Actor should still work after all cycles
    let final_flag = addr.call(SpawnQuickTask { duration_ms: 10 }).await.unwrap();
    sleep(Duration::from_millis(50)).await;
    assert!(final_flag.load(Ordering::SeqCst));
}

/// Tests that tasks are aborted when actor stops
#[test_log::test(tokio::test)]
async fn tasks_aborted_on_actor_stop() {
    let actor = TaskSpawningActor::default();
    let mut addr = actor.spawn();

    // Spawn long-running tasks
    let flags: Vec<_> = futures::future::join_all((0..3).map(|_| {
        addr.call(SpawnQuickTask {
            duration_ms: 10_000,
        })
    }))
    .await
    .into_iter()
    .map(|r| r.unwrap())
    .collect();

    sleep(Duration::from_millis(50)).await;

    // Verify none are completed yet
    for flag in &flags {
        assert!(!flag.load(Ordering::SeqCst), "Task should still be running");
    }

    // Stop the actor
    addr.stop().unwrap();
    addr.await.unwrap();

    // Give a moment for abort to take effect
    sleep(Duration::from_millis(50)).await;

    // Tasks should not have completed (were aborted)
    for flag in &flags {
        assert!(
            !flag.load(Ordering::SeqCst),
            "Task should have been aborted, not completed"
        );
    }
}

/// Tests that checking invalid handle returns None
#[test_log::test(tokio::test)]
async fn check_invalid_handle_returns_none() {
    let addr = TaskTrackingActor.spawn();

    // Create a handle by spawning and immediately stopping
    let handle: TaskHandle = addr
        .call(SpawnTrackedTask { duration_ms: 10 })
        .await
        .unwrap();

    // Stop it immediately
    addr.send(StopTrackedTask { handle }).await.unwrap();

    // Checking should return None
    let result = addr.call(CheckTaskFinished { handle }).await.unwrap();
    assert_eq!(result, None, "Checking removed task should return None");
}

/// Tests spawning many tasks and GC performance
#[test_log::test(tokio::test)]
async fn stress_test_many_tasks() {
    let actor = TaskSpawningActor::default();
    let addr = actor.spawn();

    // Spawn 50 quick tasks
    let flags: Vec<_> =
        futures::future::join_all((0..50).map(|_| addr.call(SpawnQuickTask { duration_ms: 50 })))
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

    // Wait for completion
    sleep(Duration::from_millis(150)).await;

    // Verify all completed
    for flag in &flags {
        assert!(flag.load(Ordering::SeqCst), "Task should be done");
    }

    // GC should handle cleanup without issues
    addr.send(TriggerGC).await.unwrap();

    // Actor should still be responsive
    let new_flag = addr.call(SpawnQuickTask { duration_ms: 10 }).await.unwrap();
    sleep(Duration::from_millis(50)).await;
    assert!(new_flag.load(Ordering::SeqCst));
}

// ============================================================================
// Children GC Tests
// ============================================================================

#[derive(Default, Actor)]
struct ParentActor {
    generic_child_count: usize,
    typed_child_count: usize,
}

#[derive(Default, Actor)]
#[allow(dead_code)]
struct ChildActor {
    id: usize,
}

// Unit message handler for add_child
impl Handler<()> for ChildActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: ()) {}
}

#[derive(Clone)]
#[message]
#[allow(dead_code)]
struct UpdateChild(String);

impl Handler<UpdateChild> for ChildActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: UpdateChild) {}
}

#[message(response = (Addr<ChildActor>, usize))]
struct AddGenericChild;

impl Handler<AddGenericChild> for ParentActor {
    async fn handle(
        &mut self,
        ctx: &mut Context<Self>,
        _: AddGenericChild,
    ) -> (Addr<ChildActor>, usize) {
        let id = self.generic_child_count;
        self.generic_child_count += 1;
        let child = ChildActor { id }.spawn();
        ctx.add_child(child.clone());
        (child, id)
    }
}

#[message(response = (Addr<ChildActor>, usize))]
struct AddTypedChild;

impl Handler<AddTypedChild> for ParentActor {
    async fn handle(
        &mut self,
        ctx: &mut Context<Self>,
        _: AddTypedChild,
    ) -> (Addr<ChildActor>, usize) {
        let id = self.typed_child_count;
        self.typed_child_count += 1;
        let child = ChildActor { id }.spawn();
        ctx.register_child::<UpdateChild>(child.clone());
        (child, id)
    }
}

#[message]
struct TriggerChildGC;

impl Handler<TriggerChildGC> for ParentActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, _: TriggerChildGC) {
        ctx.gc();
    }
}

#[message]
struct SendToTypedChildren;

impl Handler<SendToTypedChildren> for ParentActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, _: SendToTypedChildren) {
        ctx.send_to_children(UpdateChild("test".to_string()));
    }
}

/// Tests that generic children (added via `add_child`) are cleaned up when stopped
#[test_log::test(tokio::test)]
async fn gc_removes_stopped_generic_children() {
    let parent = ParentActor::default().spawn();

    // Add a generic child and get its address
    let (child_addr, _id) = parent.call(AddGenericChild).await.unwrap();

    sleep(Duration::from_millis(50)).await;

    // Verify child is running
    assert!(child_addr.ping().await.is_ok(), "Child should be running");

    // Trigger GC - child is still running, should not be removed
    parent.send(TriggerChildGC).await.unwrap();

    sleep(Duration::from_millis(50)).await;

    // Stop the child
    drop(child_addr);
    sleep(Duration::from_millis(50)).await;

    // Trigger GC - should remove stopped child
    parent.send(TriggerChildGC).await.unwrap();

    // Parent should still work
    parent.send(TriggerChildGC).await.unwrap();
}

/// Tests that typed children (added via `register_child<M>`) are cleaned up when stopped
/// This demonstrates the bug: typed children accumulate in memory even after stopping
#[test_log::test(tokio::test)]
async fn gc_removes_stopped_typed_children() {
    let parent = ParentActor::default().spawn();

    // Add multiple typed children and stop them immediately
    for _ in 0..10 {
        let (child_addr, _id) = parent.call(AddTypedChild).await.unwrap();
        drop(child_addr); // Stop child immediately
        sleep(Duration::from_millis(10)).await;
    }

    // Trigger GC multiple times - should remove all stopped typed children
    parent.send(TriggerChildGC).await.unwrap();
    parent.send(TriggerChildGC).await.unwrap();

    // The bug: parent.children still contains 10 Box<Sender<UpdateChild>> entries
    // even though all children are stopped. This is a memory leak.
    // We can't directly test the count, but we can verify the behavior:

    // Try sending to typed children - should not send to any (all stopped)
    // Currently logs errors for each stopped child
    parent.send(SendToTypedChildren).await.unwrap();

    // Parent should still work fine
    parent.send(TriggerChildGC).await.unwrap();
}

/// Tests mixing generic and typed children
#[test_log::test(tokio::test)]
async fn gc_with_mixed_child_types() {
    let parent = ParentActor::default().spawn();

    // Add both types of children
    let (generic_child, _) = parent.call(AddGenericChild).await.unwrap();
    let (typed_child, _) = parent.call(AddTypedChild).await.unwrap();

    sleep(Duration::from_millis(50)).await;

    // Both should be running
    assert!(generic_child.ping().await.is_ok());
    assert!(typed_child.ping().await.is_ok());

    // Trigger GC with both children alive - should not remove anything
    parent.send(TriggerChildGC).await.unwrap();

    sleep(Duration::from_millis(50)).await;

    // Stop generic child
    drop(generic_child);
    sleep(Duration::from_millis(50)).await;

    // GC should remove generic child but keep typed child
    parent.send(TriggerChildGC).await.unwrap();

    // Typed child should still be reachable
    assert!(typed_child.ping().await.is_ok());

    // Clean up
    drop(typed_child);
    parent.send(TriggerChildGC).await.unwrap();
}

/// Demonstrates the memory leak: typed children accumulate
#[test_log::test(tokio::test)]
async fn typed_children_memory_leak_demonstration() {
    let parent = ParentActor::default().spawn();

    // Create and stop many typed children
    for i in 0..100 {
        let (child, id) = parent.call(AddTypedChild).await.unwrap();
        assert_eq!(id, i, "Child ID should match iteration");
        drop(child); // Stop immediately
    }

    sleep(Duration::from_millis(100)).await;

    // Run GC - currently only removes Sender<()>, not Sender<UpdateChild>
    parent.send(TriggerChildGC).await.unwrap();

    // BUG: parent.children.get(&TypeId::of::<UpdateChild>()) still has 100 entries
    // These are all stopped senders that should have been removed

    // Send to children will attempt to send to all 100 stopped children
    // This demonstrates the leak - errors logged for each stopped child
    parent.send(SendToTypedChildren).await.unwrap();

    // With the fix, there should be 0 stopped children in the HashMap
}
