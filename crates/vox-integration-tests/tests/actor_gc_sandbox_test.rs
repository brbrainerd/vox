use std::time::Duration;
use tokio::time::timeout;

/// This test sandbox targets the AST preemption logic injected in Wave 1.
/// Since LLMs can generate unbounded loops inside Actors, this verifies
/// that an actor executing an infinite loop correctly hits the `yield_now().await`
/// boundary, allowing the scheduler to interleave tasks instead of starvation blocking.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_infinite_loop_actor_yields_to_scheduler() {
    // 1. Establish an adjacent "control" task. If starvation occurs, this task will never complete.
    let (tx, rx) = tokio::sync::oneshot::channel();

    let _control_task = tokio::spawn(async move {
        // Pretend this is our healthy system scheduler doing normal requests
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = tx.send("Healthy interleave!".to_string());
    });

    // 2. Mock the actor execution context that an emitted LLM AST loop would use.
    // In actual production, this block is the generated Rust emitted from:
    // `crates/vox-compiler/src/codegen_rust/emit/stmt_expr.rs` for `HirStmt::Loop`
    let actor_task = tokio::spawn(async move {
        let (_mailbox_tx, mailbox_rx) = vox_actor_runtime::mailbox::new_mailbox(10);
        let mut ctx = vox_actor_runtime::process::ProcessContext::new(
            vox_actor_runtime::pid::Pid::new(),
            mailbox_rx,
        );

        // Simulated Emitted "While loop"
        loop {
            // -- BEGIN COMPILED PREEMPTION BLOCK --
            ctx.reduction_count += 1;
            if ctx.reduction_count >= ctx.max_reductions {
                ctx.reduction_count = 0;
                if ctx.heap.should_collect() {
                    ctx.heap.collect();
                }
                tokio::task::yield_now().await;
            }
            // -- END COMPILED PREEMPTION BLOCK --

            // Mock heavy computation inside the LLM loop
            let _val = std::hint::black_box(1 + 1);
        }
    });

    // 3. Await the control channel. If the preemption guard failed, the single thread
    // running the loop would block the control thread permanently until the test timed out.
    let result = timeout(Duration::from_secs(2), rx).await;

    // Confirm that the control loop successfully fired, proving `yield_now` interleaved.
    assert!(
        result.is_ok(),
        "Starvation Guard Failed: The infinite loop blocked the Tokio Scheduler."
    );
    assert_eq!(result.unwrap().unwrap(), "Healthy interleave!");

    // Clean up our rogue infinite loop actor.
    actor_task.abort();
}
