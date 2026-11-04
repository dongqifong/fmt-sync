use std::{env, fs::OpenOptions, io::Write, path::Path};

fn main() {
    let argv: Vec<String> = env::args().collect();
    let tool = Path::new(&argv[0]).file_stem().unwrap().to_string_lossy().to_string();
    let args = &argv[1..];
    if let Ok(log) = env::var("FAKE_LOG") {
        let mut file = OpenOptions::new().create(true).append(true).open(log).unwrap();
        writeln!(file, "{{\"tool\":{tool:?},\"args\":{args:?}}}").unwrap();
    }
    if args.iter().any(|a| a == "--version" || a == "-V") || (tool == "go" && args.first().is_some_and(|a| a == "version")) {
        if tool == "cargo" && env::var("FAKE_MISSING_COMPONENT").ok().as_ref() == args.first() {
            eprintln!("error: no such command: {}", args[0]);
            std::process::exit(101);
        }
        let version = env::var("FAKE_VERSION").unwrap_or_else(|_| "1.2.3".into());
        if tool == "go" { println!("go version go{version} test/arch"); }
        else { println!("{tool} {version}"); }
        return;
    }
    if tool == "cargo" && args.first().is_some_and(|a| a == "metadata") {
        println!("{}", env::var("FAKE_METADATA").expect("fixture needs metadata"));
        return;
    }
    if tool == "ruff" && args.first().is_some_and(|a| a == "linter") {
        println!("{}", r#"[{"prefix":"","categories":[{"prefix":"E"},{"prefix":"W"}]},{"prefix":"F"},{"prefix":"I"}]"#);
        return;
    }
    if tool == "gofmt" && args.iter().any(|a| a == "-l") && env::var("FAKE_GOFMT_DIRTY").is_ok() {
        println!("main.go");
    }
    let key = format!("FAKE_{}_EXIT", tool.to_uppercase());
    let code = env::var(key).ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    if code != 0 { eprintln!("{tool}: fixture diagnostic"); }
    std::process::exit(code);
}
