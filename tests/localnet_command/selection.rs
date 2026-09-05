//! Terminal tests exercise real selection and confirmation without touching Docker.

use super::{Service, acton, cli};
use acton_localnet::{CreateNetwork, catalog};
use expect_test::expect;
use expectrl::{Eof, Expect, Session};
use std::time::Duration;
use tokio::process::Command;

#[tokio::test]
async fn selection_and_deletion_require_an_explicit_target_and_confirmation() {
    let mut service = Service::start(false).await;
    let client = service.client().await;
    for name in ["alpha", "beta"] {
        catalog::create(
            &service.state(),
            CreateNetwork {
                name: name.to_owned(),
                ..Default::default()
            },
        )
        .await
        .expect("network definition");
    }

    let mut failures = Vec::new();
    for args in [
        vec!["delete", "--yes"],
        vec!["delete", "alpha"],
        vec!["status"],
        vec!["start"],
    ] {
        let output = Command::from(acton(service.root.path(), &args))
            .arg("--json")
            .output()
            .await
            .expect("noninteractive command");
        failures.push(format!(
            "{}:{}:{}",
            args.join(" "),
            output.status.success(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    expect![[r"
        delete --yes:false:Error: Multiple localnet networks exist; pass a network name or ID in non-interactive mode
        delete alpha:false:Error: Deletion requires confirmation; pass --yes to delete the network in non-interactive mode
        status:false:Error: Multiple localnet networks exist; pass a network name or ID in non-interactive mode
        start:false:Error: Multiple localnet networks exist; pass a network name or ID in non-interactive mode
    "]].assert_eq(&format!("{}\n", failures.join("\n")));

    // Filtering chooses beta rather than relying on filesystem enumeration order.
    let mut terminal =
        Session::spawn(acton(service.root.path(), &["delete"])).expect("delete terminal");
    terminal.set_expect_timeout(Some(Duration::from_secs(15)));
    terminal
        .expect("Select a network")
        .expect("selection prompt");
    terminal.send_line("beta").expect("select beta");
    terminal
        .expect("Delete network \"beta\"")
        .expect("named confirmation");
    terminal.send_line("").expect("default cancellation");
    terminal
        .expect("Network deletion cancelled")
        .expect("cancellation message");
    terminal.expect(Eof).expect("cancelled exit");
    expect![["3"]].assert_eq(
        &catalog::list(&service.state())
            .await
            .expect("catalog")
            .len()
            .to_string(),
    );

    let mut terminal =
        Session::spawn(acton(service.root.path(), &["delete"])).expect("delete terminal");
    terminal.set_expect_timeout(Some(Duration::from_secs(15)));
    terminal
        .expect("Select a network")
        .expect("selection prompt");
    terminal.send_line("beta").expect("select beta");
    terminal
        .expect("Delete network \"beta\"")
        .expect("named confirmation");
    terminal.send_line("y").expect("confirm beta deletion");
    terminal.expect(Eof).expect("deleted exit");
    let remaining = catalog::list(&service.state())
        .await
        .expect("catalog")
        .into_iter()
        .map(|n| n.network.name)
        .collect::<Vec<_>>();
    expect![[r#"["alpha","integration"]"#]]
        .assert_eq(&serde_json::to_string(&remaining).expect("remaining"));
    let deleted = cli(&service.state(), &["delete", "alpha", "--yes"]).await;
    expect![["completed"]].assert_eq(deleted["status"].as_str().expect("delete status"));
    expect![["integration"]].assert_eq(
        &catalog::list(&service.state()).await.expect("catalog")[0]
            .network
            .name,
    );
    service.stop(&client).await;

    drop(service);
}
