use super::{Output, completed, exchange};
use bollard::container::LogOutput;
use expect_test::expect;

#[tokio::test]
async fn early_exit_retains_stderr_instead_of_broken_stdin() {
    let (input, receiver) = tokio::io::duplex(64);
    drop(receiver);
    let diagnostic = "snapshot restore failed";
    let output = futures::stream::iter([Ok(LogOutput::StdErr {
        message: diagnostic.as_bytes().to_vec().into(),
    })]);
    let payload = vec![0; 1024 * 1024];
    let (output, sent) = exchange(Box::pin(output), Box::pin(input), Some(&payload))
        .await
        .expect("drained output");
    let error = completed(output, sent, Some(1)).err().expect("failed tool");
    expect![["Localton tool exited with Some(1): snapshot restore failed"]]
        .assert_eq(&error.to_string());

    let error = completed(Output::default(), Ok(()), None)
        .err()
        .expect("missing exit code is not success");
    expect![["Localton tool exited with None: "]].assert_eq(&error.to_string());
}
