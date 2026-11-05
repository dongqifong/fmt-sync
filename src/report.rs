use crate::execution::StageResult;
use clap::builder::styling::AnsiColor;

pub fn print(results: &[StageResult]) -> i32 {
    let mut code = 0;
    for result in results {
        println!(
            "{} {} [{}] {}",
            result.language,
            result.stage,
            result.status.label(),
            result.tool
        );
        if !result.detail.trim().is_empty() {
            println!("{}", result.detail.trim_end());
        }
        code = code.max(result.status.code());
    }
    if code == 0 {
        let green = AnsiColor::Green.on_default();
        anstream::println!("fmt-sync: {green}passed (exit 0){green:#}");
    } else {
        let yellow = AnsiColor::Yellow.on_default();
        anstream::println!("fmt-sync: {yellow}see results above (exit {code}){yellow:#}");
    }
    code
}
