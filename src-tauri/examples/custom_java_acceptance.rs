//! Explicit local runtime check; never discovers Java or opens launcher data.
use aurora_launcher_lib::runtime::external;

#[tokio::main]
async fn main() {
    let mut arguments = std::env::args().skip(1);
    let path = arguments.next().expect("explicit absolute Java executable");
    let required = arguments
        .next()
        .expect("required major")
        .parse()
        .expect("Java major");
    let result = external::validate(&path, required).await;
    println!(
        "{}",
        serde_json::to_string_pretty(&external::ExternalJavaReport::from_result(
            &path, required, &result
        ))
        .unwrap()
    );
    if result.is_err() {
        std::process::exit(1);
    }
}
