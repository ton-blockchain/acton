use super::*;
use crate::{CreateNetwork, catalog};
use expect_test::expect;
use std::time::Duration;

#[tokio::test]
async fn completed_operations_allow_retries_while_history_is_still_being_saved()
-> anyhow::Result<()> {
    let directory = tempfile::tempdir_in("/tmp")?;
    let location = catalog::create(
        directory.path(),
        CreateNetwork {
            name: "operation-completion".into(),
            port_base: Some(29400),
            ..Default::default()
        },
    )
    .await?;
    let runtime = Runtime::open(&location.path).await?;
    runtime.inner.entry.record.write().await.status = Status::Stopped;
    let accepted = runtime.submit(Action::Stop).await?;
    let entry = &runtime.inner.entry;

    // Hold the lock after completion, as the final persistence write does.
    let guard = entry.mutation.lock().await;
    let completed = runtime.operation(&accepted.id).await?;
    entry
        .record
        .write()
        .await
        .operation
        .as_mut()
        .unwrap()
        .status = OperationStatus::Running;
    let busy = runtime
        .submit(Action::Stop)
        .await
        .expect_err("active mutation must be rejected");
    entry
        .record
        .write()
        .await
        .operation
        .as_mut()
        .unwrap()
        .status = completed.status;
    let next = runtime.submit(Action::Stop);
    tokio::pin!(next);
    let waits_for_persistence = tokio::select! {
        result = &mut next => {
            result?;
            false
        }
        () = tokio::time::sleep(Duration::from_millis(20)) => true,
    };
    drop(guard);
    let next = next.await?;
    let guard = entry.mutation.lock().await;
    let final_operation = runtime.operation(&next.id).await?;
    drop(guard);

    expect![[r"completed: Completed
active mutation: operation_in_progress
waited for history: true
next accepted: Running
next completed: Completed"]]
    .assert_eq(&format!(
        "completed: {:?}\nactive mutation: {}\nwaited for history: {waits_for_persistence}\nnext accepted: {:?}\nnext completed: {:?}",
        completed.status,
        busy.code(),
        next.status,
        final_operation.status,
    ));
    Ok(())
}
