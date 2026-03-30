use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const PROJECT_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/io"
"#;

#[test]
fn get_deployed_code_transitions_from_null_to_non_null_in_project_builder() {
    let source = format!(
        r#"
{PROJECT_IMPORTS}

get fun `test-co-get-deployed-code-transition-project-builder`() {{
    val deployer = net.treasury("co_deployer_project_builder");
    val init = ContractState {{
        code: build("simple"),
        data: createEmptyCell(),
    }};
    val target = AutoDeployAddress {{
        stateInit: init,
    }}.calculateAddress();

    expect(net.isDeployed(target)).toBeFalse();
    val before = net.getDeployedCode(target);
    expect(before).toBeNull();
    println("co-before-deploy-code-is-null");

    val deploy = createMessage({{
        bounce: false,
        value: ton("1"),
        dest: {{
            stateInit: init,
        }},
    }});
    net.send(deployer.address, deploy);

    expect(net.isDeployed(target)).toBeTrue();
    val after = net.getDeployedCode(target);
    expect(after).toBeNotNull();
    println("co-after-deploy-code-is-not-null");
}}
"#
    );

    ProjectBuilder::new("co-stdlib-get-deployed-code-project-builder")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("co_get_deployed_code_transition", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("co-before-deploy-code-is-null")
        .assert_contains("co-after-deploy-code-is-not-null")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/get_deployed_code_transitions_from_null_to_non_null_in_project_builder/get_deployed_code_transitions_from_null_to_non_null_in_project_builder.stdout.txt",
        );
}

#[test]
fn get_deployed_code_transitions_from_null_to_non_null_in_fixture_project() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/co_get_deployed_code_transition.test.tolk";

    let source = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/io"

get fun `test-co-get-deployed-code-transition-fixture-project`() {
    val deployer = net.treasury("co_deployer_fixture_project");
    val init = ContractState {
        code: build("counter"),
        data: createEmptyCell(),
    };
    val target = AutoDeployAddress {
        stateInit: init,
    }.calculateAddress();

    expect(net.isDeployed(target)).toBeFalse();
    val before = net.getDeployedCode(target);
    expect(before).toBeNull();
    println("co-fixture-before-deploy-code-is-null");

    val deploy = createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: init,
        },
    });
    net.send(deployer.address, deploy);

    expect(net.isDeployed(target)).toBeTrue();
    val after = net.getDeployedCode(target);
    expect(after).toBeNotNull();
    println("co-fixture-after-deploy-code-is-not-null");
}
"#;

    fs::write(fixture.path().join(test_path), source)
        .expect("failed to write co fixture get deployed code test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("co-fixture-before-deploy-code-is-null")
        .assert_contains("co-fixture-after-deploy-code-is-not-null")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/get_deployed_code_transitions_from_null_to_non_null_in_project_builder/get_deployed_code_transitions_from_null_to_non_null_in_fixture_project.stdout.txt",
        );
}
