//! Inspect/apply the same narrow recovery command as the native UI. No sign-in,
//! process launch or arbitrary frontend-supplied filesystem paths.
use aurora_launcher_lib::{
    application as app,
    host::{APPLICATION_IDENTIFIER, Host, LAUNCHER_VERSION, platform_local_data_dir},
};
#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let id = args.next().expect("validated instance identifier required");
    let fingerprint = args.next();
    let host = Host::native(
        platform_local_data_dir(APPLICATION_IDENTIFIER),
        LAUNCHER_VERSION,
        tokio::runtime::Handle::current(),
        |_, _| {},
    );
    if fingerprint.as_deref() == Some("--readiness") {
        let accounts = serde_json::to_value(
            app::get_accounts(host.clone()).expect("account metadata is readable"),
        )
        .unwrap();
        let request = serde_json::from_value(
            serde_json::json!({"instanceId":id,"accountId":accounts.get("selectedAccountId")}),
        )
        .unwrap();
        let report = app::get_play_readiness(host, request)
            .await
            .expect("readiness check completes");
        let mut report = serde_json::to_value(report).unwrap();
        if let Some(fields) = report.as_object_mut() {
            fields.remove("accountId");
            fields.remove("accountName");
        }
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        return;
    }
    let request=serde_json::from_value(serde_json::json!({"instanceId":id,"fingerprint":fingerprint,"keepRemovedMods":fingerprint.is_some()})).unwrap();
    let report = if fingerprint.is_some() {
        app::apply_instance_recovery(host, request).await
    } else {
        app::inspect_instance_recovery(host, request).await
    };
    match report {
        Ok(report) => println!("{}", serde_json::to_string_pretty(&report).unwrap()),
        Err(error) => {
            eprintln!("{error:?}");
            std::process::exit(1);
        }
    }
}
