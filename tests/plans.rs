use std::fs;
use std::path::Path;

use arbiter::judge::findings::integrity;
use arbiter::packs::graphed;

fn seeded(root: &Path, record: &str) {
    let _ = fs::remove_dir_all(root);
    fs::create_dir_all(root.join(".arbiter")).expect("the folder is writable");
    fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"demo\"\nversion = \"0\"\n",
    )
    .expect("the folder is writable");
    fs::write(root.join("app.py"), "value = 1\n").expect("the folder is writable");
    fs::write(root.join(".arbiter/plans.toml"), record).expect("the folder is writable");
}

fn named(root: &Path, rule: &str) -> Vec<String> {
    let found = graphed(&root.to_string_lossy()).expect("the folder exists");
    integrity(&found)
        .iter()
        .filter(|finding| finding.name == rule)
        .map(|finding| finding.reason.clone())
        .collect()
}

#[test]
fn the_service_env_names_its_world() {
    let root = std::env::temp_dir().join(format!("arbiter-env-{}", std::process::id()));
    seeded(
        &root,
        "[[service]]\nname = \"api\"\nvolumes = [\"api-data\"]\n\
         env = { API_DATA = \"{volume:api-data}\", API_PORT = \"{port}\" }\n\n\
         [[volume]]\nname = \"api-data\"\nbackup = \"daily\"\n",
    );
    let Some(Ok(reading)) = arbiter::judge::plans::read(&root) else {
        panic!("the record reads");
    };
    let mut pairs = reading.services[0].env.clone();
    pairs.sort();
    assert_eq!(
        pairs,
        [
            ("API_DATA".to_string(), "{volume:api-data}".to_string()),
            ("API_PORT".to_string(), "{port}".to_string()),
        ],
        "the bridge from plan entities to the process is declared"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_impact_of_a_service_names_who_stands_on_it() {
    let root = std::env::temp_dir().join(format!("arbiter-impact-{}", std::process::id()));
    seeded(
        &root,
        "[[service]]\nname = \"api\"\nneeds = [\"postgres\"]\n\n\
         [[service]]\nname = \"worker\"\nchannels = [{ to = \"api\" }]\n\n\
         [[service]]\nname = \"postgres\"\n\n\
         [[service]]\nname = \"aside\"\n",
    );
    let Some(Ok(reading)) = arbiter::judge::plans::read(&root) else {
        panic!("the record reads");
    };
    assert_eq!(
        arbiter::judge::plans::impacted(&reading, "postgres"),
        ["api", "worker"],
        "everything standing on a service falls with it, transitively"
    );
    assert!(
        arbiter::judge::plans::impacted(&reading, "aside").is_empty(),
        "nothing stands on a leaf"
    );
    let (boot, dials) = arbiter::judge::plans::leaning_on(&reading, "postgres");
    assert_eq!(
        boot,
        ["api"],
        "a service that needs it cannot boot without it"
    );
    assert!(dials.is_empty(), "nobody dials postgres directly");
    let (_, dialed) = arbiter::judge::plans::leaning_on(&reading, "api");
    assert_eq!(
        dialed,
        [("worker".to_string(), None)],
        "a dialing service degrades while it runs"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_phantom_service_is_named_wherever_it_is_dialed() {
    let root = std::env::temp_dir().join(format!("arbiter-phantom-{}", std::process::id()));
    seeded(
        &root,
        "[[service]]\nname = \"api\"\nneeds = [\"postgres\"]\n\
         channels = [{ to = \"worker\" }]\n\n\
         [[plan]]\nname = \"local\"\n\
         place = [{ service = \"api\", on = \"cell\" }, { service = \"ghost\", on = \"cell\" }]\n",
    );
    let reasons = named(&root, "phantom-service");
    assert!(reasons.contains(&"api needs postgres, which no service declares".to_string()));
    assert!(reasons.contains(&"api dials worker, which no service declares".to_string()));
    assert!(reasons.contains(&"plan local places ghost, which no service declares".to_string()));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_service_names_the_unit_that_builds_it_or_stays_open() {
    let root = std::env::temp_dir().join(format!("arbiter-unbuilt-{}", std::process::id()));
    seeded(
        &root,
        "[[service]]\nname = \"api\"\nunit = \"apps/ghost\"\n\n\
         [[service]]\nname = \"worker\"\n",
    );
    assert_eq!(
        named(&root, "unbuilt-artifact"),
        ["api names unit apps/ghost, which this repository does not build"]
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_bringup_forms_an_order_or_the_plan_cannot_boot() {
    let root = std::env::temp_dir().join(format!("arbiter-boot-{}", std::process::id()));
    seeded(
        &root,
        "[[service]]\nname = \"api\"\nneeds = [\"worker\"]\n\n\
         [[service]]\nname = \"worker\"\nneeds = [\"api\"]\n",
    );
    assert_eq!(
        named(&root, "boot-cycle"),
        ["the needs of api <-> worker form a cycle; this plan cannot boot"]
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn state_is_declared_and_backed_or_named() {
    let root = std::env::temp_dir().join(format!("arbiter-state-{}", std::process::id()));
    seeded(
        &root,
        "[[service]]\nname = \"api\"\nvolumes = [\"api-data\", \"scratch\"]\n\n\
         [[volume]]\nname = \"api-data\"\n\n\
         [[volume]]\nname = \"kept\"\nbackup = \"daily\"\n",
    );
    assert_eq!(
        named(&root, "undeclared-volume"),
        ["api mounts scratch, which the record does not declare"]
    );
    assert_eq!(
        named(&root, "unbacked-volume"),
        ["api-data names no backup policy; state is never implicit"]
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_channel_needs_the_called_services_consent() {
    let root = std::env::temp_dir().join(format!("arbiter-consent-{}", std::process::id()));
    seeded(
        &root,
        "[[service]]\nname = \"api\"\nchannels = [{ to = \"worker\" }]\n\n\
         [[service]]\nname = \"worker\"\naccepts = [\"web\"]\n\n\
         [[service]]\nname = \"free\"\nchannels = [{ to = \"api\" }]\n",
    );
    assert_eq!(
        named(&root, "uninvited-channel"),
        ["api dials worker, which only web may"]
    );
    assert_eq!(
        named(&root, "stale-channel-grant"),
        ["worker grants web, which never dials it"]
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_plan_fits_its_machines_and_places_every_service() {
    let root = std::env::temp_dir().join(format!("arbiter-fit-{}", std::process::id()));
    seeded(
        &root,
        "[[service]]\nname = \"api\"\ncpu = 6.0\n\n\
         [[service]]\nname = \"worker\"\ncpu = 4.0\n\n\
         [[service]]\nname = \"idle\"\n\n\
         [[machine]]\nname = \"cell\"\ncpu = 8\n\n\
         [[plan]]\nname = \"local\"\n\
         place = [{ service = \"api\", on = \"cell\" }, { service = \"worker\", on = \"cell\" }]\n",
    );
    assert_eq!(
        named(&root, "unplanned-service"),
        ["idle is placed by no plan"]
    );
    assert_eq!(
        named(&root, "overcommitted-machine"),
        ["plan local loads cell past its declared capacity"]
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_bringup_ladder_reads_in_order() {
    use arbiter::judge::plans;
    let root = std::env::temp_dir().join(format!("arbiter-ladder-{}", std::process::id()));
    seeded(
        &root,
        "[[service]]\nname = \"api\"\nneeds = [\"postgres\"]\n\n\
         [[service]]\nname = \"worker\"\nneeds = [\"postgres\"]\n\n\
         [[service]]\nname = \"postgres\"\n\n\
         [[plan]]\nname = \"local\"\nplace = [\
         { service = \"api\", on = \"cell\" }, \
         { service = \"worker\", on = \"cell\" }, \
         { service = \"postgres\", on = \"cell\" }]\n",
    );
    let Some(Ok(reading)) = plans::read(&root) else {
        panic!("the record is written and parses");
    };
    let rungs = plans::ladder(&reading, &reading.plans[0]);
    assert_eq!(
        rungs,
        [
            vec![("postgres".to_string(), "cell".to_string())],
            vec![
                ("api".to_string(), "cell".to_string()),
                ("worker".to_string(), "cell".to_string())
            ],
        ]
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_replica_count_scales_the_plans_load() {
    let root = std::env::temp_dir().join(format!("arbiter-replicas-{}", std::process::id()));
    seeded(
        &root,
        "[[service]]\nname = \"backend\"\ncpu = 3.0\nreplicas = 3\n\n\
         [[machine]]\nname = \"cell\"\ncpu = 8\n\n\
         [[plan]]\nname = \"local\"\nplace = [{ service = \"backend\", on = \"cell\" }]\n",
    );
    assert_eq!(
        named(&root, "overcommitted-machine"),
        ["plan local loads cell past its declared capacity"]
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_door_routes_to_a_declared_service() {
    let root = std::env::temp_dir().join(format!("arbiter-door-{}", std::process::id()));
    seeded(
        &root,
        "[[service]]\nname = \"edge\"\n\
         serves = [{ domain = \"$DOMAIN\", port = 443, to = \"web\" }]\n",
    );
    assert_eq!(
        named(&root, "phantom-service"),
        ["edge serves web, which no service declares"]
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_court_page_embeds_the_snapshot_whole() {
    let data = serde_json::json!({ "marker": "the-whole-snapshot-7193" });
    let page = arbiter::judge::court::page(&data);
    assert!(page.starts_with("<!doctype html"));
    assert!(page.contains("the-whole-snapshot-7193"));
    assert!(
        !page.contains("/*ARBITER_DATA*/"),
        "the placeholder is replaced"
    );
}
