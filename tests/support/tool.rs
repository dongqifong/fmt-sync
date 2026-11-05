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
    if tool == "git" && args.first().is_some_and(|a| a == "ls-remote") {
        println!("{}", env::var("FAKE_GIT_TAGS").unwrap_or_default());
    }
    if tool == "ruff" && args.first().is_some_and(|a| a == "linter") {
        if env::var("FAKE_RUFF_BAD_METADATA").is_ok() {
            println!("invalid metadata");
            return;
        }
        println!("{}", r#"[{"prefix":"","categories":[{"prefix":"E"},{"prefix":"W"}]},{"prefix":"F"},{"prefix":"I"}]"#);
        return;
    }
    if tool == "gofmt" && args.iter().any(|a| a == "-l") && env::var("FAKE_GOFMT_DIRTY").is_ok() {
        println!("main.go");
    }
    let key = format!("FAKE_{}_EXIT", tool.to_uppercase());
    let code = if env::var("FAKE_FAILURE_PATH").is_ok_and(|path| args.iter().any(|arg| arg.ends_with(&path))) {
        2
    } else {
        env::var(key).ok().and_then(|s| s.parse().ok()).unwrap_or(0)
    };
    if tool == "cargo" && args.first().is_some_and(|a| a == "install") {
        if let Ok(forbidden) = env::var("FAKE_FORBIDDEN_CWD") {
            assert_ne!(env::current_dir().unwrap(), Path::new(&forbidden));
        }
        if let Ok(executable) = env::var("FAKE_VERIFY_EXISTING_EXE") {
            assert!(std::process::Command::new(executable).arg("--version").output().unwrap().status.success());
        }
        if code == 0 {
            if let Ok(destination) = env::var("FAKE_INSTALL_DEST") {
                let staged = Path::new(&destination).with_extension("staged");
                std::fs::copy(env::var("FAKE_INSTALL_SOURCE").unwrap(), &staged).unwrap();
                std::fs::rename(staged, destination).unwrap();
            }
        }
    }
    if code != 0 { eprintln!("{tool}: fixture diagnostic"); }
    std::process::exit(code);
}
