use crate::utils::get_path;
use crate::Cli;

pub fn execute(args: Cli) -> Result<&'static str, &'static str> {
    match args.option.clone().unwrap().as_ref() {
        "severity" => check_severity(args),
        "review" => check_review(args),
        "build" => check_build(args),
        _ => panic!("Wrong severity option"),
    }
}

fn check_severity(args: Cli) -> Result<&'static str, &'static str> {
    println!("{}", get_path(args));
    println!("check_severity");
    Ok("ok")
}

fn check_review(args: Cli) -> Result<&'static str, &'static str> {
    println!("check_review");
    Ok::<&str, _>("ok")
}

fn check_build(args: Cli) -> Result<&'static str, &'static str> {
    println!("check_build");
    Ok::<&str, _>("ok")
}
