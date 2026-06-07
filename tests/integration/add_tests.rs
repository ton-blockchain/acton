use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

#[test]
fn add_counter_template_to_existing_project() {
    let project = ProjectBuilder::new("add-counter-template")
        .without_acton_toml()
        .build();

    project
        .acton()
        .current_dir(project.path())
        .arg("new")
        .arg(".")
        .arg("--name")
        .arg("add-counter-template")
        .arg("--description")
        .arg("Add template integration test")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .arg("--overwrite")
        .run()
        .success();

    project
        .acton()
        .current_dir(project.path())
        .arg("add")
        .arg("contract")
        .arg("--from")
        .arg("template")
        .arg("counter")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/add/add_counter_template.stdout.txt")
        .assert_file_snapshot_matches(
            "Acton.toml",
            "integration/snapshots/add/add_counter_template.acton.toml",
        )
        .assert_file_snapshot_matches(
            "wrappers/counter/Counter.gen.tolk",
            "integration/snapshots/add/add_counter_template.wrapper.tolk",
        );

    project
        .acton()
        .current_dir(project.path())
        .arg("add")
        .arg("contract")
        .arg("--from")
        .arg("template")
        .arg("counter")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/add/add_counter_template_duplicate.stderr.txt",
        );

    project
        .acton()
        .build()
        .arg("Counter")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/add/add_counter_template_build.stdout.txt");
}
